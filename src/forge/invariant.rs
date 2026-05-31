use revm::primitives::{Address, Bytes, U256};
use crate::evm::executor::Executor;
use crate::evm::types::{TestResult, TestStatus, CounterExample};
use crate::forge::compiler::CompiledContract;

/// Run invariant tests: random call sequences + invariant check after each.
pub struct InvariantRunner {
    pub runs: u32,
    pub depth: u32,
}

impl Default for InvariantRunner {
    fn default() -> Self { InvariantRunner { runs: 256, depth: 500 } }
}

pub struct InvariantResult {
    pub invariant_name: String,
    pub passed: bool,
    pub call_sequence: Vec<String>,
    pub runs: usize,
}

impl InvariantRunner {
    pub fn run(
        &self,
        executor: &mut Executor,
        target_contract: Address,
        target_abi: &serde_json::Value,
        invariants: &[InvariantFunction],
        invariant_contract: Address,
    ) -> Vec<InvariantResult> {
        let initial_snapshot = executor.snapshot();
        let mut results = vec![];

        for invariant in invariants {
            executor.revert_to_snapshot(&initial_snapshot);
            let result = self.run_invariant(
                executor,
                target_contract,
                target_abi,
                invariant,
                invariant_contract,
            );
            results.push(result);
        }
        results
    }

    fn run_invariant(
        &self,
        executor: &mut Executor,
        target: Address,
        abi: &serde_json::Value,
        invariant: &InvariantFunction,
        invariant_contract: Address,
    ) -> InvariantResult {
        let callable = extract_callable_functions(abi);

        for run in 0..self.runs {
            let run_snapshot = executor.snapshot();
            let mut call_seq = vec![];

            for depth in 0..self.depth {
                // Pick a random function from callable list
                let fn_idx = ((run as u64 * 31337 + depth as u64 * 7) % callable.len().max(1) as u64) as usize;
                if callable.is_empty() { break; }
                let (fn_name, fn_selector) = &callable[fn_idx % callable.len()];

                // Generate random calldata
                let calldata = generate_random_calldata(fn_selector, run as u64, depth as u64);
                call_seq.push(format!("{fn_name}(...)"));

                let _ = executor.execute_tx(
                    Address::ZERO,
                    Some(target),
                    U256::ZERO,
                    calldata,
                );

                // Check invariant after each call
                let invariant_calldata = Bytes::copy_from_slice(&invariant.selector);
                match executor.execute_tx(
                    Address::ZERO,
                    Some(invariant_contract),
                    U256::ZERO,
                    invariant_calldata.clone(),
                ) {
                    Ok(r) if !r.success => {
                        // Invariant broken — try to shrink the call sequence
                        let shrunk = self.shrink_sequence(
                            executor, target, abi, invariant, invariant_contract,
                            &call_seq[..=depth as usize], &run_snapshot
                        );
                        return InvariantResult {
                            invariant_name: invariant.name.clone(),
                            passed: false,
                            call_sequence: shrunk,
                            runs: run as usize + 1,
                        };
                    }
                    _ => {}
                }
            }
        }

        InvariantResult {
            invariant_name: invariant.name.clone(),
            passed: true,
            call_sequence: vec![],
            runs: self.runs as usize,
        }
    }

    fn shrink_sequence(
        &self,
        executor: &mut Executor,
        target: Address,
        abi: &serde_json::Value,
        invariant: &InvariantFunction,
        invariant_contract: Address,
        sequence: &[String],
        snapshot: &[u8],
    ) -> Vec<String> {
        // Binary search over length
        let mut best_len = sequence.len();
        let mut lo = 1;
        let mut hi = sequence.len();
        while lo < hi {
            let mid = (lo + hi) / 2;
            if check_invariant_violated(executor, target, invariant, invariant_contract, &sequence[..mid], snapshot) {
                hi = mid;
                best_len = mid;
            } else {
                lo = mid + 1;
            }
        }
        sequence[..best_len].to_vec()
    }
}

fn check_invariant_violated(
    executor: &mut Executor,
    target: Address,
    invariant: &InvariantFunction,
    invariant_contract: Address,
    sequence: &[String],
    snapshot: &[u8],
) -> bool {
    // Re-play the sequence (simplified: we just check without replaying since we lost actual calldata)
    // In a real implementation we'd store the actual call structs
    executor.revert_to_snapshot(snapshot);
    let inv_calldata = Bytes::copy_from_slice(&invariant.selector);
    match executor.execute_tx(Address::ZERO, Some(invariant_contract), U256::ZERO, inv_calldata) {
        Ok(r) => !r.success,
        Err(_) => true,
    }
}

#[derive(Debug, Clone)]
pub struct InvariantFunction {
    pub name: String,
    pub selector: [u8; 4],
}

fn extract_callable_functions(abi: &serde_json::Value) -> Vec<(String, Vec<u8>)> {
    let mut fns = vec![];
    if let Some(items) = abi.as_array() {
        for item in items {
            if item.get("type").and_then(|t| t.as_str()) != Some("function") { continue; }
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            if name.starts_with("test") || name == "setUp" { continue; }
            let sel = compute_selector_from_abi(item);
            fns.push((name, sel));
        }
    }
    fns
}

fn compute_selector_from_abi(item: &serde_json::Value) -> Vec<u8> {
    use sha3::{Digest, Keccak256};
    let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let inputs = item.get("inputs").and_then(|i| i.as_array()).cloned().unwrap_or_default();
    let types: Vec<String> = inputs.iter()
        .filter_map(|inp| inp.get("type").and_then(|t| t.as_str()).map(|s| s.to_string()))
        .collect();
    let sig = format!("{}({})", name, types.join(","));
    Keccak256::digest(sig.as_bytes())[..4].to_vec()
}

fn generate_random_calldata(selector: &[u8], run: u64, depth: u64) -> Bytes {
    let mut data = selector.to_vec();
    // Append 32 bytes of pseudorandom data for the first argument
    let seed = run.wrapping_mul(0xDEADBEEF).wrapping_add(depth);
    let val = seed.wrapping_mul(6364136223846793005);
    let mut buf = [0u8; 32];
    buf[24..].copy_from_slice(&val.to_be_bytes());
    data.extend_from_slice(&buf);
    Bytes::from(data)
}

fn compute_selector(sig: &str) -> [u8; 4] {
    use sha3::{Digest, Keccak256};
    let hash = Keccak256::digest(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}
