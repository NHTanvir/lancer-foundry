use std::collections::HashMap;

use revm::{
    db::InMemoryDB,
    primitives::{AccountInfo, Address, Bytecode, B256, U256},
    Database, DatabaseCommit,
};
use serde_json::json;

/// A fork backend that lazy-loads state from an RPC endpoint.
/// On cache miss it fetches from RPC; hits come from in-memory cache.
pub struct ForkedDb {
    pub rpc_url: String,
    pub fork_block: u64,
    pub cache: InMemoryDB,
    http: reqwest::blocking::Client,
}

impl ForkedDb {
    pub fn new(rpc_url: String, fork_block: u64) -> Self {
        ForkedDb {
            rpc_url,
            fork_block,
            cache: InMemoryDB::default(),
            http: reqwest::blocking::Client::new(),
        }
    }

    fn rpc_call(&self, method: &str, params: serde_json::Value) -> eyre::Result<serde_json::Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1
        });
        let resp: serde_json::Value = self.http.post(&self.rpc_url).json(&body).send()?.json()?;
        if let Some(err) = resp.get("error") {
            eyre::bail!("RPC error: {}", err);
        }
        Ok(resp["result"].clone())
    }

    fn fetch_account(&self, address: Address) -> eyre::Result<AccountInfo> {
        let block_tag = format!("0x{:x}", self.fork_block);
        let addr_hex = format!("{:#x}", address);

        let balance_res = self.rpc_call("eth_getBalance", json!([&addr_hex, &block_tag]))?;
        let nonce_res = self.rpc_call("eth_getTransactionCount", json!([&addr_hex, &block_tag]))?;
        let code_res = self.rpc_call("eth_getCode", json!([&addr_hex, &block_tag]))?;

        let balance = parse_hex_u256(balance_res.as_str().unwrap_or("0x0"));
        let nonce = parse_hex_u64(nonce_res.as_str().unwrap_or("0x0"));
        let code_hex = code_res.as_str().unwrap_or("0x");
        let code_bytes = hex::decode(code_hex.trim_start_matches("0x"))
            .unwrap_or_default();

        let (code, code_hash) = if code_bytes.is_empty() {
            (None, revm::primitives::KECCAK_EMPTY)
        } else {
            let bc = Bytecode::new_raw(revm::primitives::Bytes::copy_from_slice(&code_bytes));
            let hash = bc.hash_slow();
            (Some(bc), hash)
        };

        Ok(AccountInfo { balance, nonce, code_hash, code })
    }

    fn fetch_storage(&self, address: Address, index: U256) -> eyre::Result<U256> {
        let block_tag = format!("0x{:x}", self.fork_block);
        let addr_hex = format!("{:#x}", address);
        let slot_hex = format!("0x{:064x}", index);
        let res = self.rpc_call("eth_getStorageAt", json!([&addr_hex, &slot_hex, &block_tag]))?;
        Ok(parse_hex_u256(res.as_str().unwrap_or("0x0")))
    }
}

impl Database for ForkedDb {
    type Error = eyre::Error;

    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        // Check cache first
        if let Ok(acc) = self.cache.basic(address) {
            if let Some(info) = acc {
                return Ok(Some(info));
            }
        }
        // Fetch from RPC
        let info = self.fetch_account(address)?;
        // Store in cache
        let acc = self.cache.load_account(address)?;
        acc.info = info.clone();
        if let Some(ref code) = info.code {
            self.cache.contracts.insert(info.code_hash, code.clone());
        }
        Ok(Some(info))
    }

    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        self.cache.code_by_hash(code_hash).map_err(|_| eyre::eyre!("code not found"))
    }

    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Self::Error> {
        // Check cache
        if let Ok(acc) = self.cache.load_account(address) {
            if let Some(&val) = acc.storage.get(&index) {
                return Ok(val);
            }
        }
        // Fetch from RPC
        let val = self.fetch_storage(address, index)?;
        let acc = self.cache.load_account(address)?;
        acc.storage.insert(index, val);
        Ok(val)
    }

    fn block_hash(&mut self, number: u64) -> Result<B256, Self::Error> {
        let res = self.rpc_call("eth_getBlockByNumber", json!([format!("0x{:x}", number), false]))?;
        let hash_str = res["hash"].as_str().unwrap_or("0x0");
        Ok(parse_hex_b256(hash_str))
    }
}

fn parse_hex_u256(s: &str) -> U256 {
    let s = s.trim_start_matches("0x");
    U256::from_str_radix(s, 16).unwrap_or(U256::ZERO)
}

fn parse_hex_u64(s: &str) -> u64 {
    let s = s.trim_start_matches("0x");
    u64::from_str_radix(s, 16).unwrap_or(0)
}

fn parse_hex_b256(s: &str) -> B256 {
    let s = s.trim_start_matches("0x");
    let bytes = hex::decode(s).unwrap_or_default();
    if bytes.len() == 32 {
        B256::from_slice(&bytes)
    } else {
        B256::ZERO
    }
}
