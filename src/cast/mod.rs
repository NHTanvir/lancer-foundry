pub mod abi;
pub mod convert;
pub mod rpc;
pub mod sign_ops;
pub mod wallet;

use clap::Parser;

use crate::cast::rpc::{default_rpc_url, JsonRpc};

// ── Top-level argument struct ─────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "cast", version)]
pub struct CastArgs {
    #[command(subcommand)]
    pub command: CastCommand,
}

// ── Subcommand enum ───────────────────────────────────────────────────────────

#[derive(clap::Subcommand, Debug)]
pub enum CastCommand {
    /// Wallet operations (new, sign, address)
    Wallet(wallet::WalletArgs),

    // ── Week 2: read-only RPC ─────────────────────────────────────────────
    /// Call a contract read function
    Call(CallArgs),
    /// Get account ETH balance
    Balance(BalanceArgs),
    /// Get block information
    Block(BlockArgs),
    /// Get contract bytecode
    Code(CodeArgs),
    /// Get storage slot value
    Storage(StorageArgs),
    /// Get account nonce
    Nonce(NonceArgs),
    /// Get transaction by hash
    Tx(TxArgs),
    /// Get transaction receipt
    Receipt(ReceiptArgs),
    /// Get logs matching a filter
    Logs(LogsArgs),
    /// ABI-encode call data
    #[command(name = "abi-encode")]
    AbiEncode(AbiEncodeArgs),
    /// ABI-decode return data
    #[command(name = "abi-decode")]
    AbiDecode(AbiDecodeArgs),
    /// Compute function selector
    Sig(SigArgs),
    /// Look up 4-byte selector in the database
    #[command(name = "4byte")]
    FourByte(FourByteArgs),
    /// Keccak-256 hash of input
    Keccak(KeccakArgs),
    /// Convert a value to wei
    #[command(name = "to-wei")]
    ToWei(ToWeiArgs),
    /// Convert wei to a human unit
    #[command(name = "from-wei")]
    FromWei(FromWeiArgs),
    /// Convert a value to hex
    #[command(name = "to-hex")]
    ToHex(ToHexArgs),

    // ── Week 3: signing / tx ──────────────────────────────────────────────
    /// Sign a message with a local private key
    Sign(SignArgs),
    /// Build a raw transaction (do not broadcast)
    Mktx(MktxArgs),
    /// Build and send a transaction
    Send(SendArgs),
    /// Estimate gas for a call
    Estimate(EstimateArgs),
    /// Compute a CREATE2 deployment address
    Create2(Create2Args),
}

// ── Argument structs ──────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
pub struct CallArgs {
    /// Contract address to call
    pub to: String,
    /// Function signature, e.g. "balanceOf(address)"
    pub sig: String,
    /// Arguments to pass
    pub args: Vec<String>,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
    #[arg(long, default_value = "")]
    pub from: String,
    /// Value in wei to send with the call
    #[arg(long, default_value = "0")]
    pub value: String,
}

#[derive(Parser, Debug)]
pub struct BalanceArgs {
    /// Ethereum address
    pub address: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
    /// Display balance in ETH instead of wei
    #[arg(long)]
    pub ether: bool,
}

#[derive(Parser, Debug)]
pub struct BlockArgs {
    /// Block number or tag (latest/pending/earliest)
    #[arg(default_value = "latest")]
    pub block: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
    /// Output raw JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct CodeArgs {
    /// Contract address
    pub address: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
}

#[derive(Parser, Debug)]
pub struct StorageArgs {
    /// Contract address
    pub address: String,
    /// Storage slot (decimal or 0x hex)
    pub slot: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
}

#[derive(Parser, Debug)]
pub struct NonceArgs {
    /// Ethereum address
    pub address: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
}

#[derive(Parser, Debug)]
pub struct TxArgs {
    /// Transaction hash
    pub hash: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
    /// Output raw JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ReceiptArgs {
    /// Transaction hash
    pub hash: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
    /// Output raw JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct LogsArgs {
    #[arg(long)]
    pub from_block: Option<String>,
    #[arg(long)]
    pub to_block: Option<String>,
    #[arg(long)]
    pub address: Option<String>,
    #[arg(long)]
    pub topic: Vec<String>,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
}

#[derive(Parser, Debug)]
pub struct AbiEncodeArgs {
    /// Signature, e.g. "foo(uint256,address)" or "(uint256,address)"
    pub sig: String,
    /// Arguments
    pub args: Vec<String>,
}

#[derive(Parser, Debug)]
pub struct AbiDecodeArgs {
    /// Comma-separated types, e.g. "uint256,address"
    pub types: String,
    /// 0x-prefixed encoded data
    pub data: String,
}

