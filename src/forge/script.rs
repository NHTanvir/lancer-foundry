use std::path::PathBuf;
use revm::primitives::{Address, Bytes, U256};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::evm::executor::Executor;
use crate::forge::compiler;

/// forge script — deploy and run Solidity scripts with vm.broadcast().
pub struct ForgeScript {
    pub root: PathBuf,
    pub rpc_url: Option<String>,
    pub broadcast: bool,
    pub private_key: Option<String>,
    pub chain_id: u64,
    pub verify: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastTransaction {
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub data: Bytes,
    pub gas_limit: u64,
    pub hash: Option<String>,
}

#[derive(Debug, Default)]
pub struct ScriptResult {
    pub transactions: Vec<BroadcastTransaction>,
    pub return_value: Option<String>,
    pub gas_used: u64,
}

impl ForgeScript {
    pub fn run(&self, script_path: &str, sig: &str, args: &[String]) -> eyre::Result<ScriptResult> {
        // 1. Compile the script
        let script_file = PathBuf::from(script_path);
        if !script_file.exists() {
            eyre::bail!("script file not found: {script_path}");
        }

        let compiled = compiler::compile(
            &[script_file],
            &[self.root.clone()],
            &[],
            true,
            200,
            false,
        )?;

        // Find the script contract
        let script_contract = compiled.values()
            .find(|c| c.name.contains("Script") || script_path.contains(&c.name))
            .ok_or_else(|| eyre::eyre!("no script contract found in {script_path}"))?;

        // 2. Deploy to local EVM
        let mut executor = Executor::new(self.chain_id, 1);

        // Set up sender address
        let sender = if let Some(ref pk) = self.private_key {
            derive_sender(pk)?
        } else {
            Address::from([0xf3, 0x9f, 0xd6, 0xe5, 0x1a, 0xad, 0x88, 0xf6, 0xf4, 0xce,
                           0x6a, 0xb8, 0x82, 0x72, 0x79, 0xcf, 0xff, 0xb9, 0x22, 0x66]) // hardhat default
        };
        executor.set_balance(sender, U256::from(10u128.pow(21)));

        let deploy_result = executor.deploy(
            sender,
            Bytes::copy_from_slice(&script_contract.bytecode),
            U256::ZERO,
        )?;

        // 3. Call run() or the specified function
        let fn_name = if sig.is_empty() { "run" } else { sig.split('(').next().unwrap_or("run") };
        let calldata = build_calldata(fn_name, args, &script_contract.abi)?;

        let exec_result = executor.execute_tx(
            sender,
            Some(deploy_result.address),
            U256::ZERO,
            Bytes::from(calldata),
        )?;

        if !exec_result.success {
            let reason = exec_result.revert_reason.unwrap_or_else(|| "reverted".into());
            eyre::bail!("script execution failed: {reason}");
        }

        // 4. Collect broadcast transactions from cheatcode state
        let broadcasts: Vec<BroadcastTransaction> = {
            let cc = executor.cheatcode_state.lock().unwrap();
            cc.broadcast_txs.iter().map(|tx| BroadcastTransaction {
                from: tx.from,
                to: tx.to,
                value: tx.value,
                data: tx.data.clone(),
                gas_limit: tx.gas_limit,
                hash: None,
            }).collect()
        };

        // 5. If --broadcast, send to RPC
        let mut script_result = ScriptResult {
            transactions: broadcasts.clone(),
            return_value: Some(format!("0x{}", hex::encode(&exec_result.output))),
            gas_used: exec_result.gas_used,
        };

        if self.broadcast {
            if let Some(ref rpc) = self.rpc_url {
                for tx in &mut script_result.transactions {
                    let hash = broadcast_tx(rpc, tx, self.private_key.as_deref())?;
                    tx.hash = Some(hash);
                }
                println!("Broadcast {} transaction(s)", script_result.transactions.len());
            } else {
                eyre::bail!("--broadcast requires --rpc-url");
            }
        }

        // Print the sequence
        for tx in &script_result.transactions {
            if self.broadcast {
                if let Some(ref h) = tx.hash {
                    println!("  tx hash: {h}");
                }
            } else {
                println!("  [dry run] {:?} → {:?} value={}", tx.from, tx.to, tx.value);
            }
        }

        Ok(script_result)
    }
}

fn derive_sender(private_key: &str) -> eyre::Result<Address> {
    use k256::ecdsa::SigningKey;
    use sha3::{Digest, Keccak256};
    let key_bytes = hex::decode(private_key.trim_start_matches("0x"))?;
    let key: [u8; 32] = key_bytes.try_into().map_err(|_| eyre::eyre!("private key must be 32 bytes"))?;
    let signing_key = SigningKey::from_bytes(&key.into()).map_err(|e| eyre::eyre!("{e}"))?;
    let vk = signing_key.verifying_key();
    let pub_bytes = vk.to_encoded_point(false);
    let hash = Keccak256::digest(&pub_bytes.as_bytes()[1..]);
    Ok(Address::from_slice(&hash[12..]))
}

fn build_calldata(fn_name: &str, args: &[String], abi: &serde_json::Value) -> eyre::Result<Vec<u8>> {
    use sha3::{Digest, Keccak256};
    // Simple: just call fn_name() with no args for now
    let sig = format!("{fn_name}()");
    let hash = Keccak256::digest(sig.as_bytes());
    Ok(hash[..4].to_vec())
}

fn broadcast_tx(rpc: &str, tx: &BroadcastTransaction, private_key: Option<&str>) -> eyre::Result<String> {
    let client = reqwest::blocking::Client::new();
    let body = json!({
        "jsonrpc": "2.0",
        "method": "eth_sendTransaction",
        "params": [{
            "from": format!("{:#x}", tx.from),
            "to": tx.to.map(|a| format!("{a:#x}")),
            "value": format!("{:#x}", tx.value),
            "data": format!("0x{}", hex::encode(&tx.data)),
            "gas": format!("0x{:x}", tx.gas_limit),
        }],
        "id": 1
    });
    let resp: serde_json::Value = client.post(rpc).json(&body).send()?.json()?;
    if let Some(err) = resp.get("error") {
        eyre::bail!("RPC error: {err}");
    }
    Ok(resp["result"].as_str().unwrap_or("0x0").to_string())
}
