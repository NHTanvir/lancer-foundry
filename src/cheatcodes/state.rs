use std::collections::HashMap;
use revm::primitives::{Address, B256, Bytes, Log, U256};
use crate::evm::types::EvmLog;
use crate::evm::inspector::ExpectedRevert;

#[derive(Debug, Clone, Default)]
pub struct PrankData {
    pub sender: Address,
    pub origin: Option<Address>,
    pub depth: u64,
}

#[derive(Debug, Clone)]
pub struct ExpectedEmit {
    pub check_topic1: bool,
    pub check_topic2: bool,
    pub check_topic3: bool,
    pub check_data: bool,
    pub address: Option<Address>,
    pub expected_topics: Vec<B256>,
    pub expected_data: Option<Bytes>,
    pub found: bool,
}

#[derive(Debug, Clone)]
pub struct MockResponse {
    pub success: bool,
    pub data: Bytes,
    pub times: Option<usize>,
    pub called: usize,
}

#[derive(Debug, Clone, Default)]
pub struct CheatcodeState {
    // Block / tx env overrides
    pub warped_timestamp: Option<u64>,
    pub rolled_block: Option<u64>,
    pub coinbase: Option<Address>,
    pub fee: Option<u128>,     // base fee
    pub chain_id_override: Option<u64>,
    pub tx_gas_price: Option<u128>,
    pub prevrandao: Option<B256>,

    // Prank
    pub active_prank: Option<PrankData>,
    pub start_prank: Option<PrankData>,

    // expectRevert
    pub expected_revert: Option<ExpectedRevert>,
    pub expect_revert_failed: bool,

    // expectEmit
    pub expected_emits: Vec<ExpectedEmit>,

    // expectCall
    pub expected_calls: HashMap<Address, Vec<ExpectedCallData>>,

    // mockCall: address → (selector-prefix → response)
    pub mocked_calls: HashMap<Address, Vec<(Bytes, MockResponse)>>,

    // recordLogs
    pub recording_logs: bool,
    pub recorded_logs: Vec<EvmLog>,

    // State snapshots: id → serialized db
    pub state_snapshots: HashMap<u64, Vec<u8>>,
    pub snapshot_counter: u64,

    // Gas metering
    pub gas_paused: bool,
    pub gas_checkpoint_name: Option<String>,
    pub gas_checkpoint_start: u64,

    // Labels
    pub labels: HashMap<Address, String>,

    // fs_permissions whitelist (paths)
    pub fs_read_paths: Vec<String>,
    pub fs_write_paths: Vec<String>,

    // broadcast transactions (forge script)
    pub broadcast_txs: Vec<BroadcastTx>,
    pub broadcasting: bool,
}

#[derive(Debug, Clone)]
pub struct BroadcastTx {
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Bytes,
    pub gas_limit: u64,
}

#[derive(Debug, Clone)]
pub struct ExpectedCallData {
    pub data_prefix: Bytes,
    pub count: Option<usize>,
    pub called: usize,
}

impl CheatcodeState {
    /// Check if a mock call matches and return (success, data).
    pub fn get_mock_call(&mut self, address: Address, calldata: &Bytes) -> Option<(bool, Bytes)> {
        let mocks = self.mocked_calls.get_mut(&address)?;
        for mock_entry in mocks.iter_mut() {
            let (prefix, resp) = mock_entry;
            if calldata.starts_with(prefix) {
                if let Some(n) = resp.times {
                    if resp.called >= n { continue; }
                }
                resp.called += 1;
                return Some((resp.success, resp.data.clone()));
            }
        }
        None
    }

    /// Check if an emitted log satisfies any pending expectEmit.
    pub fn check_expected_emit(&mut self, log: &Log) {
        let topics = log.data.topics();
        for expected in self.expected_emits.iter_mut() {
            if expected.found { continue; }
            if let Some(addr) = expected.address {
                if log.address != addr { continue; }
            }
            // Match topics
            let match_t = |i: usize, check: bool| {
                if !check { return true; }
                expected.expected_topics.get(i).map_or(false, |t| topics.get(i) == Some(t))
            };
            let t0 = !expected.check_topic1 || topics.first() == expected.expected_topics.first();
            let t1 = match_t(1, expected.check_topic2);
            let t2 = match_t(2, expected.check_topic3);
            let data_ok = if expected.check_data {
                expected.expected_data.as_deref().map_or(true, |d| log.data.data.as_ref() == d)
            } else { true };
            if t0 && t1 && t2 && data_ok {
                expected.found = true;
            }
        }
    }
}
