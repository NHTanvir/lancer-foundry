use std::collections::{HashMap, HashSet};
use parking_lot::RwLock;
use revm::{
    db::InMemoryDB,
    primitives::{Address, B256, Bytes, U256},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::anvil::accounts::DevAccount;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTransaction {
    pub hash: B256,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub input: Bytes,
    pub gas_limit: u64,
    pub gas_price: u128,
    pub nonce: u64,
    pub block_number: u64,
    pub block_hash: B256,
    pub transaction_index: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredReceipt {
    pub transaction_hash: B256,
    pub transaction_index: u64,
    pub block_hash: B256,
    pub block_number: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub contract_address: Option<Address>,
    pub gas_used: u64,
    pub status: u8,
    pub logs: Vec<StoredLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredLog {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
    pub block_number: u64,
    pub block_hash: B256,
    pub transaction_hash: B256,
    pub log_index: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredBlock {
    pub hash: B256,
    pub parent_hash: B256,
    pub number: u64,
    pub timestamp: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub base_fee: u128,
    pub transactions: Vec<B256>,
}

pub struct AnvilState {
    pub db: InMemoryDB,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub chain_id: u64,
    pub gas_price: u128,
    pub base_fee: u128,
    pub block_gas_limit: u64,
    pub accounts: Vec<DevAccount>,
    pub impersonated: HashSet<Address>,
    pub transactions: HashMap<B256, StoredTransaction>,
    pub receipts: HashMap<B256, StoredReceipt>,
    pub blocks: Vec<StoredBlock>,
    pub logs: Vec<StoredLog>,
    pub auto_mine: bool,
    pub block_time: Option<u64>,
    pub fork_url: Option<String>,
    pub fork_block: Option<u64>,
    pub next_block_timestamp: Option<u64>,
}

pub type SharedState = Arc<RwLock<AnvilState>>;

impl AnvilState {
    pub fn new(
        chain_id: u64,
        accounts: Vec<DevAccount>,
        gas_price: u128,
        block_gas_limit: u64,
        block_time: Option<u64>,
        fork_url: Option<String>,
        fork_block: Option<u64>,
    ) -> Self {
        let mut db = InMemoryDB::default();

        // Fund dev accounts
        for acc in &accounts {
            let a = db.load_account(acc.address).unwrap();
            a.info.balance = acc.balance;
            a.info.nonce = 0;
        }

        let genesis_block = StoredBlock {
            hash: B256::ZERO,
            parent_hash: B256::ZERO,
            number: 0,
            timestamp: 1,
            gas_limit: block_gas_limit,
            gas_used: 0,
            base_fee: 0,
            transactions: vec![],
        };

        AnvilState {
            db,
            block_number: 0,
            block_timestamp: 1,
            chain_id,
            gas_price,
            base_fee: 0,
            block_gas_limit,
            accounts,
            impersonated: HashSet::new(),
            transactions: HashMap::new(),
            receipts: HashMap::new(),
            blocks: vec![genesis_block],
            logs: vec![],
            auto_mine: block_time.is_none(),
            block_time,
            fork_url,
            fork_block,
            next_block_timestamp: None,
        }
    }

    pub fn latest_block(&self) -> Option<&StoredBlock> {
        self.blocks.last()
    }

    pub fn mine_block(&mut self, txs: Vec<B256>) {
        let parent_hash = self.blocks.last().map(|b| b.hash).unwrap_or(B256::ZERO);
        self.block_number += 1;
        let ts = self.next_block_timestamp.take().unwrap_or(self.block_timestamp + 12);
        self.block_timestamp = ts;

        // Simple block hash: keccak256(number || timestamp)
        let mut hash_input = [0u8; 16];
        hash_input[..8].copy_from_slice(&self.block_number.to_be_bytes());
        hash_input[8..].copy_from_slice(&ts.to_be_bytes());
        use sha3::{Digest, Keccak256};
        let hash = B256::from_slice(&Keccak256::digest(&hash_input));

        // Update receipts with block info
        let gas_used: u64 = txs.iter()
            .filter_map(|h| self.receipts.get(h))
            .map(|r| r.gas_used)
            .sum();

        let block = StoredBlock {
            hash,
            parent_hash,
            number: self.block_number,
            timestamp: ts,
            gas_limit: self.block_gas_limit,
            gas_used,
            base_fee: self.base_fee,
            transactions: txs.clone(),
        };

        // Update tx/receipt block info
        for tx_hash in &txs {
            if let Some(tx) = self.transactions.get_mut(tx_hash) {
                tx.block_number = self.block_number;
                tx.block_hash = hash;
            }
            if let Some(receipt) = self.receipts.get_mut(tx_hash) {
                receipt.block_number = self.block_number;
                receipt.block_hash = hash;
                for log in &mut receipt.logs {
                    log.block_number = self.block_number;
                    log.block_hash = hash;
                }
            }
        }

        self.blocks.push(block);
    }
}
