use std::path::PathBuf;
use std::collections::HashMap;

/// Generate NatSpec documentation from Solidity source files.
pub struct DocGenerator {
    pub root: PathBuf,
    pub out: PathBuf,
}

#[derive(Debug, Default)]
struct ContractDoc {
    name: String,
    notice: Option<String>,
    dev: Option<String>,
    functions: Vec<FunctionDoc>,
    events: Vec<EventDoc>,
    errors: Vec<ErrorDoc>,
}

#[derive(Debug)]
struct FunctionDoc {
    name: String,
    signature: String,
    notice: Option<String>,
    dev: Option<String>,
    params: Vec<(String, String)>,
    returns: Vec<(String, String)>,
}

#[derive(Debug)]
struct EventDoc { name: String, notice: Option<String> }
#[derive(Debug)]
struct ErrorDoc  { name: String, notice: Option<String> }

impl DocGenerator {
    pub fn generate(&self, files: &[PathBuf]) -> eyre::Result<()> {
        std::fs::create_dir_all(&self.out)?;
        let mut all_contracts: Vec<ContractDoc> = vec![];

        for file in files {
            if let Ok(source) = std::fs::read_to_string(file) {
                let contracts = parse_natspec(&source);
                all_contracts.extend(contracts);
            }
        }

        // Write index
        self.write_index(&all_contracts)?;
        for contract in &all_contracts {
            self.write_contract_doc(contract)?;
        }

        println!("Documentation written to {}", self.out.display());
        Ok(())
    }

    fn write_index(&self, contracts: &[ContractDoc]) -> eyre::Result<()> {
        let mut md = String::from("# Smart Contract Documentation\n\n");
        md.push_str("## Contracts\n\n");
        for c in contracts {
            md.push_str(&format!("- [{}](./{}.md)\n", c.name, c.name));
        }
        std::fs::write(self.out.join("README.md"), md)?;
        Ok(())
    }

    fn write_contract_doc(&self, contract: &ContractDoc) -> eyre::Result<()> {
        let mut md = format!("# {}\n\n", contract.name);
        if let Some(ref n) = contract.notice { md.push_str(&format!("{n}\n\n")); }
        if let Some(ref d) = contract.dev { md.push_str(&format!("*{d}*\n\n")); }

        if !contract.functions.is_empty() {
            md.push_str("## Functions\n\n");
            for f in &contract.functions {
                md.push_str(&format!("### `{}`\n\n", f.signature));
                if let Some(ref n) = f.notice { md.push_str(&format!("{n}\n\n")); }
                if !f.params.is_empty() {
                    md.push_str("**Parameters:**\n\n");
                    for (name, desc) in &f.params {
                        md.push_str(&format!("- `{name}`: {desc}\n"));
                    }
                    md.push('\n');
                }
                if !f.returns.is_empty() {
                    md.push_str("**Returns:**\n\n");
                    for (name, desc) in &f.returns {
                        md.push_str(&format!("- `{name}`: {desc}\n"));
                    }
                    md.push('\n');
                }
            }
        }

        if !contract.events.is_empty() {
            md.push_str("## Events\n\n");
            for e in &contract.events {
                md.push_str(&format!("### `{}`\n\n", e.name));
                if let Some(ref n) = e.notice { md.push_str(&format!("{n}\n\n")); }
            }
        }

        std::fs::write(self.out.join(format!("{}.md", contract.name)), md)?;
        Ok(())
    }
}

fn parse_natspec(source: &str) -> Vec<ContractDoc> {
    let mut contracts = vec![];
    let mut current_contract: Option<ContractDoc> = None;
    let mut pending_notice: Option<String> = None;
    let mut pending_dev: Option<String> = None;

    for line in source.lines() {
        let trimmed = line.trim();

        // NatSpec comments
        if trimmed.starts_with("/// @notice") {
            pending_notice = Some(trimmed[11..].trim().to_string());
        } else if trimmed.starts_with("/// @dev") {
            pending_dev = Some(trimmed[8..].trim().to_string());
        } else if trimmed.starts_with("contract ") || trimmed.starts_with("abstract contract ") {
            let name = trimmed
                .trim_start_matches("abstract ")
                .trim_start_matches("contract ")
                .split_whitespace().next().unwrap_or("Unknown")
                .to_string();
            if let Some(prev) = current_contract.take() { contracts.push(prev); }
            current_contract = Some(ContractDoc {
                name,
                notice: pending_notice.take(),
                dev: pending_dev.take(),
                ..Default::default()
            });
        } else if trimmed.starts_with("function ") {
            if let Some(ref mut c) = current_contract {
                let sig = trimmed.split('{').next().unwrap_or("").trim().to_string();
                let fn_name = sig
                    .trim_start_matches("function ")
                    .split('(').next().unwrap_or("")
                    .to_string();
                c.functions.push(FunctionDoc {
                    name: fn_name,
                    signature: sig,
                    notice: pending_notice.take(),
                    dev: pending_dev.take(),
                    params: vec![],
                    returns: vec![],
                });
            }
        } else if trimmed.starts_with("event ") {
            if let Some(ref mut c) = current_contract {
                let name = trimmed.trim_start_matches("event ")
                    .split('(').next().unwrap_or("").trim().to_string();
                c.events.push(EventDoc { name, notice: pending_notice.take() });
            }
        }
    }

    if let Some(c) = current_contract { contracts.push(c); }
    contracts
}
