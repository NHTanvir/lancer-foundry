use revm::{
    interpreter::{CallOutcome, Gas, InstructionResult, InterpreterResult},
    primitives::{Address, Bytes, U256, B256},
    EvmContext, Database,
};

use crate::cheatcodes::{selectors as sel, state::CheatcodeState};
use crate::evm::inspector::ExpectedRevert;

/// Dispatch a cheatcode call and return the CallOutcome to inject back.
pub fn apply_cheatcode<DB: Database>(
    selector: [u8; 4],
    args: &[u8],
    state: &mut CheatcodeState,
    context: &mut EvmContext<DB>,
    _caller: Address,
) -> CallOutcome {
    let result = dispatch_inner(selector, args, state, context);
    match result {
        Ok(ret_data) => make_success(ret_data, 100_000),
        Err(e) => make_revert(e.to_string(), 100_000),
    }
}

fn dispatch_inner<DB: Database>(
    selector: [u8; 4],
    args: &[u8],
    state: &mut CheatcodeState,
    context: &mut EvmContext<DB>,
) -> eyre::Result<Vec<u8>> {
    // ── Block / Tx env ──────────────────────────────────────────────────────
    if selector == sel::warp() {
        let ts = abi_decode_u256(args, 0)?;
        state.warped_timestamp = Some(ts.to::<u64>());
        context.env.block.timestamp = ts;
        return Ok(vec![]);
    }
    if selector == sel::roll() {
        let bn = abi_decode_u256(args, 0)?;
        state.rolled_block = Some(bn.to::<u64>());
        context.env.block.number = bn;
        return Ok(vec![]);
    }
    if selector == sel::fee() {
        let f = abi_decode_u256(args, 0)?;
        state.fee = Some(f.to::<u128>());
        context.env.block.basefee = f;
        return Ok(vec![]);
    }
    if selector == sel::coinbase() {
        let addr = abi_decode_address(args, 0)?;
        state.coinbase = Some(addr);
        context.env.block.coinbase = addr;
        return Ok(vec![]);
    }
    if selector == sel::chain_id() {
        let cid = abi_decode_u256(args, 0)?;
        state.chain_id_override = Some(cid.to::<u64>());
        context.env.cfg.chain_id = cid.to::<u64>();
        return Ok(vec![]);
    }
    if selector == sel::prevrandao() {
        let val = abi_decode_b256(args, 0)?;
        state.prevrandao = Some(val);
        context.env.block.prevrandao = Some(val);
        return Ok(vec![]);
    }
    if selector == sel::tx_gas_price() {
        let gp = abi_decode_u256(args, 0)?;
        state.tx_gas_price = Some(gp.to::<u128>());
        context.env.tx.gas_price = gp;
        return Ok(vec![]);
    }
    if selector == sel::get_block_number() {
        return Ok(abi_encode_u256(context.env.block.number));
    }
    if selector == sel::get_block_timestamp() {
        return Ok(abi_encode_u256(context.env.block.timestamp));
    }
    if selector == sel::get_chain_id() {
        return Ok(abi_encode_u256(U256::from(context.env.cfg.chain_id)));
    }

    // ── Account state ───────────────────────────────────────────────────────
    if selector == sel::deal() {
        let addr = abi_decode_address(args, 0)?;
        let amount = abi_decode_u256(args, 32)?;
        cheatcode_load_account(context, addr, |acc| { acc.info.balance = amount; });
        return Ok(vec![]);
    }
    if selector == sel::etch() {
        let addr = abi_decode_address(args, 0)?;
        let code = abi_decode_bytes_dynamic(args, 32)?;
        let bytecode = revm::primitives::Bytecode::new_raw(Bytes::copy_from_slice(&code));
        let hash = bytecode.hash_slow();
        cheatcode_load_account(context, addr, |acc| {
            acc.info.code_hash = hash;
            acc.info.code = Some(bytecode.clone());
        });
        return Ok(vec![]);
    }
    if selector == sel::store() {
        let addr = abi_decode_address(args, 0)?;
        let slot = abi_decode_u256(args, 32)?;
        let value = abi_decode_u256(args, 64)?;
        cheatcode_load_account(context, addr, |acc| {
            acc.storage.insert(slot, revm::primitives::EvmStorageSlot::new_changed(U256::ZERO, value));
        });
        return Ok(vec![]);
    }
    if selector == sel::load() {
        let addr = abi_decode_address(args, 0)?;
        let slot = abi_decode_u256(args, 32)?;
        let val = context.journaled_state.state
            .get(&addr)
            .and_then(|acc| acc.storage.get(&slot))
            .map(|s| s.present_value)
            .unwrap_or(U256::ZERO);
        return Ok(abi_encode_u256(val));
    }
    if selector == sel::set_nonce() {
        let addr = abi_decode_address(args, 0)?;
        let nonce = abi_decode_u64(args, 32)?;
        cheatcode_load_account(context, addr, |acc| { acc.info.nonce = nonce; });
        return Ok(vec![]);
    }
    if selector == sel::get_nonce() {
        let addr = abi_decode_address(args, 0)?;
        let nonce = context.journaled_state.state
            .get(&addr)
            .map(|acc| acc.info.nonce)
            .unwrap_or(0);
        return Ok(abi_encode_u256(U256::from(nonce)));
    }
    if selector == sel::addr() {
        let private_key = abi_decode_u256(args, 0)?;
        let addr = derive_address_from_key(private_key)?;
        return Ok(abi_encode_address(addr));
    }

    // ── Prank / caller ──────────────────────────────────────────────────────
    if selector == sel::prank_sender() {
        let sender = abi_decode_address(args, 0)?;
        state.active_prank = Some(crate::cheatcodes::state::PrankData {
            sender,
            origin: None,
            depth: 0,
        });
        return Ok(vec![]);
    }
    if selector == sel::prank_both() {
        let sender = abi_decode_address(args, 0)?;
        let origin = abi_decode_address(args, 32)?;
        state.active_prank = Some(crate::cheatcodes::state::PrankData {
            sender,
            origin: Some(origin),
            depth: 0,
        });
        return Ok(vec![]);
    }
    if selector == sel::start_prank_sender() {
        let sender = abi_decode_address(args, 0)?;
        let prank = crate::cheatcodes::state::PrankData { sender, origin: None, depth: 0 };
        state.active_prank = Some(prank.clone());
        state.start_prank = Some(prank);
        return Ok(vec![]);
    }
    if selector == sel::start_prank_both() {
        let sender = abi_decode_address(args, 0)?;
        let origin = abi_decode_address(args, 32)?;
        let prank = crate::cheatcodes::state::PrankData { sender, origin: Some(origin), depth: 0 };
        state.active_prank = Some(prank.clone());
        state.start_prank = Some(prank);
        return Ok(vec![]);
    }
    if selector == sel::stop_prank() {
        state.active_prank = None;
        state.start_prank = None;
        return Ok(vec![]);
    }

    // ── Mock calls ──────────────────────────────────────────────────────────
    if selector == sel::mock_call() {
        let addr = abi_decode_address(args, 0)?;
        let data = abi_decode_bytes_dynamic(args, 32)?;
        let ret_data = abi_decode_bytes_dynamic(args, 64)?;
        let mocks = state.mocked_calls.entry(addr).or_default();
        mocks.push((
            Bytes::copy_from_slice(&data),
            crate::cheatcodes::state::MockResponse {
                success: true,
                data: Bytes::copy_from_slice(&ret_data),
                times: None,
                called: 0,
            },
        ));
        return Ok(vec![]);
    }
    if selector == sel::mock_call_revert() {
        let addr = abi_decode_address(args, 0)?;
        let data = abi_decode_bytes_dynamic(args, 32)?;
        let revert_data = abi_decode_bytes_dynamic(args, 64)?;
        let mocks = state.mocked_calls.entry(addr).or_default();
        mocks.push((
            Bytes::copy_from_slice(&data),
            crate::cheatcodes::state::MockResponse {
                success: false,
                data: Bytes::copy_from_slice(&revert_data),
                times: None,
                called: 0,
            },
        ));
        return Ok(vec![]);
    }
    if selector == sel::clear_mocked_calls() {
        state.mocked_calls.clear();
        return Ok(vec![]);
    }

    // ── expectRevert ────────────────────────────────────────────────────────
    if selector == sel::expect_revert0() {
        state.expected_revert = Some(ExpectedRevert::Any);
        return Ok(vec![]);
    }
    if selector == sel::expect_revert_bytes() {
        let data = abi_decode_bytes_dynamic(args, 0)?;
        state.expected_revert = Some(ExpectedRevert::Bytes(data));
        return Ok(vec![]);
    }
    if selector == sel::expect_revert_str() {
        if args.len() >= 4 {
            let custom_selector: [u8; 4] = args[..4].try_into().unwrap();
            state.expected_revert = Some(ExpectedRevert::Custom(custom_selector));
        }
        return Ok(vec![]);
    }

    // ── expectEmit ──────────────────────────────────────────────────────────
    if selector == sel::expect_emit4() {
        let t1 = abi_decode_bool(args, 0)?;
        let t2 = abi_decode_bool(args, 32)?;
        let t3 = abi_decode_bool(args, 64)?;
        let data = abi_decode_bool(args, 96)?;
        state.expected_emits.push(crate::cheatcodes::state::ExpectedEmit {
            check_topic1: t1, check_topic2: t2, check_topic3: t3, check_data: data,
            address: None, expected_topics: vec![], expected_data: None, found: false,
        });
        return Ok(vec![]);
    }
    if selector == sel::expect_emit5() {
        let t1 = abi_decode_bool(args, 0)?;
        let t2 = abi_decode_bool(args, 32)?;
        let t3 = abi_decode_bool(args, 64)?;
        let data = abi_decode_bool(args, 96)?;
        let addr = abi_decode_address(args, 128)?;
        state.expected_emits.push(crate::cheatcodes::state::ExpectedEmit {
            check_topic1: t1, check_topic2: t2, check_topic3: t3, check_data: data,
            address: Some(addr), expected_topics: vec![], expected_data: None, found: false,
        });
        return Ok(vec![]);
    }
    if selector == sel::expect_emit0() {
        state.expected_emits.push(crate::cheatcodes::state::ExpectedEmit {
            check_topic1: true, check_topic2: true, check_topic3: true, check_data: true,
            address: None, expected_topics: vec![], expected_data: None, found: false,
        });
        return Ok(vec![]);
    }

    // ── assume / skip ───────────────────────────────────────────────────────
    if selector == sel::assume() {
        let ok = abi_decode_bool(args, 0)?;
        if !ok { eyre::bail!("vm.assume: condition false — discard this input"); }
        return Ok(vec![]);
    }
    if selector == sel::skip_cheat() {
        let skip = abi_decode_bool(args, 0)?;
        if skip { eyre::bail!("__skip__"); }
        return Ok(vec![]);
    }

    // ── recordLogs / getRecordedLogs ────────────────────────────────────────
    if selector == sel::record_logs() {
        state.recording_logs = true;
        state.recorded_logs.clear();
        return Ok(vec![]);
    }
    if selector == sel::get_recorded_logs() {
        // Returns (address[] memory, bytes32[][] memory topics, bytes[] memory data)
        // Simplified: just return count in first slot
        let count = state.recorded_logs.len();
        return Ok(abi_encode_u256(U256::from(count)));
    }

    // ── State snapshots ─────────────────────────────────────────────────────
    if selector == sel::snapshot_state() {
        // Can't snapshot inside dispatch without access to full db; signal to runner
        let id = state.snapshot_counter;
        state.snapshot_counter += 1;
        return Ok(abi_encode_u256(U256::from(id)));
    }
    if selector == sel::revert_to_state() || selector == sel::revert_to_state_delete() {
        // Signal to runner to restore snapshot
        let id = abi_decode_u256(args, 0)?;
        return Ok(abi_encode_u256(id));  // runner interprets this
    }

    // ── Gas metering ────────────────────────────────────────────────────────
    if selector == sel::pause_gas_metering() {
        state.gas_paused = true;
        return Ok(vec![]);
    }
    if selector == sel::resume_gas_metering() {
        state.gas_paused = false;
        return Ok(vec![]);
    }

    // ── Env vars ────────────────────────────────────────────────────────────
    if selector == sel::set_env() {
        let key = abi_decode_string_dynamic(args, 0)?;
        let val = abi_decode_string_dynamic(args, 32)?;
        std::env::set_var(&key, &val);
        return Ok(vec![]);
    }
    if selector == sel::env_string() {
        let key = abi_decode_string_dynamic(args, 0)?;
        let val = std::env::var(&key).unwrap_or_default();
        return Ok(abi_encode_string(&val));
    }
    if selector == sel::env_bool() {
        let key = abi_decode_string_dynamic(args, 0)?;
        let val = std::env::var(&key).unwrap_or_default();
        let b = matches!(val.to_lowercase().as_str(), "true" | "1" | "yes");
        return Ok(abi_encode_bool(b));
    }
    if selector == sel::env_uint() {
        let key = abi_decode_string_dynamic(args, 0)?;
        let val = std::env::var(&key).unwrap_or_default();
        let n: U256 = val.parse().unwrap_or(U256::ZERO);
        return Ok(abi_encode_u256(n));
    }
    if selector == sel::env_address() {
        let key = abi_decode_string_dynamic(args, 0)?;
        let val = std::env::var(&key).unwrap_or_default();
        let addr: Address = val.parse().unwrap_or(Address::ZERO);
        return Ok(abi_encode_address(addr));
    }
    if selector == sel::env_bytes32() {
        let key = abi_decode_string_dynamic(args, 0)?;
        let val = std::env::var(&key).unwrap_or_default();
        let hex_str = val.trim_start_matches("0x");
        let bytes = hex::decode(hex_str).unwrap_or_default();
        let mut b32 = [0u8; 32];
        let len = bytes.len().min(32);
        b32[..len].copy_from_slice(&bytes[..len]);
        return Ok(b32.to_vec());
    }

    // ── Filesystem ──────────────────────────────────────────────────────────
    if selector == sel::read_file() {
        let path = abi_decode_string_dynamic(args, 0)?;
        let content = std::fs::read_to_string(&path)
            .map_err(|e| eyre::eyre!("readFile({path}): {e}"))?;
        return Ok(abi_encode_string(&content));
    }
    if selector == sel::write_file() {
        let path = abi_decode_string_dynamic(args, 0)?;
        let content = abi_decode_string_dynamic(args, 32)?;
        std::fs::write(&path, &content)
            .map_err(|e| eyre::eyre!("writeFile({path}): {e}"))?;
        return Ok(vec![]);
    }
    if selector == sel::exists() {
        let path = abi_decode_string_dynamic(args, 0)?;
        return Ok(abi_encode_bool(std::path::Path::new(&path).exists()));
    }

    // ── Utility ─────────────────────────────────────────────────────────────
    if selector == sel::label() {
        let addr = abi_decode_address(args, 0)?;
        let lbl = abi_decode_string_dynamic(args, 32)?;
        state.labels.insert(addr, lbl);
        return Ok(vec![]);
    }
    if selector == sel::get_label() {
        let addr = abi_decode_address(args, 0)?;
        let lbl = state.labels.get(&addr).cloned().unwrap_or_else(|| format!("{addr:#x}"));
        return Ok(abi_encode_string(&lbl));
    }
    if selector == sel::project_root() {
        let root = std::env::current_dir().unwrap_or_default();
        return Ok(abi_encode_string(&root.to_string_lossy()));
    }
    if selector == sel::random_uint() {
        let val = rand_u256();
        return Ok(abi_encode_u256(val));
    }
    if selector == sel::random_address() {
        let u = rand_u256();
        let addr = Address::from_word(B256::from(u));
        return Ok(abi_encode_address(addr));
    }

    // ── ffi ─────────────────────────────────────────────────────────────────
    if selector == sel::ffi() {
        let cmd_args = decode_string_array(args)?;
        if cmd_args.is_empty() { eyre::bail!("ffi: empty command"); }
        let out = std::process::Command::new(&cmd_args[0])
            .args(&cmd_args[1..])
            .output()
            .map_err(|e| eyre::eyre!("ffi failed: {e}"))?;
        let stdout = out.stdout;
        // Return as bytes
        return Ok(abi_encode_bytes(&stdout));
    }

    // ── Signing ─────────────────────────────────────────────────────────────
    if selector == sel::sign_msg() {
        let pk = abi_decode_u256(args, 0)?;
        let digest = abi_decode_b256(args, 32)?;
        let (v, r, s) = sign_with_key(pk, digest)?;
        // Returns (uint8 v, bytes32 r, bytes32 s)
        let mut ret = vec![0u8; 96];
        ret[31] = v;
        ret[32..64].copy_from_slice(r.as_slice());
        ret[64..96].copy_from_slice(s.as_slice());
        return Ok(ret);
    }

    eyre::bail!("unknown cheatcode selector: 0x{}", hex::encode(selector))
}

