use sha3::{Digest, Keccak256};

/// Compute 4-byte selector from function signature string.
pub fn selector(sig: &str) -> [u8; 4] {
    let hash = Keccak256::digest(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

// ── Block / Tx env ──────────────────────────────────────────────────────────
pub fn warp()           -> [u8; 4] { selector("warp(uint256)") }
pub fn roll()           -> [u8; 4] { selector("roll(uint256)") }
pub fn fee()            -> [u8; 4] { selector("fee(uint256)") }
pub fn coinbase()       -> [u8; 4] { selector("coinbase(address)") }
pub fn chain_id()       -> [u8; 4] { selector("chainId(uint256)") }
pub fn prevrandao()     -> [u8; 4] { selector("prevrandao(bytes32)") }
pub fn tx_gas_price()   -> [u8; 4] { selector("txGasPrice(uint256)") }
pub fn get_block_number()   -> [u8; 4] { selector("getBlockNumber()") }
pub fn get_block_timestamp() -> [u8; 4] { selector("getBlockTimestamp()") }
pub fn get_chain_id()   -> [u8; 4] { selector("getChainId()") }

// ── Account state ───────────────────────────────────────────────────────────
pub fn deal()           -> [u8; 4] { selector("deal(address,uint256)") }
pub fn etch()           -> [u8; 4] { selector("etch(address,bytes)") }
pub fn store()          -> [u8; 4] { selector("store(address,bytes32,bytes32)") }
pub fn load()           -> [u8; 4] { selector("load(address,bytes32)") }
pub fn set_nonce()      -> [u8; 4] { selector("setNonce(address,uint64)") }
pub fn get_nonce()      -> [u8; 4] { selector("getNonce(address)") }
pub fn addr()           -> [u8; 4] { selector("addr(uint256)") }
pub fn get_code()       -> [u8; 4] { selector("getCode(string)") }
pub fn get_deployed_code() -> [u8; 4] { selector("getDeployedCode(string)") }

// ── Prank / caller ──────────────────────────────────────────────────────────
pub fn prank_sender()   -> [u8; 4] { selector("prank(address)") }
pub fn prank_both()     -> [u8; 4] { selector("prank(address,address)") }
pub fn start_prank_sender() -> [u8; 4] { selector("startPrank(address)") }
pub fn start_prank_both() -> [u8; 4] { selector("startPrank(address,address)") }
pub fn stop_prank()     -> [u8; 4] { selector("stopPrank()") }

// ── Mock ────────────────────────────────────────────────────────────────────
pub fn mock_call()      -> [u8; 4] { selector("mockCall(address,bytes,bytes)") }
pub fn mock_call_with_value() -> [u8; 4] { selector("mockCall(address,uint256,bytes,bytes)") }
pub fn mock_call_revert() -> [u8; 4] { selector("mockCallRevert(address,bytes,bytes)") }
pub fn clear_mocked_calls() -> [u8; 4] { selector("clearMockedCalls()") }

// ── Expect / assert ─────────────────────────────────────────────────────────
pub fn expect_revert0() -> [u8; 4] { selector("expectRevert()") }
pub fn expect_revert_bytes() -> [u8; 4] { selector("expectRevert(bytes)") }
pub fn expect_revert_str() -> [u8; 4] { selector("expectRevert(bytes4)") }
pub fn expect_emit4()   -> [u8; 4] { selector("expectEmit(bool,bool,bool,bool)") }
pub fn expect_emit5()   -> [u8; 4] { selector("expectEmit(bool,bool,bool,bool,address)") }
pub fn expect_emit0()   -> [u8; 4] { selector("expectEmit()") }
pub fn expect_call()    -> [u8; 4] { selector("expectCall(address,bytes)") }
pub fn assume()         -> [u8; 4] { selector("assume(bool)") }
pub fn skip_cheat()     -> [u8; 4] { selector("skip(bool)") }

// ── Recording ───────────────────────────────────────────────────────────────
pub fn record_logs()        -> [u8; 4] { selector("recordLogs()") }
pub fn get_recorded_logs()  -> [u8; 4] { selector("getRecordedLogs()") }

// ── Snapshots ───────────────────────────────────────────────────────────────
pub fn snapshot_state()         -> [u8; 4] { selector("snapshotState()") }
pub fn revert_to_state()        -> [u8; 4] { selector("revertToState(uint256)") }
pub fn revert_to_state_delete() -> [u8; 4] { selector("revertToStateAndDelete(uint256)") }

// ── Gas ─────────────────────────────────────────────────────────────────────
pub fn pause_gas_metering()     -> [u8; 4] { selector("pauseGasMetering()") }
pub fn resume_gas_metering()    -> [u8; 4] { selector("resumeGasMetering()") }
pub fn start_snapshot_gas()     -> [u8; 4] { selector("startSnapshotGas(string)") }
pub fn stop_snapshot_gas()      -> [u8; 4] { selector("stopSnapshotGas(string)") }

// ── Env vars ────────────────────────────────────────────────────────────────
pub fn env_bool()       -> [u8; 4] { selector("envBool(string)") }
pub fn env_uint()       -> [u8; 4] { selector("envUint(string)") }
pub fn env_int()        -> [u8; 4] { selector("envInt(string)") }
pub fn env_address()    -> [u8; 4] { selector("envAddress(string)") }
pub fn env_bytes32()    -> [u8; 4] { selector("envBytes32(string)") }
pub fn env_string()     -> [u8; 4] { selector("envString(string)") }
pub fn env_bytes()      -> [u8; 4] { selector("envBytes(string)") }
pub fn set_env()        -> [u8; 4] { selector("setEnv(string,string)") }

// ── Filesystem ──────────────────────────────────────────────────────────────
pub fn read_file()      -> [u8; 4] { selector("readFile(string)") }
pub fn write_file()     -> [u8; 4] { selector("writeFile(string,string)") }
pub fn read_line()      -> [u8; 4] { selector("readLine(string)") }
pub fn exists()         -> [u8; 4] { selector("exists(string)") }

// ── Utility ─────────────────────────────────────────────────────────────────
pub fn label()          -> [u8; 4] { selector("label(address,string)") }
pub fn get_label()      -> [u8; 4] { selector("getLabel(address)") }
pub fn ffi()            -> [u8; 4] { selector("ffi(string[])") }
pub fn project_root()   -> [u8; 4] { selector("projectRoot()") }
pub fn random_uint()    -> [u8; 4] { selector("randomUint()") }
pub fn random_address() -> [u8; 4] { selector("randomAddress()") }

// ── Signing ─────────────────────────────────────────────────────────────────
pub fn sign_msg()       -> [u8; 4] { selector("sign(uint256,bytes32)") }
pub fn derive_key()     -> [u8; 4] { selector("deriveKey(string,uint32)") }
