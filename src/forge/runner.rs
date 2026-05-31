use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use revm::primitives::{Address, Bytes, U256};

use crate::evm::executor::Executor;
use crate::evm::types::{TestResult, TestStatus};
use crate::forge::compiler::{self, CompiledContract, TestFunction};
use crate::forge::filter::TestFilter;
use crate::forge::fuzz::FuzzRunner;

/// Runs all forge tests in a project directory.
pub struct ForgeTestRunner {
    pub root: PathBuf,
    pub filter: TestFilter,
    pub verbosity: u8,
    pub fuzz_runs: u32,
    pub gas_report: bool,
    pub fail_fast: bool,
}

impl Default for ForgeTestRunner {
    fn default() -> Self {
        ForgeTestRunner {
            root: std::env::current_dir().unwrap_or_default(),
            filter: TestFilter::default(),
            verbosity: 0,
            fuzz_runs: 256,
            gas_report: false,
            fail_fast: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct TestSuiteResult {
    pub contract: String,
    pub results: Vec<TestResult>,
    pub setup_failed: bool,
    pub setup_error: Option<String>,
}

impl TestSuiteResult {
    pub fn passed(&self) -> usize {
        self.results.iter().filter(|r| r.status == TestStatus::Pass).count()
    }
    pub fn failed(&self) -> usize {
        self.results.iter().filter(|r| matches!(r.status, TestStatus::Fail { .. })).count()
    }
    pub fn skipped(&self) -> usize {
        self.results.iter().filter(|r| r.status == TestStatus::Skipped).count()
    }
}

impl ForgeTestRunner {
    pub fn run(&self) -> eyre::Result<Vec<TestSuiteResult>> {
        // 1. Find test files
        let test_files = compiler::find_sol_files(&self.root, "test/**/*.t.sol");
        if test_files.is_empty() {
            tracing::warn!("No .t.sol files found under {}", self.root.display());
            return Ok(vec![]);
        }

        // 2. Compile
        let src_files = compiler::find_sol_files(&self.root, "src/**/*.sol");
        let lib_files = compiler::find_sol_files(&self.root, "lib/**/*.sol");
        let mut all_files: Vec<PathBuf> = test_files.clone();
        all_files.extend(src_files);
        all_files.extend(lib_files);
        all_files.sort();
        all_files.dedup();

        let compiled = compiler::compile(
            &all_files,
            &[self.root.clone()],
            &[],
            true,
            200,
            false,
        )?;

        // 3. Run each test contract
        let mut suite_results = vec![];
        for (key, contract) in &compiled {
            if !compiler::is_test_contract(Path::new(&contract.source_file), &contract.name) {
                continue;
            }
            if self.filter.match_contract(&contract.name) {
                let result = self.run_suite(contract)?;
                if self.fail_fast && result.failed() > 0 {
                    suite_results.push(result);
                    break;
                }
                suite_results.push(result);
            }
        }
        Ok(suite_results)
    }

    fn run_suite(&self, contract: &CompiledContract) -> eyre::Result<TestSuiteResult> {
        let mut result = TestSuiteResult {
            contract: contract.name.clone(),
            ..Default::default()
        };

        let test_fns = compiler::extract_test_functions(&contract.abi);
        if test_fns.is_empty() { return Ok(result); }

        // Create executor + deploy contract
        let mut executor = Executor::new(1, 0);
        executor.set_balance(Address::ZERO, U256::from(10u128.pow(21))); // Give test contract ETH

        let deploy_result = executor.deploy(
            Address::ZERO,
            Bytes::copy_from_slice(&contract.bytecode),
            U256::ZERO,
        );

        let contract_address = match deploy_result {
            Ok(r) => r.address,
            Err(e) => {
                result.setup_failed = true;
                result.setup_error = Some(format!("deploy failed: {e}"));
                return Ok(result);
            }
        };

        // Run setUp() if present
        let has_setup = compiler::has_setup(&contract.abi);
        let setup_snapshot = executor.snapshot();

        if has_setup {
            let setup_selector = {
                use sha3::{Digest, Keccak256};
                let hash = Keccak256::digest(b"setUp()");
                hash[..4].to_vec()
            };
            let setup_result = executor.execute_tx(
                Address::ZERO,
                Some(contract_address),
                U256::ZERO,
                Bytes::copy_from_slice(&setup_selector),
            );
            match setup_result {
                Ok(r) if !r.success => {
                    result.setup_failed = true;
                    result.setup_error = r.revert_reason;
                    return Ok(result);
                }
                Err(e) => {
                    result.setup_failed = true;
                    result.setup_error = Some(e.to_string());
                    return Ok(result);
                }
                _ => {}
            }
        }

        // Snapshot after setUp for test isolation
        let post_setup_snapshot = executor.snapshot();

        // Run each test function
        for test_fn in &test_fns {
            if !self.filter.match_test(&test_fn.name) { continue; }

            // Restore state to post-setUp snapshot
            executor.revert_to_snapshot(&post_setup_snapshot);

            let test_result = if test_fn.is_fuzz {
                self.run_fuzz_test(&mut executor, contract_address, test_fn, &contract.abi)
            } else {
                self.run_unit_test(&mut executor, contract_address, test_fn)
            };

            result.results.push(test_result);
        }

        Ok(result)
    }

    fn run_unit_test(
        &self,
        executor: &mut Executor,
        contract_address: Address,
        test_fn: &TestFunction,
    ) -> TestResult {
        let calldata = Bytes::copy_from_slice(&test_fn.selector);
        let exec_result = executor.execute_tx(
            Address::ZERO,
            Some(contract_address),
            U256::ZERO,
            calldata,
        );

        match exec_result {
            Ok(r) => {
                // Check cheatcode state for violations
                let cc_state = executor.cheatcode_state.lock().unwrap();
                let cheat_failed = cc_state.expect_revert_failed;
                drop(cc_state);

                if cheat_failed {
                    // Reset state
                    let mut cc = executor.cheatcode_state.lock().unwrap();
                    cc.expect_revert_failed = false;
                    drop(cc);
                    return TestResult {
                        name: test_fn.name.clone(),
                        status: TestStatus::Fail { reason: Some("expectRevert: call did not revert".into()) },
                        gas_used: r.gas_used,
                        logs: r.logs,
                        counterexample: None,
                    };
                }

                let passed = if test_fn.expects_fail { !r.success } else { r.success };
                let status = if passed {
                    TestStatus::Pass
                } else {
                    // Check if test was skipped via vm.skip()
                    let reason_str = r.revert_reason.as_deref().unwrap_or("");
                    if reason_str == "__skip__" || reason_str.contains("__skip__") {
                        TestStatus::Skipped
                    } else {
                        TestStatus::Fail { reason: r.revert_reason }
                    }
                };
                TestResult { name: test_fn.name.clone(), status, gas_used: r.gas_used, logs: r.logs, counterexample: None }
            }
            Err(e) => TestResult {
                name: test_fn.name.clone(),
                status: TestStatus::Fail { reason: Some(e.to_string()) },
                gas_used: 0,
                logs: vec![],
                counterexample: None,
            },
        }
    }

    fn run_fuzz_test(
        &self,
        executor: &mut Executor,
        contract_address: Address,
        test_fn: &TestFunction,
        abi: &serde_json::Value,
    ) -> TestResult {
        let fuzzer = FuzzRunner::new(self.fuzz_runs);
        fuzzer.run(executor, contract_address, test_fn, abi)
    }
}
