use std::path::PathBuf;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintFinding {
    pub rule: String,
    pub severity: Severity,
    pub file: String,
    pub line: usize,
    pub message: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Severity {
    High,
    Medium,
    Low,
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Severity::High => write!(f, "HIGH"), Severity::Medium => write!(f, "MED"),
            Severity::Low => write!(f, "LOW"), Severity::Info => write!(f, "INFO") }
    }
}

/// Run all lint rules over a set of Solidity source files.
pub fn lint_files(files: &[PathBuf]) -> Vec<LintFinding> {
    let mut findings = vec![];
    for file in files {
        if let Ok(source) = std::fs::read_to_string(file) {
            let file_str = file.to_string_lossy().to_string();
            findings.extend(lint_source(&file_str, &source));
        }
    }
    findings
}

fn lint_source(file: &str, source: &str) -> Vec<LintFinding> {
    let mut findings = vec![];
    let rules: &[(&str, &dyn Fn(&str, &str, usize) -> Option<LintFinding>)] = &[
        ("reentrancy-events",       &check_reentrancy_events),
        ("unprotected-initializer", &check_unprotected_initializer),
        ("locked-ether",            &check_locked_ether),
        ("weak-prng",               &check_weak_prng),
        ("arbitrary-send-erc20",    &check_arbitrary_send_erc20),
        ("missing-return",          &check_missing_return),
        ("unchecked-low-call",      &check_unchecked_low_call),
        ("selfdestruct",            &check_selfdestruct),
        ("tx-origin-auth",          &check_tx_origin_auth),
        ("integer-overflow",        &check_integer_overflow),
    ];

    for (i, line) in source.lines().enumerate() {
        let lineno = i + 1;
        for (rule_name, rule_fn) in rules {
            if let Some(mut finding) = rule_fn(file, line, lineno) {
                finding.rule = rule_name.to_string();
                findings.push(finding);
            }
        }
    }
    findings
}

fn check_reentrancy_events(_file: &str, line: &str, _lineno: usize) -> Option<LintFinding> {
    // Emit after external call pattern
    if (line.contains(".call{") || line.contains(".call(")) && !line.contains("//") {
        return None; // Need multi-line analysis; just flag for now
    }
    None
}

fn check_unprotected_initializer(file: &str, line: &str, lineno: usize) -> Option<LintFinding> {
    if (line.contains("function initialize(") || line.contains("function __init("))
        && !line.contains("initializer")
        && !line.contains("onlyOwner")
        && !line.contains("//")
    {
        return Some(LintFinding {
            rule: String::new(),
            severity: Severity::High,
            file: file.to_string(),
            line: lineno,
            message: "Unprotected initializer function may be called by anyone".into(),
            snippet: Some(line.trim().to_string()),
        });
    }
    None
}

fn check_locked_ether(file: &str, line: &str, lineno: usize) -> Option<LintFinding> {
    // Contract receives ETH (payable) but has no withdraw mechanism
    // Simple heuristic: function marked payable without send/transfer/call in the contract
    if line.contains("payable") && line.contains("function") && !line.contains("withdraw") {
        // This is a very simplistic check - real impl needs contract-level analysis
    }
    None
}

fn check_weak_prng(file: &str, line: &str, lineno: usize) -> Option<LintFinding> {
    if (line.contains("block.timestamp") || line.contains("blockhash") || line.contains("block.difficulty"))
        && (line.contains("% ") || line.contains("mod "))
        && !line.contains("//")
    {
        return Some(LintFinding {
            rule: String::new(),
            severity: Severity::Medium,
            file: file.to_string(),
            line: lineno,
            message: "Weak pseudo-random number generation using block variables".into(),
            snippet: Some(line.trim().to_string()),
        });
    }
    None
}

fn check_arbitrary_send_erc20(file: &str, line: &str, lineno: usize) -> Option<LintFinding> {
    if line.contains("transferFrom(") && !line.contains("msg.sender") && !line.contains("//") {
        return Some(LintFinding {
            rule: String::new(),
            severity: Severity::High,
            file: file.to_string(),
            line: lineno,
            message: "transferFrom with arbitrary from address may allow unauthorized token transfer".into(),
            snippet: Some(line.trim().to_string()),
        });
    }
    None
}

fn check_missing_return(_file: &str, _line: &str, _lineno: usize) -> Option<LintFinding> { None }

fn check_unchecked_low_call(file: &str, line: &str, lineno: usize) -> Option<LintFinding> {
    if (line.contains(".call{") || line.contains(".call(")) && !line.contains("require") && !line.contains("//") {
        // Only flag if the return value isn't captured
        if !line.contains("(bool") && !line.contains("bool ") {
            return Some(LintFinding {
                rule: String::new(),
                severity: Severity::Medium,
                file: file.to_string(),
                line: lineno,
                message: "Low-level call return value not checked".into(),
                snippet: Some(line.trim().to_string()),
            });
        }
    }
    None
}

fn check_selfdestruct(file: &str, line: &str, lineno: usize) -> Option<LintFinding> {
    if (line.contains("selfdestruct(") || line.contains("suicide(")) && !line.contains("//") {
        return Some(LintFinding {
            rule: String::new(),
            severity: Severity::High,
            file: file.to_string(),
            line: lineno,
            message: "Use of selfdestruct — verify access controls and deprecation warnings".into(),
            snippet: Some(line.trim().to_string()),
        });
    }
    None
}

fn check_tx_origin_auth(file: &str, line: &str, lineno: usize) -> Option<LintFinding> {
    if line.contains("tx.origin") && (line.contains("==") || line.contains("require")) && !line.contains("//") {
        return Some(LintFinding {
            rule: String::new(),
            severity: Severity::Medium,
            file: file.to_string(),
            line: lineno,
            message: "Authorization using tx.origin is unsafe — use msg.sender instead".into(),
            snippet: Some(line.trim().to_string()),
        });
    }
    None
}

fn check_integer_overflow(file: &str, line: &str, lineno: usize) -> Option<LintFinding> {
    // Warn about unchecked arithmetic blocks
    if line.contains("unchecked {") && !line.contains("//") {
        return Some(LintFinding {
            rule: String::new(),
            severity: Severity::Info,
            file: file.to_string(),
            line: lineno,
            message: "Unchecked arithmetic block — ensure no integer overflow/underflow is possible".into(),
            snippet: Some(line.trim().to_string()),
        });
    }
    None
}

pub fn print_findings(findings: &[LintFinding]) {
    if findings.is_empty() {
        println!("No lint findings.");
        return;
    }

    let mut high = 0; let mut med = 0; let mut low = 0; let mut info = 0;
    for f in findings {
        match f.severity { Severity::High => high += 1, Severity::Medium => med += 1,
            Severity::Low => low += 1, Severity::Info => info += 1 }
        println!("[{}] {} ({}:{})", f.severity, f.rule, f.file, f.line);
        println!("     {}", f.message);
        if let Some(ref s) = f.snippet { println!("     > {s}"); }
        println!();
    }
    println!("Summary: {high} high, {med} medium, {low} low, {info} informational");
}
