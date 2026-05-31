use revm::primitives::{Address, Bytes, U256};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};

use crate::anvil::{backend::{eth_call, execute_transaction, RawTx}, state::SharedState};

fn addr_from_str(s: &str) -> eyre::Result<Address> {
    let s = s.trim_start_matches("0x");
    let b = hex::decode(format!("{:0>40}", s))?;
    Ok(Address::from_slice(&b))
}

fn u256_from_hex(s: &str) -> U256 {
    let s = s.trim_start_matches("0x");
    U256::from_str_radix(s, 16).unwrap_or(U256::ZERO)
}

fn hex_u64(v: u64) -> String { format!("0x{v:x}") }
fn hex_u128(v: u128) -> String { format!("0x{v:x}") }
fn hex_u256(v: U256) -> String { format!("{v:#x}") }
fn hex_bytes(b: &[u8]) -> String { format!("0x{}", hex::encode(b)) }

pub fn chain_id(state: &SharedState) -> eyre::Result<Value> {
    let s = state.read();
    Ok(json!(hex_u64(s.chain_id)))
}

pub fn block_number(state: &SharedState) -> eyre::Result<Value> {
    let s = state.read();
    Ok(json!(hex_u64(s.block_number)))
}

pub fn gas_price(state: &SharedState) -> eyre::Result<Value> {
    let s = state.read();
    Ok(json!(hex_u128(s.gas_price)))
}

pub fn accounts(state: &SharedState) -> eyre::Result<Value> {
    let s = state.read();
    let addrs: Vec<String> = s.accounts.iter().map(|a| format!("{:#x}", a.address)).collect();
    Ok(json!(addrs))
}

pub fn net_version(state: &SharedState) -> eyre::Result<Value> {
    let s = state.read();
    Ok(json!(s.chain_id.to_string()))
}

pub fn web3_sha3(params: &[Value]) -> eyre::Result<Value> {
    let data_hex = params.first().and_then(|v| v.as_str()).unwrap_or("0x");
    let data = hex::decode(data_hex.trim_start_matches("0x")).unwrap_or_default();
    let hash = Keccak256::digest(&data);
    Ok(json!(hex_bytes(&hash)))
}

pub fn get_balance(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let mut s = state.write();
    let balance = s.db.load_account(addr).map(|a| a.info.balance).unwrap_or(U256::ZERO);
    Ok(json!(hex_u256(balance)))
}

pub fn get_code(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let mut s = state.write();
    let code = s.db.load_account(addr)
        .ok()
        .and_then(|a| a.info.code.as_ref().map(|c| hex_bytes(c.bytes().as_ref())))
        .unwrap_or_else(|| "0x".into());
    Ok(json!(code))
}

pub fn get_storage_at(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let slot = u256_from_hex(params[1].as_str().unwrap_or("0x0"));
    let mut s = state.write();
    let val = s.db.load_account(addr)
        .ok()
        .and_then(|a| a.storage.get(&slot).copied())
        .unwrap_or(U256::ZERO);
    Ok(json!(format!("0x{:064x}", val)))
}

pub fn get_transaction_count(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let mut s = state.write();
    let nonce = s.db.load_account(addr).map(|a| a.info.nonce).unwrap_or(0);
    Ok(json!(hex_u64(nonce)))
}

pub fn call(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let tx = RawTx::from_json(&params[0])?;
    let mut s = state.write();
    let result = eth_call(&mut s, tx)?;
    Ok(json!(hex_bytes(&result)))
}

pub fn estimate_gas(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let tx = RawTx::from_json(&params[0])?;
    let mut s = state.write();
    let gas = crate::anvil::backend::estimate_gas(&mut s, tx)?;
    Ok(json!(hex_u64(gas)))
}

pub fn send_raw_transaction(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let raw_hex = params[0].as_str().unwrap_or("0x");
    let raw = hex::decode(raw_hex.trim_start_matches("0x"))?;
    // Decode RLP-encoded signed transaction
    let tx = decode_raw_transaction(&raw)?;
    let mut s = state.write();
    let hash = execute_transaction(&mut s, tx)?;
    Ok(json!(format!("{hash:#x}")))
}

