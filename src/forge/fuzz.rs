use revm::primitives::{Address, Bytes, U256};
use crate::evm::executor::Executor;
use crate::evm::types::{TestResult, TestStatus, CounterExample};
use crate::forge::compiler::TestFunction;

/// Property-based fuzz runner backed by proptest strategies.
pub struct FuzzRunner {
    pub runs: u32,
}

impl FuzzRunner {
    pub fn new(runs: u32) -> Self { FuzzRunner { runs } }

    pub fn run(
        &self,
        executor: &mut Executor,
        contract_address: Address,
        test_fn: &TestFunction,
        abi: &serde_json::Value,
    ) -> TestResult {
        let param_types = extract_param_types(abi, &test_fn.name);
        let initial_snapshot = executor.snapshot();

        for run in 0..self.runs {
            // Generate random arguments
            let args = generate_args(&param_types, run as u64);
            let calldata = encode_call(&test_fn.selector, &args, &param_types);

            executor.revert_to_snapshot(&initial_snapshot);

            // Reset cheatcode assume-failure flag
            {
                let mut cc = executor.cheatcode_state.lock().unwrap();
                cc.expect_revert_failed = false;
            }

            let exec_result = executor.execute_tx(
                Address::ZERO,
                Some(contract_address),
                U256::ZERO,
                calldata,
            );

            match exec_result {
                Ok(r) => {
                    // vm.assume() failure signals discard
                    let reason = r.revert_reason.as_deref().unwrap_or("");
                    if reason.contains("vm.assume") || reason.contains("condition false") {
                        continue; // discard this input
                    }

                    let failed = if test_fn.expects_fail { r.success } else { !r.success };
                    if failed {
                        // Try to shrink the counterexample
                        let shrunk = shrink(executor, contract_address, test_fn, &param_types, &args, &initial_snapshot);
                        return TestResult {
                            name: test_fn.name.clone(),
                            status: TestStatus::Fail {
                                reason: r.revert_reason.clone(),
                            },
                            gas_used: r.gas_used,
                            logs: r.logs,
                            counterexample: Some(CounterExample {
                                args: shrunk,
                                runs: run as usize + 1,
                            }),
                        };
                    }
                }
                Err(e) => {
                    return TestResult {
                        name: test_fn.name.clone(),
                        status: TestStatus::Fail { reason: Some(e.to_string()) },
                        gas_used: 0,
                        logs: vec![],
                        counterexample: Some(CounterExample { args: format_args(&args), runs: run as usize + 1 }),
                    };
                }
            }
        }

        executor.revert_to_snapshot(&initial_snapshot);
        TestResult {
            name: test_fn.name.clone(),
            status: TestStatus::Pass,
            gas_used: 0,
            logs: vec![],
            counterexample: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FuzzType {
    Uint(u32),
    Int(u32),
    Address,
    Bool,
    Bytes(Option<u32>),
    String,
}

fn extract_param_types(abi: &serde_json::Value, fn_name: &str) -> Vec<FuzzType> {
    let items = match abi.as_array() { Some(a) => a, None => return vec![] };
    for item in items {
        if item.get("type").and_then(|t| t.as_str()) != Some("function") { continue; }
        if item.get("name").and_then(|n| n.as_str()) != Some(fn_name) { continue; }
        if let Some(inputs) = item.get("inputs").and_then(|i| i.as_array()) {
            return inputs.iter().filter_map(|inp| {
                parse_sol_type(inp.get("type").and_then(|t| t.as_str()).unwrap_or(""))
            }).collect();
        }
    }
    vec![]
}

fn parse_sol_type(ty: &str) -> Option<FuzzType> {
    if ty.starts_with("uint") {
        let bits = ty[4..].parse().unwrap_or(256);
        Some(FuzzType::Uint(bits))
    } else if ty.starts_with("int") {
        let bits = ty[3..].parse().unwrap_or(256);
        Some(FuzzType::Int(bits))
    } else if ty == "address" {
        Some(FuzzType::Address)
    } else if ty == "bool" {
        Some(FuzzType::Bool)
    } else if ty.starts_with("bytes") {
        let n = ty[5..].parse().ok();
        Some(FuzzType::Bytes(n))
    } else if ty == "string" {
        Some(FuzzType::String)
    } else {
        None
    }
}

/// Generate pseudorandom arguments using a seed.
fn generate_args(types: &[FuzzType], seed: u64) -> Vec<Vec<u8>> {
    use rand_core::{OsRng, RngCore};
    // Use seed to create a deterministic sequence
    let mut state = seed.wrapping_mul(0x517cc1b727220a95).wrapping_add(0x6c62272e07bb0142);
    types.iter().map(|ty| {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let r = state;
        generate_single(ty, r)
    }).collect()
}

fn generate_single(ty: &FuzzType, r: u64) -> Vec<u8> {
    match ty {
        FuzzType::Uint(bits) => {
            let mut buf = [0u8; 32];
            let val = r as u128;
            let mask = if *bits >= 128 { u128::MAX } else { (1u128 << bits) - 1 };
            let masked = val & mask;
            buf[16..].copy_from_slice(&masked.to_be_bytes());
            buf.to_vec()
        }
        FuzzType::Int(bits) => {
            let mut buf = [0u8; 32];
            let val = r as i64;
            let sign_ext = if val < 0 { 0xFFu8 } else { 0 };
            buf[..24].fill(sign_ext);
            buf[24..].copy_from_slice(&(val as i64).to_be_bytes());
            buf.to_vec()
        }
        FuzzType::Address => {
            let mut buf = [0u8; 32];
            buf[12..20].copy_from_slice(&r.to_be_bytes());
            buf.to_vec()
        }
        FuzzType::Bool => {
            let mut buf = [0u8; 32];
            buf[31] = (r & 1) as u8;
            buf.to_vec()
        }
        FuzzType::Bytes(Some(n)) => {
            let mut buf = vec![0u8; 32];
            let n = *n as usize;
            for i in 0..n.min(32) {
                buf[i] = ((r >> i) & 0xFF) as u8;
            }
            buf
        }
        FuzzType::Bytes(None) | FuzzType::String => {
            let len = (r % 32) as usize;
            let mut header = [0u8; 64];
            // offset = 32
            header[31] = 32;
            // length
            header[63] = len as u8;
            let mut data = vec![0u8; ((len + 31) / 32) * 32];
            for i in 0..len { data[i] = ((r >> i) & 0xFF) as u8; }
            let mut out = header.to_vec();
            out.extend(data);
            out
        }
    }
}

fn encode_call(selector: &[u8; 4], args: &[Vec<u8>], types: &[FuzzType]) -> Bytes {
    let mut calldata = selector.to_vec();
    for arg in args {
        calldata.extend(arg);
    }
    Bytes::from(calldata)
}

fn format_args(args: &[Vec<u8>]) -> Vec<String> {
    args.iter().map(|a| format!("0x{}", hex::encode(a))).collect()
}

/// Attempt to find a smaller counterexample.
fn shrink(
    executor: &mut Executor,
    contract: Address,
    test_fn: &TestFunction,
    types: &[FuzzType],
    failing_args: &[Vec<u8>],
    snapshot: &[u8],
) -> Vec<String> {
    // Simple shrinking: try zeroed args first, then half-values
    let mut best = failing_args.to_vec();

    for i in 0..types.len() {
        let mut candidate = best.clone();
        candidate[i] = vec![0u8; 32]; // try zero
        if still_fails(executor, contract, test_fn, &candidate, snapshot) {
            best = candidate;
            continue;
        }
        // Try halving the value
        if best[i].len() == 32 {
            let mut halved = best[i].clone();
            for j in 0..32 {
                halved[j] >>= 1;
            }
            candidate = best.clone();
            candidate[i] = halved;
            if still_fails(executor, contract, test_fn, &candidate, snapshot) {
                best = candidate;
            }
        }
    }

    format_args(&best)
}

fn still_fails(
    executor: &mut Executor,
    contract: Address,
    test_fn: &TestFunction,
    args: &[Vec<u8>],
    snapshot: &[u8],
) -> bool {
    executor.revert_to_snapshot(snapshot);
    let types: Vec<FuzzType> = args.iter().map(|a| FuzzType::Uint(256)).collect();
    let calldata = encode_call(&test_fn.selector, args, &types);
    match executor.execute_tx(Address::ZERO, Some(contract), U256::ZERO, calldata) {
        Ok(r) => if test_fn.expects_fail { r.success } else { !r.success },
        Err(_) => true,
    }
}
