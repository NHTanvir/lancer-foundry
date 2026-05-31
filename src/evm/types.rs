use revm::primitives::{Address, Bytes, B256, U256};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct CallResult {
    pub success: bool,
    pub output: Bytes,
    pub gas_used: u64,
    pub logs: Vec<EvmLog>,
    pub revert_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvmLog {
    pub address: Address,
    pub topics: Vec<B256>,
    pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestStatus {
    Pass,
    Fail { reason: Option<String> },
    Skipped,
}

#[derive(Debug, Clone)]
pub struct TestResult {
    pub name: String,
    pub status: TestStatus,
    pub gas_used: u64,
    pub logs: Vec<EvmLog>,
    pub counterexample: Option<CounterExample>,
}

#[derive(Debug, Clone)]
pub struct CounterExample {
    pub args: Vec<String>,
    pub runs: usize,
}

#[derive(Debug, Clone)]
pub struct DeployResult {
    pub address: Address,
    pub gas_used: u64,
}

#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub balance: U256,
    pub nonce: u64,
    pub code: Option<Bytes>,
}
