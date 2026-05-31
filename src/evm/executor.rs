use std::sync::{Arc, Mutex};

use revm::{
    db::InMemoryDB,
    primitives::{
        AccountInfo, Address, Bytecode, Bytes, ExecutionResult, Output,
        SpecId, TransactTo, U256, KECCAK_EMPTY,
    },
    Evm,
};

use crate::cheatcodes::state::CheatcodeState;
use crate::evm::inspector::CheatcodeInspector;
use crate::evm::types::{CallResult, DeployResult, EvmLog};
use crate::evm::fork::ForkedDb;

/// Which database backend to use.
pub enum DbKind {
    Memory(InMemoryDB),
    Forked(Box<ForkedDb>),
}

impl DbKind {
    pub fn as_memory_mut(&mut self) -> Option<&mut InMemoryDB> {
        match self {
            DbKind::Memory(db) => Some(db),
            _ => None,
        }
    }
}

/// Central EVM executor. Wraps revm with our cheatcode inspector.
pub struct Executor {
    pub db: InMemoryDB,
    pub cheatcode_state: Arc<Mutex<CheatcodeState>>,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub chain_id: u64,
    pub block_coinbase: Address,
    pub gas_limit: u64,
    pub gas_price: u128,
    pub base_fee: u128,
    pub spec_id: SpecId,
}

impl Default for Executor {
    fn default() -> Self {
        Executor::new(1, 0)
    }
}

impl Executor {
    pub fn new(chain_id: u64, block_number: u64) -> Self {
        Executor {
            db: InMemoryDB::default(),
            cheatcode_state: Arc::new(Mutex::new(CheatcodeState::default())),
            block_number,
            block_timestamp: 1,
            chain_id,
            block_coinbase: Address::ZERO,
            gas_limit: 30_000_000,
            gas_price: 1_000_000_000,
            base_fee: 0,
            spec_id: SpecId::CANCUN,
        }
    }

    /// Set account balance.
    pub fn set_balance(&mut self, address: Address, balance: U256) {
        let acc = self.db.load_account(address).unwrap();
        acc.info.balance = balance;
    }

    /// Set account nonce.
    pub fn set_nonce(&mut self, address: Address, nonce: u64) {
        let acc = self.db.load_account(address).unwrap();
        acc.info.nonce = nonce;
    }

    /// Deploy bytecode at address (etch).
    pub fn set_code(&mut self, address: Address, code: Bytes) {
        let bytecode = Bytecode::new_raw(code);
        let code_hash = bytecode.hash_slow();
        let acc = self.db.load_account(address).unwrap();
        acc.info.code_hash = code_hash;
        acc.info.code = Some(bytecode.clone());
        self.db.contracts.insert(code_hash, bytecode);
    }

    /// Write storage slot.
    pub fn set_storage(&mut self, address: Address, slot: U256, value: U256) {
        let acc = self.db.load_account(address).unwrap();
        acc.storage.insert(slot, value);
    }

    /// Read storage slot.
    pub fn get_storage(&mut self, address: Address, slot: U256) -> U256 {
        let acc = self.db.load_account(address).unwrap();
        acc.storage.get(&slot).copied().unwrap_or(U256::ZERO)
    }

    /// Get balance.
    pub fn get_balance(&mut self, address: Address) -> U256 {
        self.db.load_account(address).map(|a| a.info.balance).unwrap_or(U256::ZERO)
    }

    /// Get nonce.
    pub fn get_nonce(&mut self, address: Address) -> u64 {
        self.db.load_account(address).map(|a| a.info.nonce).unwrap_or(0)
    }

    /// Get code.
    pub fn get_code(&mut self, address: Address) -> Bytes {
        let acc = self.db.load_account(address).unwrap();
        acc.info.code.as_ref().map(|c| c.bytes().clone()).unwrap_or_default()
    }

    /// Deploy a contract (CREATE).
    pub fn deploy(
        &mut self,
        from: Address,
        init_code: Bytes,
        value: U256,
    ) -> eyre::Result<DeployResult> {
        let result = self.execute_tx(from, None, value, init_code)?;
        if !result.success {
            let reason = result.revert_reason.unwrap_or_else(|| "deployment reverted".into());
            eyre::bail!("deployment failed: {reason}");
        }
        // Extract deployed address from logs/output – simplified: use CREATE address derivation
        let nonce = self.get_nonce(from).saturating_sub(1);
        let deployed = compute_create_address(from, nonce);
        Ok(DeployResult { address: deployed, gas_used: result.gas_used })
    }

    /// Execute a raw CALL or CREATE.
    pub fn execute_tx(
        &mut self,
        from: Address,
        to: Option<Address>,
        value: U256,
        data: Bytes,
    ) -> eyre::Result<CallResult> {
        self.execute_tx_with_gas(from, to, value, data, self.gas_limit)
    }