#[derive(Parser, Debug)]
pub struct SigArgs {
    /// Function signature, e.g. "transfer(address,uint256)"
    pub sig: String,
}

#[derive(Parser, Debug)]
pub struct FourByteArgs {
    /// 4-byte selector (0x-prefixed hex)
    pub selector: String,
}

#[derive(Parser, Debug)]
pub struct KeccakArgs {
    /// Input string to hash
    pub input: String,
}

#[derive(Parser, Debug)]
pub struct ToWeiArgs {
    /// Value to convert
    pub value: String,
    /// Unit: ether|gwei|wei
    #[arg(default_value = "ether")]
    pub unit: String,
}

#[derive(Parser, Debug)]
pub struct FromWeiArgs {
    /// Wei value to convert
    pub value: String,
    /// Unit: ether|gwei|wei
    #[arg(default_value = "ether")]
    pub unit: String,
}

#[derive(Parser, Debug)]
pub struct ToHexArgs {
    /// Decimal or 0x-hex value
    pub value: String,
}

#[derive(Parser, Debug)]
pub struct SignArgs {
    /// Message to sign
    pub message: String,
    #[arg(long)]
    pub private_key: String,
    /// Skip EIP-191 prefixing (sign raw keccak of the bytes)
    #[arg(long)]
    pub no_hash: bool,
}

#[derive(Parser, Debug)]
pub struct MktxArgs {
    #[arg(long)]
    pub to: Option<String>,
    #[arg(long, default_value = "0")]
    pub value: String,
    #[arg(long, default_value = "0x")]
    pub data: String,
    #[arg(long)]
    pub private_key: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
}

#[derive(Parser, Debug)]
pub struct SendArgs {
    #[arg(long)]
    pub to: Option<String>,
    #[arg(long, default_value = "0")]
    pub value: String,
    #[arg(long, default_value = "0x")]
    pub data: String,
    #[arg(long)]
    pub private_key: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
}

#[derive(Parser, Debug)]
pub struct EstimateArgs {
    #[arg(long, default_value = "")]
    pub to: String,
    #[arg(long, default_value = "0x")]
    pub data: String,
    #[arg(long, default_value = "")]
    pub from: String,
    #[arg(long, default_value_t = default_rpc_url())]
    pub rpc_url: String,
}

#[derive(Parser, Debug)]
pub struct Create2Args {
    /// CREATE2 factory deployer address
    #[arg(long, default_value = "0x4e59b44847b379578588920cA78FbF26c0B4956C")]
    pub deployer: String,
    /// Salt (0x-prefixed 32-byte hex or shorter)
    #[arg(long)]
    pub salt: String,
    /// Init code (0x-prefixed hex)
    #[arg(long)]
    pub init_code: String,
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn run(args: CastArgs) -> eyre::Result<()> {
    match args.command {
        CastCommand::Wallet(a) => wallet::run(a),

        // ── Week 2 ───────────────────────────────────────────────────────
        CastCommand::Call(a) => cmd_call(a).await,
        CastCommand::Balance(a) => cmd_balance(a).await,
        CastCommand::Block(a) => cmd_block(a).await,
        CastCommand::Code(a) => cmd_code(a).await,
        CastCommand::Storage(a) => cmd_storage(a).await,
        CastCommand::Nonce(a) => cmd_nonce(a).await,
        CastCommand::Tx(a) => cmd_tx(a).await,
        CastCommand::Receipt(a) => cmd_receipt(a).await,
        CastCommand::Logs(a) => cmd_logs(a).await,
        CastCommand::AbiEncode(a) => cmd_abi_encode(a),
        CastCommand::AbiDecode(a) => cmd_abi_decode(a),
        CastCommand::Sig(a) => cmd_sig(a),
        CastCommand::FourByte(a) => cmd_4byte(a).await,
        CastCommand::Keccak(a) => cmd_keccak(a),
        CastCommand::ToWei(a) => cmd_to_wei(a),
        CastCommand::FromWei(a) => cmd_from_wei(a),
        CastCommand::ToHex(a) => cmd_to_hex(a),

        // ── Week 3 ───────────────────────────────────────────────────────
        CastCommand::Sign(a) => cmd_sign(a),
        CastCommand::Mktx(a) => cmd_mktx(a).await,
        CastCommand::Send(a) => cmd_send(a).await,
        CastCommand::Estimate(a) => cmd_estimate(a).await,
        CastCommand::Create2(a) => cmd_create2(a),
    }
}

// ── Command implementations ───────────────────────────────────────────────────

async fn cmd_call(a: CallArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);

    // ABI-encode the call data
    let data = abi::abi_encode(&a.sig, &a.args)?;

