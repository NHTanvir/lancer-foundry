use std::time::Duration;
use crate::evm::types::{TestResult, TestStatus};
use crate::forge::runner::TestSuiteResult;

pub fn print_results(suites: &[TestSuiteResult], elapsed: Duration, verbosity: u8) {
    let mut total_pass = 0usize;
    let mut total_fail = 0usize;
    let mut total_skip = 0usize;

    for suite in suites {
        println!("Running {} tests for {}", suite.results.len(), suite.contract);

        if suite.setup_failed {
            let err = suite.setup_error.as_deref().unwrap_or("unknown");
            println!("  setUp() FAILED: {err}");
            continue;
        }

        for r in &suite.results {
            match &r.status {
                TestStatus::Pass => {
                    total_pass += 1;
                    if verbosity >= 1 {
                        println!("  [PASS] {} (gas: {})", r.name, r.gas_used);
                    } else {
                        print!(".");
                    }
                }
                TestStatus::Fail { reason } => {
                    total_fail += 1;
                    let reason_str = reason.as_deref().unwrap_or("no reason");
                    println!("  [FAIL] {}: {}", r.name, reason_str);
                    if let Some(ce) = &r.counterexample {
                        println!("    Counterexample: args = [{}] (after {} runs)",
                            ce.args.join(", "), ce.runs);
                    }
                    if verbosity >= 2 {
                        for log in &r.logs {
                            println!("    Log: {:?}", log);
                        }
                    }
                }
                TestStatus::Skipped => {
                    total_skip += 1;
                    if verbosity >= 1 {
                        println!("  [SKIP] {}", r.name);
                    }
                }
            }
        }

        if verbosity == 0 {
            let p = suite.passed();
            let f = suite.failed();
            println!();
            print_suite_summary(suite);
        }
    }

    println!();
    println!("Test result: {} passed; {} failed; {} skipped; finished in {:.2?}",
        total_pass, total_fail, total_skip, elapsed);

    if total_fail == 0 && (total_pass + total_skip) > 0 {
        println!("All tests pass ✓");
    }
}

fn print_suite_summary(suite: &TestSuiteResult) {
    let p = suite.passed();
    let f = suite.failed();
    let s = suite.skipped();
    if f > 0 {
        println!("  suite result: FAILED. {p} passed; {f} failed; {s} skipped");
    } else {
        println!("  suite result: ok. {p} passed; {f} failed; {s} skipped");
    }
}

pub fn print_gas_report(suites: &[TestSuiteResult]) {
    println!("\n╔══════════════════════════════════════════╗");
    println!("║              Gas Report                  ║");
    println!("╚══════════════════════════════════════════╝");
    println!("{:<40} {:>12}", "Test", "Gas Used");
    println!("{}", "─".repeat(54));
    for suite in suites {
        for r in &suite.results {
            println!("{:<40} {:>12}", r.name, r.gas_used);
        }
    }
    println!();
}