// ── ABI encoding helpers ─────────────────────────────────────────────────────

fn abi_decode_u256(data: &[u8], offset: usize) -> eyre::Result<U256> {
    if data.len() < offset + 32 { eyre::bail!("abi: too short for u256 at {offset}"); }
    Ok(U256::from_be_slice(&data[offset..offset + 32]))
}

fn abi_decode_u64(data: &[u8], offset: usize) -> eyre::Result<u64> {
    let u = abi_decode_u256(data, offset)?;
    Ok(u.to::<u64>())
}

fn abi_decode_bool(data: &[u8], offset: usize) -> eyre::Result<bool> {
    let u = abi_decode_u256(data, offset)?;
    Ok(u != U256::ZERO)
}

fn abi_decode_address(data: &[u8], offset: usize) -> eyre::Result<Address> {
    if data.len() < offset + 32 { eyre::bail!("abi: too short for address at {offset}"); }
    Ok(Address::from_slice(&data[offset + 12..offset + 32]))
}

fn abi_decode_b256(data: &[u8], offset: usize) -> eyre::Result<B256> {
    if data.len() < offset + 32 { eyre::bail!("abi: too short for b256 at {offset}"); }
    Ok(B256::from_slice(&data[offset..offset + 32]))
}

fn abi_decode_bytes_dynamic(data: &[u8], offset: usize) -> eyre::Result<Vec<u8>> {
    // data[offset..offset+32] is the offset pointer
    let ptr = U256::from_be_slice(&data[offset..offset + 32]).to::<usize>();
    if data.len() < ptr + 32 { eyre::bail!("abi: bytes ptr out of range"); }
    let len = U256::from_be_slice(&data[ptr..ptr + 32]).to::<usize>();
    if data.len() < ptr + 32 + len { eyre::bail!("abi: bytes data out of range"); }
    Ok(data[ptr + 32..ptr + 32 + len].to_vec())
}