pub fn send_transaction(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let tx = RawTx::from_json(&params[0])?;
    let from = tx.from;
    // Check if impersonated
    {
        let s = state.read();
        if !s.impersonated.contains(&from) {
            // Verify sender is a known dev account
            let is_dev = s.accounts.iter().any(|a| a.address == from);
            if !is_dev {
                eyre::bail!("sender not in accounts and not impersonated");
            }
        }
    }
    let mut s = state.write();
    let hash = execute_transaction(&mut s, tx)?;
    Ok(json!(format!("{hash:#x}")))
}

pub fn get_transaction_by_hash(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let hash_str = params[0].as_str().unwrap_or("0x");
    let hash_bytes = hex::decode(hash_str.trim_start_matches("0x"))?;
    if hash_bytes.len() != 32 { eyre::bail!("invalid hash"); }
    let hash = revm::primitives::B256::from_slice(&hash_bytes);
    let s = state.read();
    match s.transactions.get(&hash) {
        Some(tx) => Ok(json!({
            "hash": format!("{:#x}", tx.hash),
            "from": format!("{:#x}", tx.from),
            "to": tx.to.map(|a| format!("{a:#x}")),
            "value": hex_u256(tx.value),
            "input": hex_bytes(&tx.input),
            "gas": hex_u64(tx.gas_limit),
            "gasPrice": hex_u128(tx.gas_price),
            "nonce": hex_u64(tx.nonce),
            "blockNumber": hex_u64(tx.block_number),
            "blockHash": format!("{:#x}", tx.block_hash),
            "transactionIndex": hex_u64(tx.transaction_index),
            "type": "0x0",
        })),
        None => Ok(Value::Null),
    }
}

pub fn get_transaction_receipt(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let hash_str = params[0].as_str().unwrap_or("0x");
    let hash_bytes = hex::decode(hash_str.trim_start_matches("0x"))?;
    if hash_bytes.len() != 32 { eyre::bail!("invalid hash"); }
    let hash = revm::primitives::B256::from_slice(&hash_bytes);
    let s = state.read();
    match s.receipts.get(&hash) {
        Some(r) => {
            let logs: Vec<Value> = r.logs.iter().map(|l| json!({
                "address": format!("{:#x}", l.address),
                "topics": l.topics.iter().map(|t| format!("{t:#x}")).collect::<Vec<_>>(),
                "data": hex_bytes(&l.data),
                "blockNumber": hex_u64(l.block_number),
                "transactionHash": format!("{:#x}", l.transaction_hash),
                "logIndex": hex_u64(l.log_index),
            })).collect();
            Ok(json!({
                "transactionHash": format!("{:#x}", r.transaction_hash),
                "transactionIndex": hex_u64(r.transaction_index),
                "blockHash": format!("{:#x}", r.block_hash),
                "blockNumber": hex_u64(r.block_number),
                "from": format!("{:#x}", r.from),
                "to": r.to.map(|a| format!("{a:#x}")),
                "contractAddress": r.contract_address.map(|a| format!("{a:#x}")),
                "gasUsed": hex_u64(r.gas_used),
                "status": hex_u64(r.status as u64),
                "logs": logs,
                "logsBloom": format!("0x{}", "0".repeat(512)),
                "type": "0x0",
            }))
        }
        None => Ok(Value::Null),
    }
}

pub fn get_block_by_number(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let tag = params[0].as_str().unwrap_or("latest");
    let full = params.get(1).and_then(|v| v.as_bool()).unwrap_or(false);
    let s = state.read();
    let block = match tag {
        "latest" | "safe" | "finalized" => s.blocks.last(),
        "earliest" | "0x0" => s.blocks.first(),
        "pending" => s.blocks.last(),
        hex => {
            let n = u64::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(0);
            s.blocks.get(n as usize)
        }
    };
    Ok(block_to_json(block, &s, full))
}

pub fn get_block_by_hash(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let hash_str = params[0].as_str().unwrap_or("0x");
    let hash_bytes = hex::decode(hash_str.trim_start_matches("0x")).unwrap_or_default();
    let s = state.read();
    if hash_bytes.len() == 32 {
        let hash = revm::primitives::B256::from_slice(&hash_bytes);
        let block = s.blocks.iter().find(|b| b.hash == hash);
        Ok(block_to_json(block, &s, false))
    } else {
        Ok(Value::Null)
    }
}

