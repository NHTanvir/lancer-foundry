mod anvil;
mod cast;
mod cheatcodes;
mod chisel;
mod config;
mod evm;
mod forge;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "lancer-foundry",
    version,
    about = "Ethereum smart contract development toolkit (Lancer clone of Foundry)"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Build, test, deploy, and lint Solidity contracts
    Forge(forge::ForgeArgs),
    /// EVM RPC, ABI codec, wallet, and transaction utilities
    Cast(cast::CastArgs),
    /// Local EVM devnet with fork support
    Anvil(anvil::AnvilArgs),
    /// Interactive Solidity REPL
    Chisel(chisel::ChiselArgs),
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();
    match cli.command {
        Commands::Forge(a) => forge::run(a).await,
        Commands::Cast(a) => cast::run(a).await,
        Commands::Anvil(a) => anvil::run(a).await,
        Commands::Chisel(a) => chisel::run(a).await,
    }
}