fn abi_decode_string_dynamic(data: &[u8], offset: usize) -> eyre::Result<String> {
    let bytes = abi_decode_bytes_dynamic(data, offset)?;
    Ok(String::from_utf8(bytes).unwrap_or_default())
}

fn abi_encode_u256(val: U256) -> Vec<u8> {
    val.to_be_bytes::<32>().to_vec()
}

fn abi_encode_bool(val: bool) -> Vec<u8> {
    abi_encode_u256(if val { U256::from(1u8) } else { U256::ZERO })
}

fn abi_encode_address(addr: Address) -> Vec<u8> {
    let mut buf = [0u8; 32];
    buf[12..].copy_from_slice(addr.as_slice());
    buf.to_vec()
}

fn abi_encode_string(s: &str) -> Vec<u8> {
    abi_encode_bytes(s.as_bytes())
}

fn abi_encode_bytes(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; 64 + ((data.len() + 31) / 32) * 32];
    // offset = 32 (points to length word)
    out[31] = 32;
    // length
    let len_bytes = U256::from(data.len());
    out[32..64].copy_from_slice(&len_bytes.to_be_bytes::<32>());
    // data
    out[64..64 + data.len()].copy_from_slice(data);
    out
}

fn decode_string_array(data: &[u8]) -> eyre::Result<Vec<String>> {
    // ABI-encoded string[]
    if data.len() < 64 { return Ok(vec![]); }
    let array_offset = U256::from_be_slice(&data[..32]).to::<usize>();
    let count = U256::from_be_slice(&data[array_offset..array_offset + 32]).to::<usize>();
    let mut result = Vec::with_capacity(count);
    for i in 0..count {
        let elem_offset_pos = array_offset + 32 + i * 32;
        let elem_offset = U256::from_be_slice(&data[elem_offset_pos..elem_offset_pos + 32]).to::<usize>() + array_offset;
        let len = U256::from_be_slice(&data[elem_offset..elem_offset + 32]).to::<usize>();
        let s = String::from_utf8(data[elem_offset + 32..elem_offset + 32 + len].to_vec())
            .unwrap_or_default();
        result.push(s);
    }
    Ok(result)
}

