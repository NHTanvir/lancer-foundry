use revm::{
    primitives::{
        Address, Bytes, ExecutionResult, Output, SpecId, TransactTo, U256, B256,
    },
    Evm,
};
use sha3::{Digest, Keccak256};

use crate::anvil::state::{AnvilState, StoredTransaction, StoredReceipt, StoredLog};
use crate::evm::executor::decode_revert_reason;
use crate::evm::types::EvmLog;

/// Execute a raw (already-decoded) transaction against the anvil state and
/// return the resulting tx hash.
pub fn execute_transaction(state: &mut AnvilState, tx: RawTx) -> eyre::Result<B256> {
    let from = tx.from;
    let nonce = {
        let acc = state.db.load_account(from)?;
        let n = acc.info.nonce;
        acc.info.nonce += 1;
        n
    };

    let gas_price = tx.gas_price.unwrap_or(state.gas_price);
    let gas_limit = tx.gas_limit.unwrap_or(state.block_gas_limit.min(21_000_000));
    let value = tx.value.unwrap_or(U256::ZERO);
    let data = tx.data.clone().unwrap_or_default();

    let chain_id = state.chain_id;
    let bn = state.block_number;
    let ts = state.block_timestamp;
    let base_fee = U256::from(state.base_fee);
    let gas_limit_block = U256::from(state.block_gas_limit);

    let transact_to = match tx.to {
        Some(a) => TransactTo::Call(a),
        None => TransactTo::Create,
    };

    // Deduct gas cost from sender
    let max_cost = U256::from(gas_limit) * U256::from(gas_price);
    {
        let acc = state.db.load_account(from)?;
        if acc.info.balance < max_cost + value {
            eyre::bail!("insufficient funds");
        }
        acc.info.balance -= max_cost;
    }

    let exec_result = {
        let mut evm = Evm::builder()
            .with_db(&mut state.db)
            .modify_cfg_env(|c| { c.chain_id = chain_id; })
            .modify_block_env(|b| {
                b.number = U256::from(bn);
                b.timestamp = U256::from(ts);
                b.gas_limit = gas_limit_block;
                b.basefee = base_fee;
            })
            .modify_tx_env(|t| {
                t.caller = from;
                t.transact_to = transact_to;
                t.value = value;
                t.data = data.clone();
                t.gas_limit = gas_limit;
                t.gas_price = U256::from(gas_price);
                t.chain_id = Some(chain_id);
                t.nonce = Some(nonce);
            })
            .build();
        evm.transact_commit()
    };

    let exec_result = exec_result.map_err(|e| eyre::eyre!("EVM error: {e:?}"))?;

    let (status, contract_address, gas_used, logs) = match exec_result {
        ExecutionResult::Success { gas_used, logs, output, .. } => {
            let contract_address = if tx.to.is_none() {
                if let Output::Create(_, Some(addr)) = output { Some(addr) } else { None }
            } else { None };
            let evm_logs: Vec<EvmLog> = logs.iter().map(|l| EvmLog {
                address: l.address,
                topics: l.data.topics().to_vec(),
                data: l.data.data.clone(),
            }).collect();
            (1u8, contract_address, gas_used, evm_logs)
        }
        ExecutionResult::Revert { gas_used, .. } => (0u8, None, gas_used, vec![]),
        ExecutionResult::Halt { gas_used, .. } => (0u8, None, gas_used, vec![]),
    };

    // Refund unused gas
    let used_cost = U256::from(gas_used) * U256::from(gas_price);
    {
        let acc = state.db.load_account(from).unwrap();
        acc.info.balance += max_cost - used_cost;
    }

    // Build tx hash: keccak256(from || nonce)
    let mut hash_input = Vec::new();
    hash_input.extend_from_slice(from.as_slice());
    hash_input.extend_from_slice(&nonce.to_be_bytes());
    let tx_hash = B256::from_slice(&Keccak256::digest(&hash_input));

    let tx_index = state.transactions.len() as u64;

    let stored_logs: Vec<StoredLog> = logs.into_iter().enumerate().map(|(i, l)| StoredLog {
        address: l.address,
        topics: l.topics,
        data: l.data,
        block_number: bn,
        block_hash: B256::ZERO,
        transaction_hash: tx_hash,
        log_index: i as u64,
    }).collect();

    state.logs.extend(stored_logs.clone());

    state.transactions.insert(tx_hash, StoredTransaction {
        hash: tx_hash,
        from,
        to: tx.to,
        value,
        input: data,
        gas_limit,
        gas_price,
        nonce,
        block_number: bn,
        block_hash: B256::ZERO,
        transaction_index: tx_index,
    });

    state.receipts.insert(tx_hash, StoredReceipt {
        transaction_hash: tx_hash,
        transaction_index: tx_index,
        block_hash: B256::ZERO,
        block_number: bn,
        from,
        to: tx.to,
        contract_address,
        gas_used,
        status,
        logs: stored_logs,
    });

    // Auto-mine: seal a new block immediately
    if state.auto_mine {
        state.mine_block(vec![tx_hash]);
    }

    Ok(tx_hash)
}

