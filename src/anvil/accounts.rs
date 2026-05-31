use revm::primitives::{Address, U256};
use sha3::{Digest, Keccak256};
use k256::ecdsa::SigningKey;

const DEFAULT_MNEMONIC: &str =
    "test test test test test test test test test test test junk";

#[derive(Debug, Clone)]
pub struct DevAccount {
    pub address: Address,
    pub private_key: [u8; 32],
    pub balance: U256,
}

/// Derive `count` dev accounts from a BIP-39 mnemonic using simple path derivation.
/// For test purposes we derive from keccak256(mnemonic || index).
pub fn derive_accounts(mnemonic: &str, count: usize, balance_eth: u128) -> Vec<DevAccount> {
    let balance = U256::from(balance_eth) * U256::from(10u128.pow(18));
    (0..count).map(|i| {
        let seed = format!("{mnemonic}:{i}");
        let hash = Keccak256::digest(seed.as_bytes());
        let mut key_bytes: [u8; 32] = hash.into();
        // Clamp to valid secp256k1 range (just mask the highest bit for simplicity)
        key_bytes[0] &= 0x7f;
        if key_bytes == [0u8; 32] { key_bytes[31] = 1; }

        let address = derive_address(&key_bytes);
        DevAccount { address, private_key: key_bytes, balance }
    }).collect()
}

pub fn derive_address(private_key: &[u8; 32]) -> Address {
    let key = SigningKey::from_bytes(private_key.into()).expect("valid key");
    let vk = key.verifying_key();
    let pub_bytes = vk.to_encoded_point(false);
    let hash = Keccak256::digest(&pub_bytes.as_bytes()[1..]);
    Address::from_slice(&hash[12..])
}

pub fn private_key_hex(acc: &DevAccount) -> String {
    format!("0x{}", hex::encode(acc.private_key))
}
