use alloy_primitives::U256;
use eyre::eyre;
use sha3::{Digest, Keccak256};

/// Convert a value (decimal or 0x-hex) and unit to wei (as decimal string).
/// unit: "ether" | "gwei" | "wei"
pub fn to_wei(value: &str, unit: &str) -> eyre::Result<String> {
    let multiplier: U256 = match unit.to_lowercase().as_str() {
        "ether" | "eth" => U256::from(1_000_000_000_000_000_000u128),
        "gwei" => U256::from(1_000_000_000u64),
        "wei" => U256::from(1u64),
        other => return Err(eyre!("unknown unit: {}", other)),
    };

    let v = parse_u256(value)?;
    let wei = v
        .checked_mul(multiplier)
        .ok_or_else(|| eyre!("overflow in to_wei"))?;
    Ok(wei.to_string())
}

/// Convert wei (decimal or 0x-hex) to a given unit as decimal string.
/// unit: "ether" | "gwei" | "wei"
pub fn from_wei(value: &str, unit: &str) -> eyre::Result<String> {
    let divisor: U256 = match unit.to_lowercase().as_str() {
        "ether" | "eth" => U256::from(1_000_000_000_000_000_000u128),
        "gwei" => U256::from(1_000_000_000u64),
        "wei" => U256::from(1u64),
        other => return Err(eyre!("unknown unit: {}", other)),
    };

    let v = parse_u256(value)?;
    let result = v / divisor;
    Ok(result.to_string())
}

/// Convert a decimal or 0x-hex value to 0x-prefixed hex.
pub fn to_hex(value: &str) -> eyre::Result<String> {
    let v = parse_u256(value)?;
    Ok(format!("0x{:x}", v))
}

/// Return keccak256 of the UTF-8 bytes of input as 0x-prefixed hex.
pub fn keccak256(input: &str) -> String {
    let hash = Keccak256::digest(input.as_bytes());
    format!("0x{}", hex::encode(hash))
}

/// Maximum value for an unsigned integer of `bits` width.
pub fn max_uint(bits: u32) -> String {
    if bits == 0 || bits > 256 {
        return "0".to_string();
    }
    if bits == 256 {
        return U256::MAX.to_string();
    }
    let v = (U256::from(1u64) << bits) - U256::from(1u64);
    v.to_string()
}

/// Maximum value for a signed integer of `bits` width (2^(bits-1) - 1).
pub fn max_int(bits: u32) -> String {
    if bits == 0 || bits > 256 {
        return "0".to_string();
    }
    let v = (U256::from(1u64) << (bits - 1)) - U256::from(1u64);
    v.to_string()
}

/// Minimum value for a signed integer of `bits` width (-(2^(bits-1))).
/// Returned as a negative decimal string.
pub fn min_int(bits: u32) -> String {
    if bits == 0 || bits > 256 {
        return "0".to_string();
    }
    // 2^(bits-1) as U256 then negate conceptually — just return the string form
    let magnitude = U256::from(1u64) << (bits - 1);
    format!("-{}", magnitude)
}

// ── helpers ─────────────────────────────────────────────────────────────────

pub fn parse_u256(s: &str) -> eyre::Result<U256> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        U256::from_str_radix(hex, 16).map_err(|e| eyre!("invalid hex number '{}': {}", s, e))
    } else {
        s.parse::<U256>()
            .map_err(|e| eyre!("invalid decimal number '{}': {}", s, e))
    }
}