    let mut tx = serde_json::json!({
        "to": a.to,
        "data": data,
    });
    if !a.from.is_empty() {
        tx["from"] = serde_json::json!(a.from);
    }
    if a.value != "0" {
        tx["value"] = serde_json::json!(format!("0x{:x}", a.value.parse::<u128>().unwrap_or(0)));
    }

    let result = client
        .call("eth_call", serde_json::json!([tx, "latest"]))
        .await?;
    println!("{}", result.as_str().unwrap_or(&result.to_string()));
    Ok(())
}

async fn cmd_balance(a: BalanceArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);
    let result = client
        .call("eth_getBalance", serde_json::json!([a.address, "latest"]))
        .await?;
    let hex_str = result
        .as_str()
        .ok_or_else(|| eyre::eyre!("unexpected balance response"))?;

    if a.ether {
        let wei = convert::parse_u256(hex_str)?;
        let divisor = alloy_primitives::U256::from(1_000_000_000_000_000_000u128);
        println!("{}", wei / divisor);
    } else {
        println!("{}", hex_str);
    }
    Ok(())
}

async fn cmd_block(a: BlockArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);

    // Normalise block tag
    let block_param = if a.block.starts_with("0x") || a.block.parse::<u64>().is_ok() {
        let num = if a.block.starts_with("0x") {
            u64::from_str_radix(a.block.trim_start_matches("0x"), 16).unwrap_or(0)
        } else {
            a.block.parse::<u64>().unwrap_or(0)
        };
        serde_json::json!(format!("0x{:x}", num))
    } else {
        serde_json::json!(a.block)
    };

    let result = client
        .call(
            "eth_getBlockByNumber",
            serde_json::json!([block_param, false]),
        )
        .await?;

    if a.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        pretty_block(&result);
    }
    Ok(())
}

fn pretty_block(block: &serde_json::Value) {
    let fields = [
        "number",
        "hash",
        "timestamp",
        "gasLimit",
        "gasUsed",
        "miner",
        "parentHash",
        "difficulty",
        "totalDifficulty",
        "size",
    ];
    for f in &fields {
        if let Some(v) = block.get(f) {
            println!("{}: {}", f, v);
        }
    }
    if let Some(txs) = block.get("transactions") {
        if let Some(arr) = txs.as_array() {
            println!("transactions: {}", arr.len());
        }
    }
}

async fn cmd_code(a: CodeArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);
    let result = client
        .call("eth_getCode", serde_json::json!([a.address, "latest"]))
        .await?;
    println!("{}", result.as_str().unwrap_or(&result.to_string()));
    Ok(())
}

async fn cmd_storage(a: StorageArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);
    // Normalise slot to 0x hex
    let slot_hex = if a.slot.starts_with("0x") || a.slot.starts_with("0X") {
        a.slot.clone()
    } else {
        let n: u64 = a
            .slot
            .parse()
            .map_err(|_| eyre::eyre!("invalid slot: {}", a.slot))?;
        format!("0x{:x}", n)
    };
    let result = client
        .call(
            "eth_getStorageAt",
            serde_json::json!([a.address, slot_hex, "latest"]),
        )
        .await?;
    println!("{}", result.as_str().unwrap_or(&result.to_string()));
    Ok(())
}

async fn cmd_nonce(a: NonceArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);
    let result = client
        .call(
            "eth_getTransactionCount",
            serde_json::json!([a.address, "latest"]),
        )
        .await?;
    println!("{}", result.as_str().unwrap_or(&result.to_string()));
    Ok(())
}

async fn cmd_tx(a: TxArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);
    let result = client
        .call(
            "eth_getTransactionByHash",
            serde_json::json!([a.hash]),
        )
        .await?;
    if a.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        pretty_tx(&result);
    }
    Ok(())
}

fn pretty_tx(tx: &serde_json::Value) {
    let fields = [
        "hash",
        "from",
        "to",
        "value",
        "gasPrice",
        "gas",
        "nonce",
        "input",
        "blockNumber",
        "blockHash",
        "transactionIndex",
    ];
    for f in &fields {
        if let Some(v) = tx.get(f) {
            println!("{}: {}", f, v);
        }
    }
}

async fn cmd_receipt(a: ReceiptArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);
    let result = client
        .call(
            "eth_getTransactionReceipt",
            serde_json::json!([a.hash]),
        )
        .await?;
    if a.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
    } else {
        pretty_receipt(&result);
    }
    Ok(())
}

