pub mod repl;
pub mod session;

use clap::Parser;

#[derive(Parser, Debug)]
pub struct ChiselArgs {
    /// Fork URL for the REPL session
    #[arg(long)]
    pub fork_url: Option<String>,
    /// Fork block number
    #[arg(long)]
    pub fork_block_number: Option<u64>,
}

pub async fn run(args: ChiselArgs) -> eyre::Result<()> {
    tokio::task::spawn_blocking(move || {
        repl::run_repl(args.fork_url)
    }).await??;
    Ok(())
}
