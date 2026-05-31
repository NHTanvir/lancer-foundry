use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledContract {
    pub name: String,
    pub abi: serde_json::Value,
    pub bytecode: Vec<u8>,
    pub deployed_bytecode: Vec<u8>,
    pub source_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SolcOutput {
    contracts: Option<HashMap<String, HashMap<String, SolcContract>>>,
    errors: Option<Vec<SolcError>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SolcContract {
    abi: Option<serde_json::Value>,
    bin: Option<String>,
    #[serde(rename = "bin-runtime")]
    bin_runtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SolcError {
    #[serde(rename = "type")]
    error_type: Option<String>,
    severity: Option<String>,
    message: Option<String>,
    #[serde(rename = "formattedMessage")]
    formatted_message: Option<String>,
}

/// Find solc binary in PATH or common locations.
pub fn find_solc() -> Option<PathBuf> {
    for name in &["solc", "solc.exe"] {
        if let Ok(path) = which::which(name) {
            return Some(path);
        }
    }
    // Check ~/.svm/ for svm-installed versions
    if let Some(home) = dirs::home_dir() {
        let svm_dir = home.join(".svm");
        if svm_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&svm_dir) {
                let mut versions: Vec<_> = entries
                    .filter_map(|e| e.ok())
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .collect();
                versions.sort_by_key(|e| e.file_name());
                for entry in versions.iter().rev() {
                    let solc = entry.path().join("solc");
                    if solc.exists() { return Some(solc); }
                    let solc = entry.path().join("solc.exe");
                    if solc.exists() { return Some(solc); }
                }
            }
        }
    }
    None
}

/// Compile a list of Solidity source files using solc.
/// Returns compiled contracts keyed by "{file}:{ContractName}".
pub fn compile(
    sources: &[PathBuf],
    allow_paths: &[PathBuf],
    remappings: &[(String, String)],
    optimizer: bool,
    optimizer_runs: u32,
    via_ir: bool,
) -> eyre::Result<HashMap<String, CompiledContract>> {
    let solc = find_solc().ok_or_else(|| eyre::eyre!(
        "solc not found. Install via: curl -L https://foundry.paradigm.xyz | bash && foundryup\n\
         Or install solc manually from https://docs.soliditylang.org/en/latest/installing-solidity.html"
    ))?;

    let mut args = vec![
        "--combined-json".to_string(),
        "abi,bin,bin-runtime".to_string(),
    ];

    if optimizer {
        args.push("--optimize".to_string());
        args.push("--optimize-runs".to_string());
        args.push(optimizer_runs.to_string());
    }

    if via_ir {
        args.push("--via-ir".to_string());
    }

    for (from, to) in remappings {
        args.push(format!("{from}={to}"));
    }

    let allow_path_str: Vec<String> = allow_paths.iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    if !allow_path_str.is_empty() {
        args.push("--allow-paths".to_string());
        args.push(allow_path_str.join(","));
    }

    for src in sources {
        args.push(src.to_string_lossy().to_string());
    }

    let output = Command::new(&solc)
        .args(&args)
        .output()
        .map_err(|e| eyre::eyre!("failed to run solc: {e}"))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() && !output.status.success() {
        eyre::bail!("solc error:\n{stderr}");
    }

    // Log warnings
    if !stderr.is_empty() {
        tracing::warn!("solc warnings:\n{stderr}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let solc_out: SolcOutput = serde_json::from_str(&stdout)
        .map_err(|e| eyre::eyre!("failed to parse solc output: {e}\n{stdout}"))?;

    // Log any errors
    if let Some(errors) = &solc_out.errors {
        for err in errors {
            if err.severity.as_deref() == Some("error") {
                let msg = err.formatted_message.as_deref()
                    .or(err.message.as_deref())
                    .unwrap_or("unknown error");
                eyre::bail!("compilation error: {msg}");
            }
        }
    }

    let mut result = HashMap::new();

    if let Some(contracts) = solc_out.contracts {
        for (file_path, file_contracts) in contracts {
            for (contract_name, contract) in file_contracts {
                let bytecode = hex::decode(
                    contract.bin.as_deref().unwrap_or("").trim_start_matches("0x")
                ).unwrap_or_default();

                let deployed_bytecode = hex::decode(
                    contract.bin_runtime.as_deref().unwrap_or("").trim_start_matches("0x")
                ).unwrap_or_default();

                let abi = contract.abi.unwrap_or(serde_json::Value::Array(vec![]));

                let key = format!("{file_path}:{contract_name}");
                result.insert(key, CompiledContract {
                    name: contract_name,
                    abi,
                    bytecode,
                    deployed_bytecode,
                    source_file: file_path.clone(),
                });
            }
        }
    }

    Ok(result)
}

/// Find all .sol files matching a glob pattern.
pub fn find_sol_files(root: &Path, pattern: &str) -> Vec<PathBuf> {
    glob::glob(&root.join(pattern).to_string_lossy())
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .collect()
}

/// Detect if a contract is a test contract (name ends with "Test" or file ends with ".t.sol").
pub fn is_test_contract(file: &Path, name: &str) -> bool {
    file.to_string_lossy().ends_with(".t.sol")
        || name.ends_with("Test")
        || name.starts_with("Test")
}

/// Extract test function names from ABI (functions starting with "test" or "testFail").
pub fn extract_test_functions(abi: &serde_json::Value) -> Vec<TestFunction> {
    let mut fns = vec![];
    if let Some(items) = abi.as_array() {
        for item in items {
            if item.get("type").and_then(|t| t.as_str()) != Some("function") { continue; }
            let name = item.get("name").and_then(|n| n.as_str()).unwrap_or("");
            if name.starts_with("test") {
                let expects_fail = name.starts_with("testFail");
                let is_fuzz = item.get("inputs")
                    .and_then(|i| i.as_array())
                    .map(|inputs| !inputs.is_empty())
                    .unwrap_or(false);
                fns.push(TestFunction {
                    name: name.to_string(),
                    expects_fail,
                    is_fuzz,
                    selector: compute_selector(name),
                });
            }
        }
    }
    fns
}

#[derive(Debug, Clone)]
pub struct TestFunction {
    pub name: String,
    pub expects_fail: bool,
    pub is_fuzz: bool,
    pub selector: [u8; 4],
}

pub fn has_setup(abi: &serde_json::Value) -> bool {
    abi.as_array().map_or(false, |items| {
        items.iter().any(|item| {
            item.get("type").and_then(|t| t.as_str()) == Some("function")
                && item.get("name").and_then(|n| n.as_str()) == Some("setUp")
        })
    })
}

fn compute_selector(name: &str) -> [u8; 4] {
    use sha3::{Digest, Keccak256};
    // Assume no-arg selector for setUp / test functions
    let sig = format!("{name}()");
    let hash = Keccak256::digest(sig.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}
