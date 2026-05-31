pub mod accounts;
pub mod anvil_api;
pub mod backend;
pub mod eth_api;
pub mod rpc;
pub mod state;

use std::sync::Arc;
use clap::Parser;
use parking_lot::RwLock;

use accounts::{derive_accounts, private_key_hex};
use state::{AnvilState, SharedState};

const DEFAULT_MNEMONIC: &str =
    "test test test test test test test test test test test junk";

#[derive(Parser, Debug)]
pub struct AnvilArgs {
    /// RPC endpoint to fork from
    #[arg(long)]
    pub fork_url: Option<String>,
    /// Block number to fork at
    #[arg(long)]
    pub fork_block_number: Option<u64>,
    /// Override fork chain ID
    #[arg(long)]
    pub fork_chain_id: Option<u64>,
    /// Chain ID (default 31337)
    #[arg(long, default_value = "31337")]
    pub chain_id: u64,
    /// Number of dev accounts (default 10)
    #[arg(short, long, default_value = "10")]
    pub accounts: usize,
    /// BIP-39 mnemonic for dev accounts
    #[arg(long, default_value = DEFAULT_MNEMONIC)]
    pub mnemonic: String,
    /// Dev account balance in ETH (default 10000)
    #[arg(long, default_value = "10000")]
    pub balance: u128,
    /// Block gas limit
    #[arg(long, default_value = "30000000")]
    pub gas_limit: u64,
    /// Gas price in wei
    #[arg(long, default_value = "1000000000")]
    pub gas_price: u128,
    /// Block time in seconds (auto-mine if not set)
    #[arg(long)]
    pub block_time: Option<u64>,
    /// Port (default 8545)
    #[arg(short, long, default_value = "8545")]
    pub port: u16,
    /// Host (default 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Do not print the banner
    #[arg(long)]
    pub silent: bool,
}

pub async fn run(args: AnvilArgs) -> eyre::Result<()> {
    let chain_id = args.fork_chain_id.unwrap_or(args.chain_id);
    let dev_accounts = derive_accounts(&args.mnemonic, args.accounts, args.balance);

    if !args.silent {
        print_banner(&dev_accounts, chain_id, args.port, &args.fork_url);
    }

    let anvil_state = AnvilState::new(
        chain_id,
        dev_accounts,
        args.gas_price,
        args.gas_limit,
        args.block_time,
        args.fork_url,
        args.fork_block_number,
    );

    let shared: SharedState = Arc::new(RwLock::new(anvil_state));

    // If block_time is set, spawn a background mining task
    if let Some(interval) = args.block_time {
        let shared_clone = shared.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(
                std::time::Duration::from_secs(interval)
            );
            loop {
                ticker.tick().await;
                shared_clone.write().mine_block(vec![]);
            }
        });
    }

    rpc::start(shared, &args.host, args.port).await
}

fn print_banner(accounts: &[accounts::DevAccount], chain_id: u64, port: u16, fork_url: &Option<String>) {
    println!("\n                             _   _");
    println!("  __ _ _ ____   ___ | |  ");
    println!(" / _` | '_ \\ \\ / / | |  ");
    println!("| (_| | | | \\ V /| | |  ");
    println!(" \\__,_|_| |_|\\_/ |_|_|  lancer-foundry");
    println!();
    if let Some(url) = fork_url {
        println!("Forking from: {url}");
        println!();
    }
    println!("Available Accounts");
    println!("==================");
    for (i, acc) in accounts.iter().enumerate() {
        println!("({i}) {:#x} ({} ETH)", acc.address, acc.balance / revm::primitives::U256::from(10u128.pow(18)));
    }
    println!();
    println!("Private Keys");
    println!("============");
    for (i, acc) in accounts.iter().enumerate() {
        println!("({i}) {}", private_key_hex(acc));
    }
    println!();
    println!("Chain ID: {chain_id}");
    println!("Listening on 127.0.0.1:{port}");
    println!();
}