    pub fn execute_tx_with_gas(
        &mut self,
        from: Address,
        to: Option<Address>,
        value: U256,
        data: Bytes,
        gas_limit: u64,
    ) -> eyre::Result<CallResult> {
        let mut inspector = CheatcodeInspector::new(self.cheatcode_state.clone());

        let chain_id = self.chain_id;
        let block_number = self.block_number;
        let block_timestamp = self.block_timestamp;
        let block_coinbase = self.block_coinbase;
        let base_fee = U256::from(self.base_fee);
        let gas_price = U256::from(self.gas_price);
        let spec_id = self.spec_id;

        let transact_to = match to {
            Some(addr) => TransactTo::Call(addr),
            None => TransactTo::Create,
        };

        // Temporarily take the db
        let mut db = std::mem::take(&mut self.db);

        let result = {
            let mut evm = Evm::builder()
                .with_db(&mut db)
                .with_external_context(&mut inspector)
                .append_handler_register(revm::inspector_handle_register)
                .modify_cfg_env(|c| {
                    c.chain_id = chain_id;
                })
                .modify_block_env(|b| {
                    b.number = U256::from(block_number);
                    b.timestamp = U256::from(block_timestamp);
                    b.coinbase = block_coinbase;
                    b.gas_limit = U256::from(self.gas_limit);
                    b.basefee = base_fee;
                })
                .modify_tx_env(|tx| {
                    tx.caller = from;
                    tx.transact_to = transact_to;
                    tx.value = value;
                    tx.data = data.clone();
                    tx.gas_limit = gas_limit;
                    tx.gas_price = gas_price;
                    tx.chain_id = Some(chain_id);
                    tx.nonce = Some(self.get_nonce(from));
                })
                .build();

            evm.transact_commit()
        };

        self.db = db;

        // Apply any pending cheatcode state changes to block/tx env
        {
            let cs = self.cheatcode_state.lock().unwrap();
            if let Some(ts) = cs.warped_timestamp { self.block_timestamp = ts; }
            if let Some(bn) = cs.rolled_block { self.block_number = bn; }
            if let Some(cb) = cs.coinbase { self.block_coinbase = cb; }
        }

        let exec_result = result.map_err(|e| eyre::eyre!("EVM error: {:?}", e))?;

        collect_result(exec_result)
    }

    /// Snapshot current DB state, return snapshot id.
    pub fn snapshot(&self) -> Vec<u8> {
        // Serialize db state to bytes (simplified: use bincode/json)
        serde_json::to_vec(&self.db).unwrap_or_default()
    }

    /// Revert to a previously taken snapshot.
    pub fn revert_to_snapshot(&mut self, snapshot: &[u8]) {
        if let Ok(db) = serde_json::from_slice(snapshot) {
            self.db = db;
        }
    }
}

fn collect_result(result: ExecutionResult) -> eyre::Result<CallResult> {
    match result {
        ExecutionResult::Success { output, gas_used, logs, .. } => {
            let output_bytes = match output {
                Output::Call(b) => b,
                Output::Create(b, _) => b,
            };
            let evm_logs = logs.into_iter().map(|l| EvmLog {
                address: l.address,
                topics: l.data.topics().to_vec(),
                data: l.data.data.clone(),
            }).collect();
            Ok(CallResult { success: true, output: output_bytes, gas_used, logs: evm_logs, revert_reason: None })
        }
        ExecutionResult::Revert { output, gas_used } => {
            let reason = decode_revert_reason(&output);
            Ok(CallResult { success: false, output, gas_used, logs: vec![], revert_reason: Some(reason) })
        }
        ExecutionResult::Halt { reason, gas_used } => {
            Ok(CallResult {
                success: false,
                output: Bytes::new(),
                gas_used,
                logs: vec![],
                revert_reason: Some(format!("{:?}", reason)),
            })
        }
    }
}

pub fn decode_revert_reason(data: &Bytes) -> String {
    // Error(string) selector = 0x08c379a0
    if data.len() >= 4 && &data[..4] == b"\x08\xc3\x79\xa0" {
        if let Ok(s) = decode_abi_string(&data[4..]) {
            return s;
        }
    }
    if data.is_empty() {
        return "reverted without reason".into();
    }
    format!("0x{}", hex::encode(&**data))
}

fn compute_create_address(from: Address, nonce: u64) -> Address {
    use sha3::{Digest, Keccak256};
    let mut buf = Vec::with_capacity(23);
    buf.push(0x94u8); // 0x80 + 20
    buf.extend_from_slice(from.as_slice());
    if nonce == 0 {
        buf.push(0x80);
    } else if nonce < 0x80 {
        buf.push(nonce as u8);
    } else {
        let be = nonce.to_be_bytes();
        let start = be.iter().position(|&x| x != 0).unwrap_or(7);
        let trimmed = &be[start..];
        buf.push(0x80 + trimmed.len() as u8);
        buf.extend_from_slice(trimmed);
    }
    let list_len = buf.len();
    let mut full = Vec::with_capacity(1 + list_len);
    full.push(0xc0 + list_len as u8);
    full.extend_from_slice(&buf);
    let hash = Keccak256::digest(&full);
    Address::from_slice(&hash[12..])
}

fn decode_abi_string(data: &[u8]) -> eyre::Result<String> {
    // ABI-encoded string: offset (32) + length (32) + data
    if data.len() < 64 { eyre::bail!("too short"); }
    let offset = U256::from_be_slice(&data[..32]).to::<usize>();
    if offset + 32 > data.len() { eyre::bail!("bad offset"); }
    let len = U256::from_be_slice(&data[offset..offset + 32]).to::<usize>();
    if offset + 32 + len > data.len() { eyre::bail!("bad length"); }
    let s = std::str::from_utf8(&data[offset + 32..offset + 32 + len])?;
    Ok(s.to_string())
}