fn rand_u256() -> U256 {
    use rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    U256::from_be_bytes(bytes)
}

fn derive_address_from_key(pk: U256) -> eyre::Result<Address> {
    use k256::ecdsa::SigningKey;
    use sha3::{Digest, Keccak256};
    let bytes = pk.to_be_bytes::<32>();
    let key = SigningKey::from_bytes(&bytes.into()).map_err(|e| eyre::eyre!("{e}"))?;
    let vk = key.verifying_key();
    let pub_bytes = vk.to_encoded_point(false);
    let hash = Keccak256::digest(&pub_bytes.as_bytes()[1..]);
    Ok(Address::from_slice(&hash[12..]))
}

fn sign_with_key(pk: U256, digest: B256) -> eyre::Result<(u8, B256, B256)> {
    use k256::ecdsa::{SigningKey, signature::hazmat::PrehashSigner};
    let bytes = pk.to_be_bytes::<32>();
    let key = SigningKey::from_bytes(&bytes.into()).map_err(|e| eyre::eyre!("{e}"))?;
    let (sig, rec_id): (k256::ecdsa::Signature, k256::ecdsa::RecoveryId) =
        key.sign_prehash(digest.as_slice()).map_err(|e| eyre::eyre!("{e}"))?;
    let v = rec_id.to_byte() + 27;
    let r = B256::from_slice(&sig.r().to_bytes());
    let s_val = B256::from_slice(&sig.s().to_bytes());
    Ok((v, r, s_val))
}

