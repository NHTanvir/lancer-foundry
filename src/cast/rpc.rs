use eyre::eyre;
use serde_json::{json, Value};

pub struct JsonRpc {
    client: reqwest::Client,
    url: String,
}

impl JsonRpc {
    pub fn new(url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: url.to_string(),
        }
    }

    pub async fn call(&self, method: &str, params: Value) -> eyre::Result<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params
        });

        let resp = self
            .client
            .post(&self.url)
            .json(&body)
            .send()
            .await?
            .json::<Value>()
            .await?;

        if let Some(err) = resp.get("error") {
            return Err(eyre!("JSON-RPC error: {}", err));
        }

        resp.get("result")
            .cloned()
            .ok_or_else(|| eyre!("missing 'result' in JSON-RPC response"))
    }
}

pub fn default_rpc_url() -> String {
    std::env::var("ETH_RPC_URL").unwrap_or_else(|_| "http://localhost:8545".into())
}
