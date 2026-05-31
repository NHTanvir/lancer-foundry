use revm::primitives::{Address, Bytes, U256};
use serde_json::{json, Value};

use crate::anvil::state::SharedState;

fn addr_from_str(s: &str) -> eyre::Result<Address> {
    let s = s.trim_start_matches("0x");
    let b = hex::decode(format!("{:0>40}", s))?;
    Ok(Address::from_slice(&b))
}

fn u256_from_hex(s: &str) -> U256 {
    let s = s.trim_start_matches("0x");
    U256::from_str_radix(s, 16).unwrap_or(U256::ZERO)
}

pub fn set_balance(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let val = u256_from_hex(params[1].as_str().unwrap_or("0x0"));
    let mut s = state.write();
    let acc = s.db.load_account(addr)?;
    acc.info.balance = val;
    Ok(json!(null))
}

pub fn add_balance(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let val = u256_from_hex(params[1].as_str().unwrap_or("0x0"));
    let mut s = state.write();
    let acc = s.db.load_account(addr)?;
    acc.info.balance += val;
    Ok(json!(null))
}

pub fn set_code(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let code_hex = params[1].as_str().unwrap_or("0x");
    let code_bytes = hex::decode(code_hex.trim_start_matches("0x")).unwrap_or_default();
    let mut s = state.write();
    let bytecode = revm::primitives::Bytecode::new_raw(Bytes::copy_from_slice(&code_bytes));
    let hash = bytecode.hash_slow();
    let acc = s.db.load_account(addr)?;
    acc.info.code_hash = hash;
    acc.info.code = Some(bytecode.clone());
    s.db.contracts.insert(hash, bytecode);
    Ok(json!(null))
}

pub fn set_storage_at(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let slot = u256_from_hex(params[1].as_str().unwrap_or("0x0"));
    let val = u256_from_hex(params[2].as_str().unwrap_or("0x0"));
    let mut s = state.write();
    let acc = s.db.load_account(addr)?;
    acc.storage.insert(slot, val);
    Ok(json!(null))
}

pub fn set_nonce(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    let nonce_val = params[1].as_str().unwrap_or("0x0");
    let nonce = u64::from_str_radix(nonce_val.trim_start_matches("0x"), 16).unwrap_or(0);
    let mut s = state.write();
    let acc = s.db.load_account(addr)?;
    acc.info.nonce = nonce;
    Ok(json!(null))
}

pub fn impersonate_account(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    state.write().impersonated.insert(addr);
    Ok(json!(null))
}

pub fn stop_impersonating(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let addr = addr_from_str(params[0].as_str().unwrap_or("0x0"))?;
    state.write().impersonated.remove(&addr);
    Ok(json!(null))
}

pub fn mine(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let count = params.first()
        .and_then(|v| v.as_str())
        .and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
        .unwrap_or(1);
    let mut s = state.write();
    for _ in 0..count {
        s.mine_block(vec![]);
    }
    Ok(json!(null))
}

pub fn set_next_block_timestamp(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let ts = params[0].as_u64()
        .or_else(|| params[0].as_str().and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok()))
        .unwrap_or(0);
    state.write().next_block_timestamp = Some(ts);
    Ok(json!(null))
}

pub fn set_block_gas_limit(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    let gl = params[0].as_str().unwrap_or("0x1c9c380");
    let val = u64::from_str_radix(gl.trim_start_matches("0x"), 16).unwrap_or(30_000_000);
    state.write().block_gas_limit = val;
    Ok(json!(null))
}

pub fn reset(state: &SharedState, _params: &[Value]) -> eyre::Result<Value> {
    // TODO: re-fork or clear state — for now just clear transactions
    let mut s = state.write();
    s.transactions.clear();
    s.receipts.clear();
    s.logs.clear();
    Ok(json!(null))
}

pub fn dump_state(state: &SharedState) -> eyre::Result<Value> {
    let s = state.read();
    // Serialize accounts
    let mut accounts = serde_json::Map::new();
    for (addr, acc_db) in &s.db.accounts {
        accounts.insert(format!("{addr:#x}"), json!({
            "balance": format!("{:#x}", acc_db.info.balance),
            "nonce": acc_db.info.nonce,
        }));
    }
    Ok(json!({"accounts": accounts, "blockNumber": s.block_number}))
}

pub fn load_state(state: &SharedState, params: &[Value]) -> eyre::Result<Value> {
    // Restore balances from a dump
    if let Some(accounts) = params[0].get("accounts").and_then(|v| v.as_object()) {
        let mut s = state.write();
        for (addr_str, info) in accounts {
            if let Ok(addr) = addr_str.parse::<Address>() {
                let balance = u256_from_hex(info["balance"].as_str().unwrap_or("0x0"));
                let nonce = info["nonce"].as_u64().unwrap_or(0);
                let acc = s.db.load_account(addr)?;
                acc.info.balance = balance;
                acc.info.nonce = nonce;
            }
        }
    }
    Ok(json!(true))
}