fn pretty_receipt(r: &serde_json::Value) {
    let fields = [
        "transactionHash",
        "blockNumber",
        "blockHash",
        "from",
        "to",
        "contractAddress",
        "status",
        "gasUsed",
        "cumulativeGasUsed",
        "logsBloom",
    ];
    for f in &fields {
        if let Some(v) = r.get(f) {
            println!("{}: {}", f, v);
        }
    }
    if let Some(logs) = r.get("logs") {
        if let Some(arr) = logs.as_array() {
            println!("logs: {}", arr.len());
        }
    }
}

async fn cmd_logs(a: LogsArgs) -> eyre::Result<()> {
    let client = JsonRpc::new(&a.rpc_url);
    let mut filter = serde_json::json!({});
    if let Some(fb) = a.from_block {
        filter["fromBlock"] = serde_json::json!(fb);
    }
    if let Some(tb) = a.to_block {
        filter["toBlock"] = serde_json::json!(tb);
    }
    if let Some(addr) = a.address {
        filter["address"] = serde_json::json!(addr);
    }
    if !a.topic.is_empty() {
        filter["topics"] = serde_json::json!(a.topic);
    }
    let result = client
        .call("eth_getLogs", serde_json::json!([filter]))
        .await?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn cmd_abi_encode(a: AbiEncodeArgs) -> eyre::Result<()> {
    let encoded = abi::abi_encode(&a.sig, &a.args)?;
    println!("{}", encoded);
    Ok(())
}

fn cmd_abi_decode(a: AbiDecodeArgs) -> eyre::Result<()> {
    let decoded = abi::abi_decode(&a.types, &a.data)?;
    println!("{}", decoded);
    Ok(())
}

fn cmd_sig(a: SigArgs) -> eyre::Result<()> {
    let selector = abi::function_selector(&a.sig);
    println!("{}", selector);
    Ok(())
}

async fn cmd_4byte(a: FourByteArgs) -> eyre::Result<()> {
    let url = format!(
        "https://api.openchain.xyz/signature-database/v1/lookup?function={}",
        a.selector
    );
    let resp = reqwest::get(&url)
        .await?
        .json::<serde_json::Value>()
        .await?;

    // Response format: {"ok":true,"result":{"function":{"0xaabbccdd":[{"name":"...","filtered":false}]}}}
    let found = resp
        .pointer("/result/function")
        .and_then(|f| f.as_object())
        .and_then(|m| {
            // Get first key's array
            m.values().next().and_then(|v| v.as_array().cloned())
        });

    if let Some(sigs) = found {
        for sig in sigs {
            if let Some(name) = sig.get("name").and_then(|n| n.as_str()) {
                println!("{}", name);
            }
        }
    } else {
        println!("No matching signatures found.");
    }
    Ok(())
}

fn cmd_keccak(a: KeccakArgs) -> eyre::Result<()> {
    println!("{}", convert::keccak256(&a.input));
    Ok(())
}

fn cmd_to_wei(a: ToWeiArgs) -> eyre::Result<()> {
    println!("{}", convert::to_wei(&a.value, &a.unit)?);
    Ok(())
}

fn cmd_from_wei(a: FromWeiArgs) -> eyre::Result<()> {
    println!("{}", convert::from_wei(&a.value, &a.unit)?);
    Ok(())
}

fn cmd_to_hex(a: ToHexArgs) -> eyre::Result<()> {
    println!("{}", convert::to_hex(&a.value)?);
    Ok(())
}

fn cmd_sign(a: SignArgs) -> eyre::Result<()> {
    let sig = sign_ops::wallet_sign(&a.message, &a.private_key, a.no_hash)?;
    println!("{}", sig);
    Ok(())
}

async fn cmd_mktx(a: MktxArgs) -> eyre::Result<()> {
    let raw = sign_ops::build_and_send(
        a.to.as_deref(),
        &a.value,
        &a.data,
        &a.private_key,
        &a.rpc_url,
        false,
    )
    .await?;
    println!("{}", raw);
    Ok(())
}

async fn cmd_send(a: SendArgs) -> eyre::Result<()> {
    let hash = sign_ops::build_and_send(
        a.to.as_deref(),
        &a.value,
        &a.data,
        &a.private_key,
        &a.rpc_url,
        true,
    )
    .await?;
    println!("{}", hash);
    Ok(())
}

async fn cmd_estimate(a: EstimateArgs) -> eyre::Result<()> {
    let gas = sign_ops::estimate_gas(&a.to, &a.data, &a.from, &a.rpc_url).await?;
    println!("{}", gas);
    Ok(())
}

fn cmd_create2(a: Create2Args) -> eyre::Result<()> {
    let addr = sign_ops::create2_address(&a.deployer, &a.salt, &a.init_code)?;
    println!("{}", addr);
    Ok(())
}