/// Execute eth_call (no state changes).
pub fn eth_call(state: &mut AnvilState, tx: RawTx) -> eyre::Result<Bytes> {
    let from = tx.from;
    let to = tx.to.unwrap_or(Address::ZERO);
    let value = tx.value.unwrap_or(U256::ZERO);
    let data = tx.data.clone().unwrap_or_default();
    let gas_limit = tx.gas_limit.unwrap_or(state.block_gas_limit);
    let chain_id = state.chain_id;
    let bn = state.block_number;
    let ts = state.block_timestamp;

    // Clone DB for read-only call
    let mut db_clone = state.db.clone();

    let result = Evm::builder()
        .with_db(&mut db_clone)
        .modify_cfg_env(|c| { c.chain_id = chain_id; })
        .modify_block_env(|b| {
            b.number = U256::from(bn);
            b.timestamp = U256::from(ts);
            b.gas_limit = U256::from(state.block_gas_limit);
        })
        .modify_tx_env(|t| {
            t.caller = from;
            t.transact_to = TransactTo::Call(to);
            t.value = value;
            t.data = data;
            t.gas_limit = gas_limit;
            t.gas_price = U256::from(state.gas_price);
        })
        .build()
        .transact()
        .map_err(|e| eyre::eyre!("call failed: {e:?}"))?;

    match result.result {
        ExecutionResult::Success { output, .. } => {
            Ok(match output { Output::Call(b) => b, Output::Create(b, _) => b })
        }
        ExecutionResult::Revert { output, .. } => {
            let reason = decode_revert_reason(&output);
            eyre::bail!("execution reverted: {reason}")
        }
        ExecutionResult::Halt { reason, .. } => eyre::bail!("execution halted: {reason:?}"),
    }
}

/// Estimate gas for a transaction.
pub fn estimate_gas(state: &mut AnvilState, tx: RawTx) -> eyre::Result<u64> {
    let from = tx.from;
    let to = tx.to.unwrap_or(Address::ZERO);
    let value = tx.value.unwrap_or(U256::ZERO);
    let data = tx.data.clone().unwrap_or_default();
    let chain_id = state.chain_id;
    let bn = state.block_number;
    let ts = state.block_timestamp;

    let mut db_clone = state.db.clone();

    let result = Evm::builder()
        .with_db(&mut db_clone)
        .modify_cfg_env(|c| { c.chain_id = chain_id; })
        .modify_block_env(|b| {
            b.number = U256::from(bn);
            b.timestamp = U256::from(ts);
            b.gas_limit = U256::from(state.block_gas_limit);
        })
        .modify_tx_env(|t| {
            t.caller = from;
            t.transact_to = TransactTo::Call(to);
            t.value = value;
            t.data = data;
            t.gas_limit = state.block_gas_limit;
            t.gas_price = U256::from(state.gas_price);
        })
        .build()
        .transact()
        .map_err(|e| eyre::eyre!("estimate failed: {e:?}"))?;

    let gas_used = match result.result {
        ExecutionResult::Success { gas_used, .. } => gas_used,
        ExecutionResult::Revert { gas_used, .. } => gas_used,
        ExecutionResult::Halt { gas_used, .. } => gas_used,
    };

    // Add 10% buffer
    Ok(gas_used * 110 / 100)
}

#[derive(Debug, Default)]
pub struct RawTx {
    pub from: Address,
    pub to: Option<Address>,
    pub value: Option<U256>,
    pub data: Option<Bytes>,
    pub gas_limit: Option<u64>,
    pub gas_price: Option<u128>,
    pub nonce: Option<u64>,
}

impl RawTx {
    pub fn from_json(v: &serde_json::Value) -> eyre::Result<Self> {
        Ok(RawTx {
            from: parse_addr(v.get("from").and_then(|x| x.as_str()).unwrap_or("0x0000000000000000000000000000000000000000"))?,
            to: v.get("to").and_then(|x| x.as_str()).map(parse_addr).transpose()?,
            value: v.get("value").and_then(|x| x.as_str()).map(parse_u256_hex).transpose()?,
            data: v.get("data").or_else(|| v.get("input"))
                .and_then(|x| x.as_str())
                .map(|s| Ok::<_, eyre::Error>(Bytes::from(hex::decode(s.trim_start_matches("0x")).unwrap_or_default()))).transpose()?,
            gas_limit: v.get("gas").and_then(|x| x.as_str()).map(parse_u64_hex).transpose()?,
            gas_price: v.get("gasPrice").and_then(|x| x.as_str()).map(|s| Ok::<_, eyre::Error>(parse_u128_hex(s)?)).transpose()?,
            nonce: v.get("nonce").and_then(|x| x.as_str()).map(parse_u64_hex).transpose()?,
        })
    }
}

fn parse_addr(s: &str) -> eyre::Result<Address> {
    let s = s.trim_start_matches("0x");
    let b = hex::decode(format!("{:0>40}", s))?;
    Ok(Address::from_slice(&b))
}

fn parse_u256_hex(s: &str) -> eyre::Result<U256> {
    let s = s.trim_start_matches("0x");
    Ok(U256::from_str_radix(s, 16)?)
}

fn parse_u64_hex(s: &str) -> eyre::Result<u64> {
    let s = s.trim_start_matches("0x");
    Ok(u64::from_str_radix(s, 16)?)
}

fn parse_u128_hex(s: &str) -> eyre::Result<u128> {
    let s = s.trim_start_matches("0x");
    Ok(u128::from_str_radix(s, 16)?)
}
