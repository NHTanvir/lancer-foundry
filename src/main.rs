//! lancer-foundry
//! Rust clone of foundry. See plan.md (local, gitignored) for the spec.

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "lancer-foundry", version, about = "Rust clone of foundry")]
struct Cli {}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let _cli = Cli::parse();
    println!("lancer-foundry - scaffold ready. Implement per plan.md.");
    Ok(())
}
