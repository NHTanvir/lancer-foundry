use clap::Parser;
use k256::ecdsa::SigningKey;
use rand_core::OsRng;
use sha3::{Digest, Keccak256};

use crate::cast::sign_ops;

#[derive(Parser, Debug)]
pub struct WalletArgs {
    #[command(subcommand)]
    pub command: WalletCommand,
}

#[derive(clap::Subcommand, Debug)]
pub enum WalletCommand {
    /// Generate a new random keypair
    New(NewArgs),
    /// Sign a message with a local private key
    Sign(SignArgs),
    /// Display the address for a private key
    Address(AddressArgs),
}

#[derive(Parser, Debug)]
pub struct NewArgs {
    /// Number of wallets to generate
    #[arg(short = 'n', long, default_value = "1")]
    pub number: usize,
}

#[derive(Parser, Debug)]
pub struct SignArgs {
    /// Message to sign (hex or plain string)
    pub message: String,
    /// Private key (0x-prefixed hex)
    #[arg(long)]
    pub private_key: String,
    /// Sign raw bytes without EIP-191 prefix
    #[arg(long, default_value = "false")]
    pub no_hash: bool,
}

#[derive(Parser, Debug)]
pub struct AddressArgs {
    /// Private key (0x-prefixed hex)
    pub private_key: String,
}

pub fn run(args: WalletArgs) -> eyre::Result<()> {
    match args.command {
        WalletCommand::New(a) => cmd_new(a),
        WalletCommand::Sign(a) => {
            let sig = sign_ops::wallet_sign(&a.message, &a.private_key, a.no_hash)?;
            println!("{}", sig);
            Ok(())
        }
        WalletCommand::Address(a) => {
            let addr = sign_ops::wallet_address(&a.private_key)?;
            println!("{}", addr);
            Ok(())
        }
    }
}

fn cmd_new(args: NewArgs) -> eyre::Result<()> {
    for _ in 0..args.number {
        let (address, privkey) = generate_keypair();
        println!("Successfully created new keypair.");
        println!("Address:     {address}");
        println!("Private key: 0x{privkey}");
        println!();
    }
    Ok(())
}

fn generate_keypair() -> (String, String) {
    let signing_key = SigningKey::random(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    // Uncompressed public key: 0x04 || x || y  (65 bytes total)
    let pubkey_bytes = verifying_key.to_encoded_point(false);
    // Drop the 0x04 prefix; hash the remaining 64 bytes
    let pubkey_uncompressed = &pubkey_bytes.as_bytes()[1..];
    let hash = Keccak256::digest(pubkey_uncompressed);
    // Ethereum address = last 20 bytes of keccak256(pubkey)
    let address_bytes: [u8; 20] = hash[12..].try_into().expect("slice is 20 bytes");

    let privkey_hex = hex::encode(signing_key.to_bytes());
    let address = eip55_checksum(&address_bytes);
    (address, privkey_hex)
}

/// EIP-55 mixed-case checksum encoding of an Ethereum address.
fn eip55_checksum(address: &[u8; 20]) -> String {
    let hex_lower = hex::encode(address);
    let hash = Keccak256::digest(hex_lower.as_bytes());
    let hash_hex = hex::encode(hash);

    let checksummed: String = hex_lower
        .chars()
        .zip(hash_hex.chars())
        .map(|(c, h)| if h >= '8' { c.to_ascii_uppercase() } else { c })
        .collect();

    format!("0x{checksummed}")
}