pub fn get_logs(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let filter = &params[0];
    let s = state.read();

    let from_block = parse_block_tag(filter.get("fromBlock").and_then(|v| v.as_str()).unwrap_or("latest"), s.block_number);
    let to_block = parse_block_tag(filter.get("toBlock").and_then(|v| v.as_str()).unwrap_or("latest"), s.block_number);

    let addr_filter: Option<Address> = filter.get("address")
        .and_then(|v| v.as_str())
        .and_then(|s| addr_from_str(s).ok());

    let logs: Vec<Value> = s.logs.iter()
        .filter(|l| l.block_number >= from_block && l.block_number <= to_block)
        .filter(|l| addr_filter.map_or(true, |a| l.address == a))
        .map(|l| json!({
            "address": format!("{:#x}", l.address),
            "topics": l.topics.iter().map(|t| format!("{t:#x}")).collect::<Vec<_>>(),
            "data": hex_bytes(&l.data),
            "blockNumber": hex_u64(l.block_number),
            "transactionHash": format!("{:#x}", l.transaction_hash),
            "logIndex": hex_u64(l.log_index),
        }))
        .collect();

    Ok(json!(logs))
}

pub fn fee_history(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let s = state.read();
    Ok(json!({
        "oldestBlock": hex_u64(s.block_number),
        "baseFeePerGas": [hex_u128(s.base_fee)],
        "gasUsedRatio": [0.0],
        "reward": [[hex_u128(s.gas_price)]]
    }))
}

fn block_to_json(block: Option<&crate::anvil::state::StoredBlock>, s: &crate::anvil::state::AnvilState, full: bool) -> Value {
    match block {
        None => Value::Null,
        Some(b) => {
            let txs: Value = if full {
                json!(b.transactions.iter().filter_map(|h| s.transactions.get(h)).map(|tx| json!({
                    "hash": format!("{:#x}", tx.hash),
                    "from": format!("{:#x}", tx.from),
                    "to": tx.to.map(|a| format!("{a:#x}")),
                    "value": hex_u256(tx.value),
                    "input": hex_bytes(&tx.input),
                    "gas": hex_u64(tx.gas_limit),
                    "gasPrice": hex_u128(tx.gas_price),
                    "nonce": hex_u64(tx.nonce),
                    "blockNumber": hex_u64(tx.block_number),
                    "blockHash": format!("{:#x}", tx.block_hash),
                    "transactionIndex": hex_u64(tx.transaction_index),
                })).collect::<Vec<_>>())
            } else {
                json!(b.transactions.iter().map(|h| format!("{h:#x}")).collect::<Vec<_>>())
            };

            json!({
                "hash": format!("{:#x}", b.hash),
                "parentHash": format!("{:#x}", b.parent_hash),
                "number": hex_u64(b.number),
                "timestamp": hex_u64(b.timestamp),
                "gasLimit": hex_u64(b.gas_limit),
                "gasUsed": hex_u64(b.gas_used),
                "baseFeePerGas": hex_u128(b.base_fee),
                "transactions": txs,
                "miner": "0x0000000000000000000000000000000000000000",
                "difficulty": "0x0",
                "totalDifficulty": "0x0",
                "extraData": "0x",
                "logsBloom": format!("0x{}", "0".repeat(512)),
                "nonce": "0x0000000000000000",
                "receiptsRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "sha3Uncles": "0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
                "stateRoot": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "transactionsRoot": "0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421",
                "uncles": [],
                "size": "0x100",
            })
        }
    }
}

fn parse_block_tag(s: &str, current: u64) -> u64 {
    match s {
        "latest" | "safe" | "finalized" | "pending" => current,
        "earliest" => 0,
        hex => u64::from_str_radix(hex.trim_start_matches("0x"), 16).unwrap_or(current),
    }
}

/// Minimal RLP decoder for legacy transactions (type 0).
fn decode_raw_transaction(raw: &[u8]) -> eyre::Result<RawTx> {
    // Very simplified: just treat it as a sendTransaction from first dev account
    // A real implementation would RLP-decode and recover the signer via ecrecover
    // For now, extract what we can
    eyre::bail!("eth_sendRawTransaction: full RLP decoding not implemented; use eth_sendTransaction for dev testing")
}
