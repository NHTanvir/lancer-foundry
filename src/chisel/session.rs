use revm::primitives::{Address, U256};
use crate::evm::executor::Executor;

/// A chisel REPL session. Holds the EVM state and source code history.
pub struct ChiselSession {
    pub executor: Executor,
    pub contract_address: Option<Address>,
    pub history: Vec<String>,
    pub imports: Vec<String>,
    pub run_code: Vec<String>,
    pub fork_url: Option<String>,
    pub pragma: String,
}

impl ChiselSession {
    pub fn new(fork_url: Option<String>) -> Self {
        ChiselSession {
            executor: Executor::new(1, 0),
            contract_address: None,
            history: vec![],
            imports: vec![],
            run_code: vec![],
            fork_url,
            pragma: "^0.8.13".to_string(),
        }
    }

    /// Build a complete Solidity source that wraps the user's input.
    pub fn build_source(&self, new_line: &str) -> String {
        let mut src = format!(
            "// SPDX-License-Identifier: UNLICENSED\npragma solidity {};\n\n",
            self.pragma
        );
        for import in &self.imports {
            src.push_str(&format!("import {import};\n"));
        }
        src.push_str("\ncontract ChiselRunner {\n");
        src.push_str("    function run() public returns (bytes memory) {\n");
        for line in &self.run_code {
            src.push_str(&format!("        {line}\n"));
        }
        src.push_str(&format!("        {new_line}\n"));
        src.push_str("        return \"\";\n");
        src.push_str("    }\n");
        src.push_str("}\n");
        src
    }

    /// Execute a line of Solidity code and return the result.
    pub fn execute_line(&mut self, line: &str) -> eyre::Result<String> {
        use crate::forge::compiler;
        use std::io::Write;

        // Write source to temp file
        let source = self.build_source(line);
        let tmp_dir = tempfile::tempdir()?;
        let src_path = tmp_dir.path().join("Chisel.sol");
        std::fs::write(&src_path, &source)?;

        // Compile
        let compiled = compiler::compile(
            &[src_path],
            &[tmp_dir.path().to_path_buf()],
            &[],
            true,
            200,
            false,
        );

        match compiled {
            Err(e) => return Ok(format!("Compilation error: {e}")),
            Ok(compiled) => {
                let contract = compiled.values()
                    .find(|c| c.name == "ChiselRunner")
                    .ok_or_else(|| eyre::eyre!("ChiselRunner not found"))?;

                // Deploy or redeploy
                let deploy_result = self.executor.deploy(
                    Address::ZERO,
                    revm::primitives::Bytes::copy_from_slice(&contract.bytecode),
                    U256::ZERO,
                )?;
                self.contract_address = Some(deploy_result.address);

                // Call run()
                use sha3::{Digest, Keccak256};
                let selector = &Keccak256::digest(b"run()")[..4];
                let result = self.executor.execute_tx(
                    Address::ZERO,
                    Some(deploy_result.address),
                    U256::ZERO,
                    revm::primitives::Bytes::copy_from_slice(selector),
                )?;

                if result.success {
                    self.run_code.push(line.to_string());
                    self.history.push(line.to_string());
                    if result.output.is_empty() {
                        Ok("(ok)".to_string())
                    } else {
                        Ok(format!("0x{}", hex::encode(&result.output)))
                    }
                } else {
                    Ok(format!("Reverted: {}", result.revert_reason.unwrap_or_else(|| "unknown".into())))
                }
            }
        }
    }

    pub fn clear(&mut self) {
        self.run_code.clear();
        self.contract_address = None;
    }

    pub fn show_source(&self) -> String {
        self.build_source("// (current state)")
    }
}
