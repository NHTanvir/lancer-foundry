use std::sync::Arc;

use axum::{extract::State, routing::post, Json, Router};
use parking_lot::RwLock;
use serde_json::{json, Value};
use tokio::net::TcpListener;

use crate::anvil::{eth_api, anvil_api, state::SharedState};

pub async fn start(state: SharedState, host: &str, port: u16) -> eyre::Result<()> {
    let app = Router::new()
        .route("/", post(handle_rpc))
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("anvil listening on http://{addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_rpc(
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Json<Value> {
    // Support both single requests and batch arrays
    if body.is_array() {
        let responses: Vec<Value> = body
            .as_array()
            .unwrap()
            .iter()
            .map(|req| dispatch_single(state.clone(), req))
            .collect();
        return Json(Value::Array(responses));
    }
    Json(dispatch_single(state, &body))
}

fn dispatch_single(state: SharedState, req: &Value) -> Value {
    let id = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(Value::Array(vec![]));

    match handle_method(state, method, &params) {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": -32603, "message": e.to_string()}
        }),
    }
}

fn handle_method(state: SharedState, method: &str, params: &Value) -> eyre::Result<Value> {
    let p = params.as_array().map(|a| a.as_slice()).unwrap_or(&[]);

    match method {
        // ── Standard eth_* ────────────────────────────────────────────────
        "eth_chainId"             => eth_api::chain_id(&state),
        "eth_blockNumber"         => eth_api::block_number(&state),
        "eth_gasPrice"            => eth_api::gas_price(&state),
        "eth_accounts"            => eth_api::accounts(&state),
        "eth_getBalance"          => eth_api::get_balance(&state, p),
        "eth_getCode"             => eth_api::get_code(&state, p),
        "eth_getStorageAt"        => eth_api::get_storage_at(&state, p),
        "eth_getTransactionCount" => eth_api::get_transaction_count(&state, p),
        "eth_call"                => eth_api::call(&state, p),
        "eth_estimateGas"         => eth_api::estimate_gas(&state, p),
        "eth_sendRawTransaction"  => eth_api::send_raw_transaction(&state, p),
        "eth_sendTransaction"     => eth_api::send_transaction(&state, p),
        "eth_getTransactionByHash"    => eth_api::get_transaction_by_hash(&state, p),
        "eth_getTransactionReceipt"   => eth_api::get_transaction_receipt(&state, p),
        "eth_getBlockByNumber"        => eth_api::get_block_by_number(&state, p),
        "eth_getBlockByHash"          => eth_api::get_block_by_hash(&state, p),
        "eth_getLogs"                 => eth_api::get_logs(&state, p),
        "eth_feeHistory"              => eth_api::fee_history(&state, p),
        "net_version"                 => eth_api::net_version(&state),
        "net_listening"               => Ok(json!(true)),
        "eth_syncing"                 => Ok(json!(false)),
        "web3_clientVersion"          => Ok(json!("lancer-foundry/0.1.0")),
        "web3_sha3"                   => eth_api::web3_sha3(p),

        // ── anvil_* ─────────────────────────────────────────────────────
        "anvil_setBalance"            => anvil_api::set_balance(&state, p),
        "anvil_setCode"               => anvil_api::set_code(&state, p),
        "anvil_setStorageAt"          => anvil_api::set_storage_at(&state, p),
        "anvil_setNonce"              => anvil_api::set_nonce(&state, p),
        "anvil_impersonateAccount"    => anvil_api::impersonate_account(&state, p),
        "anvil_stopImpersonatingAccount" => anvil_api::stop_impersonating(&state, p),
        "anvil_mine"                  => anvil_api::mine(&state, p),
        "anvil_setNextBlockTimestamp" => anvil_api::set_next_block_timestamp(&state, p),
        "anvil_setBlockGasLimit"      => anvil_api::set_block_gas_limit(&state, p),
        "anvil_addBalance"            => anvil_api::add_balance(&state, p),
        "anvil_reset"                 => anvil_api::reset(&state, p),
        "anvil_dumpState"             => anvil_api::dump_state(&state),
        "anvil_loadState"             => anvil_api::load_state(&state, p),
        "hardhat_setBalance"          => anvil_api::set_balance(&state, p),
        "hardhat_setCode"             => anvil_api::set_code(&state, p),
        "hardhat_impersonateAccount"  => anvil_api::impersonate_account(&state, p),
        "hardhat_mine"                => anvil_api::mine(&state, p),

        _ => eyre::bail!("method not supported: {method}"),
    }
}