/// Load account into journaled state (inserting from DB if not yet present) then apply a mutation.
/// This avoids the double-borrow of &mut journaled_state and &mut db simultaneously.
fn cheatcode_load_account<DB: Database, F: FnOnce(&mut revm::primitives::Account)>(
    context: &mut EvmContext<DB>,
    addr: Address,
    f: F,
) {
    if !context.journaled_state.state.contains_key(&addr) {
        let info = context.db.basic(addr).ok().flatten().unwrap_or_default();
        context.journaled_state.state.insert(
            addr,
            revm::primitives::Account {
                info,
                storage: Default::default(),
                status: revm::primitives::AccountStatus::Loaded,
            },
        );
    }
    if let Some(acc) = context.journaled_state.state.get_mut(&addr) {
        f(acc);
        acc.mark_touch();
    }
}

fn make_success(data: Vec<u8>, gas: u64) -> CallOutcome {
    CallOutcome {
        result: InterpreterResult {
            result: InstructionResult::Return,
            output: Bytes::from(data),
            gas: Gas::new(gas),
        },
        memory_offset: 0..0,
    }
}

fn make_revert(msg: String, gas: u64) -> CallOutcome {
    // ABI-encode as Error(string)
    let mut out = vec![0x08, 0xc3, 0x79, 0xa0]; // Error(string) selector
    out.extend_from_slice(&abi_encode_string(&msg));
    CallOutcome {
        result: InterpreterResult {
            result: InstructionResult::Revert,
            output: Bytes::from(out),
            gas: Gas::new(gas),
        },
        memory_offset: 0..0,
    }
}
