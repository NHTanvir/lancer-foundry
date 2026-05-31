use eyre::eyre;
use k256::ecdsa::{RecoveryId, Signature, SigningKey};
use sha3::{Digest, Keccak256};

use crate::cast::rpc::JsonRpc;

// ── Signing ──────────────────────────────────────────────────────────────────

/// EIP-191 personal_sign: prefix the message, keccak256, then sign.
/// If `no_hash` is true, sign raw bytes without prefixing.
/// Returns `0x{r}{s}{v}` where v is 27 or 28.
pub fn wallet_sign(message: &str, private_key: &str, no_hash: bool) -> eyre::Result<String> {
    let key_bytes = decode_private_key(private_key)?;
    let signing_key =
        SigningKey::from_slice(&key_bytes).map_err(|e| eyre!("invalid private key: {}", e))?;

    let digest: [u8; 32] = if no_hash {
        // Sign the raw keccak of the message bytes
        Keccak256::digest(message.as_bytes()).into()
    } else {
        // EIP-191: "\x19Ethereum Signed Message:\n" + len + message
        let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
        let mut hasher = Keccak256::new();
        hasher.update(prefix.as_bytes());
        hasher.update(message.as_bytes());
        hasher.finalize().into()
    };

    let (sig, recid) = sign_prehash(&signing_key, &digest)?;
    let v = 27u8 + recid.to_byte();
    let (r_bytes, s_bytes) = sig.split_bytes();
    Ok(format!(
        "0x{}{}{}",
        hex::encode(r_bytes),
        hex::encode(s_bytes),
        hex::encode([v])
    ))
}

/// Derive the Ethereum address (EIP-55 checksummed) from a private key.
pub fn wallet_address(private_key: &str) -> eyre::Result<String> {
    let key_bytes = decode_private_key(private_key)?;
    let signing_key =
        SigningKey::from_slice(&key_bytes).map_err(|e| eyre!("invalid private key: {}", e))?;
    let verifying_key = signing_key.verifying_key();
    let pubkey = verifying_key.to_encoded_point(false);
    let pubkey_bytes = &pubkey.as_bytes()[1..]; // drop 0x04 prefix
    let hash = Keccak256::digest(pubkey_bytes);
    let address_bytes: [u8; 20] = hash[12..].try_into().expect("20 bytes");
    Ok(eip55_checksum(&address_bytes))
}

// ── RPC helpers ───────────────────────────────────────────────────────────────

pub async fn estimate_gas(to: &str, data: &str, from: &str, rpc: &str) -> eyre::Result<String> {
    let client = JsonRpc::new(rpc);
    let mut tx = serde_json::json!({
        "to": to,
        "data": data,
    });
    if !from.is_empty() {
        tx["from"] = serde_json::json!(from);
    }
    let result = client
        .call("eth_estimateGas", serde_json::json!([tx]))
        .await?;
    result
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| eyre!("unexpected estimate response"))
}

pub async fn send_raw_tx(raw_tx: &str, rpc: &str) -> eyre::Result<String> {
    let client = JsonRpc::new(rpc);
    let result = client
        .call("eth_sendRawTransaction", serde_json::json!([raw_tx]))
        .await?;
    result
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| eyre!("unexpected sendRawTransaction response"))
}

// ── Transaction building ──────────────────────────────────────────────────────

/// RLP-encode a legacy (type-0) unsigned transaction for EIP-155 signing.
/// Returns the bytes that should be keccak256-hashed before signing.
pub fn encode_legacy_tx(
    nonce: u64,
    gas_price: u128,
    gas_limit: u64,
    to: Option<&str>,
    value: u128,
    data: &[u8],
    chain_id: u64,
) -> Vec<u8> {
    // EIP-155 signing: [nonce, gasPrice, gasLimit, to, value, data, chainId, 0, 0]
    let mut stream = RlpList::new();
    stream.append_uint(nonce as u128);
    stream.append_uint(gas_price);
    stream.append_uint(gas_limit as u128);
    if let Some(addr) = to {
        let addr_bytes = decode_address_bytes(addr).unwrap_or_default();
        stream.append_bytes(&addr_bytes);
    } else {
        stream.append_bytes(&[]); // contract creation
    }
    stream.append_uint(value);
    stream.append_bytes(data);
    stream.append_uint(chain_id as u128);
    stream.append_uint(0u128); // r = 0
    stream.append_uint(0u128); // s = 0
    stream.encode()
}

/// keccak256 the RLP unsigned tx, sign with k256, return final signed RLP.
pub fn sign_tx(rlp_unsigned: &[u8], private_key: &str, chain_id: u64) -> eyre::Result<Vec<u8>> {
    let key_bytes = decode_private_key(private_key)?;
    let signing_key =
        SigningKey::from_slice(&key_bytes).map_err(|e| eyre!("invalid private key: {}", e))?;

    let digest: [u8; 32] = Keccak256::digest(rlp_unsigned).into();
    let (sig, recid) = sign_prehash(&signing_key, &digest)?;

    // EIP-155: v = chain_id * 2 + 35 + recid
    let v = chain_id * 2 + 35 + recid.to_byte() as u64;

    let (r_bytes, s_bytes) = sig.split_bytes();

    // Re-parse the unsigned list fields (nonce through data) then append v,r,s
    let items = rlp_decode_list(rlp_unsigned)?;
    if items.len() < 9 {
        return Err(eyre!("unexpected RLP item count: {}", items.len()));
    }

    let mut stream = RlpList::new();
    // First 6 items: nonce, gasPrice, gasLimit, to, value, data
    for item in &items[..6] {
        stream.append_raw_bytes(item);
    }
    // v, r, s
    stream.append_uint(v as u128);
    stream.append_bytes_trimmed(r_bytes.as_ref());
    stream.append_bytes_trimmed(s_bytes.as_ref());
    Ok(stream.encode())
}

pub async fn build_and_send(
    to: Option<&str>,
    value: &str,
    data: &str,
    private_key: &str,
    rpc: &str,
    send: bool,
) -> eyre::Result<String> {
    let client = JsonRpc::new(rpc);
    let sender = wallet_address(private_key)?;

    // Fetch nonce
    let nonce_hex = client
        .call(
            "eth_getTransactionCount",
            serde_json::json!([sender, "latest"]),
        )
        .await?;
    let nonce_str = nonce_hex.as_str().unwrap_or("0x0");
    let nonce = u64_from_hex(nonce_str)?;

    // Fetch gas price
    let gp_hex = client
        .call("eth_gasPrice", serde_json::json!([]))
        .await?;
    let gp_str = gp_hex.as_str().unwrap_or("0x1");
    let gas_price = u128_from_hex(gp_str)?;

    // Fetch chain_id
    let cid_hex = client
        .call("eth_chainId", serde_json::json!([]))
        .await?;
    let cid_str = cid_hex.as_str().unwrap_or("0x1");
    let chain_id = u64_from_hex(cid_str)?;

    let value_wei = parse_value_u128(value)?;
    let data_bytes = decode_hex_data(data)?;

    // Estimate gas
    let mut tx_obj = serde_json::json!({
        "from": sender,
        "data": data,
        "value": format!("0x{:x}", value_wei),
    });
    if let Some(t) = to {
        tx_obj["to"] = serde_json::json!(t);
    }
    let gas_est = client
        .call("eth_estimateGas", serde_json::json!([tx_obj]))
        .await
        .unwrap_or(serde_json::json!("0x5208"));
    let gas_limit = u64_from_hex(gas_est.as_str().unwrap_or("0x5208"))?;
    // Add 20% buffer
    let gas_limit = gas_limit * 12 / 10;

    let rlp_unsigned = encode_legacy_tx(
        nonce,
        gas_price,
        gas_limit,
        to,
        value_wei,
        &data_bytes,
        chain_id,
    );
    let signed = sign_tx(&rlp_unsigned, private_key, chain_id)?;
    let raw_tx = format!("0x{}", hex::encode(&signed));

    if send {
        send_raw_tx(&raw_tx, rpc).await
    } else {
        Ok(raw_tx)
    }
}

// ── CREATE2 ───────────────────────────────────────────────────────────────────

/// Compute the CREATE2 deployment address.
/// `keccak256(0xff ++ deployer ++ salt ++ keccak256(init_code))[12..]`
pub fn create2_address(deployer: &str, salt: &str, init_code: &str) -> eyre::Result<String> {
    let deployer_bytes = decode_address_bytes(deployer)
        .map_err(|e| eyre!("invalid deployer address: {}", e))?;
    let salt_bytes = decode_32_bytes(salt).map_err(|e| eyre!("invalid salt: {}", e))?;
    let init_code_bytes = {
        let s = init_code.trim();
        let hex = s
            .strip_prefix("0x")
            .or_else(|| s.strip_prefix("0X"))
            .unwrap_or(s);
        hex::decode(hex).map_err(|e| eyre!("invalid init-code hex: {}", e))?
    };

    let code_hash: [u8; 32] = Keccak256::digest(&init_code_bytes).into();

    let mut preimage = Vec::with_capacity(1 + 20 + 32 + 32);
    preimage.push(0xff);
    preimage.extend_from_slice(&deployer_bytes);
    preimage.extend_from_slice(&salt_bytes);
    preimage.extend_from_slice(&code_hash);

    let hash: [u8; 32] = Keccak256::digest(&preimage).into();
    let address_bytes: [u8; 20] = hash[12..].try_into().expect("20 bytes");
    Ok(eip55_checksum(&address_bytes))
}

// ── Minimal RLP encoder ───────────────────────────────────────────────────────

struct RlpList {
    items: Vec<Vec<u8>>,
}

impl RlpList {
    fn new() -> Self {
        Self { items: Vec::new() }
    }

    /// Append an unsigned integer (canonical: no leading zeros; 0 → empty).
    fn append_uint(&mut self, v: u128) {
        self.items.push(rlp_encode_uint(v));
    }

    /// Append raw bytes as an RLP string item.
    fn append_bytes(&mut self, b: &[u8]) {
        self.items.push(rlp_encode_bytes(b));
    }

    /// Append bytes as big-integer (strip leading zeros).
    fn append_bytes_trimmed(&mut self, b: &[u8]) {
        let start = b.iter().position(|&x| x != 0).unwrap_or(b.len());
        let trimmed = &b[start..];
        self.items.push(rlp_encode_bytes(trimmed));
    }

    /// Append a pre-encoded RLP item verbatim.
    fn append_raw_bytes(&mut self, b: &[u8]) {
        self.items.push(b.to_vec());
    }

    fn encode(self) -> Vec<u8> {
        let payload: Vec<u8> = self.items.into_iter().flatten().collect();
        let mut out = Vec::new();
        rlp_write_length(&mut out, payload.len(), 0xc0);
        out.extend_from_slice(&payload);
        out
    }
}

fn rlp_encode_uint(v: u128) -> Vec<u8> {
    if v == 0 {
        return vec![0x80]; // RLP empty string
    }
    let bytes = v.to_be_bytes(); // 16 bytes
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(15);
    let trimmed = &bytes[start..];
    rlp_encode_bytes(trimmed)
}

fn rlp_encode_bytes(b: &[u8]) -> Vec<u8> {
    if b.len() == 1 && b[0] < 0x80 {
        return vec![b[0]];
    }
    let mut out = Vec::new();
    rlp_write_length(&mut out, b.len(), 0x80);
    out.extend_from_slice(b);
    out
}

fn rlp_write_length(out: &mut Vec<u8>, len: usize, offset: u8) {
    if len < 56 {
        out.push(offset + len as u8);
    } else {
        let len_bytes = minimal_be_bytes(len as u64);
        out.push(offset + 55 + len_bytes.len() as u8);
        out.extend_from_slice(&len_bytes);
    }
}

fn minimal_be_bytes(v: u64) -> Vec<u8> {
    let bytes = v.to_be_bytes();
    let start = bytes.iter().position(|&b| b != 0).unwrap_or(7);
    bytes[start..].to_vec()
}

// ── RLP list decoder (for re-signing) ────────────────────────────────────────

fn rlp_decode_list(data: &[u8]) -> eyre::Result<Vec<Vec<u8>>> {
    if data.is_empty() {
        return Err(eyre!("empty RLP input"));
    }
    let first = data[0];
    let payload_start = if first < 0xc0 {
        return Err(eyre!("expected RLP list"));
    } else if first <= 0xf7 {
        1usize
    } else {
        let len_of_len = (first - 0xf7) as usize;
        if data.len() < 1 + len_of_len {
            return Err(eyre!("RLP too short"));
        }
        1 + len_of_len
    };

    let mut items = Vec::new();
    let mut pos = payload_start;
    while pos < data.len() {
        let (item, new_pos) = rlp_next_item(data, pos)?;
        items.push(item.to_vec());
        pos = new_pos;
    }
    Ok(items)
}

fn rlp_next_item(data: &[u8], pos: usize) -> eyre::Result<(&[u8], usize)> {
    if pos >= data.len() {
        return Err(eyre!("RLP decode: out of bounds"));
    }
    let b = data[pos];
    if b < 0x80 {
        Ok((&data[pos..pos + 1], pos + 1))
    } else if b < 0xb8 {
        let len = (b - 0x80) as usize;
        let end = pos + 1 + len;
        Ok((&data[pos..end], end))
    } else if b < 0xc0 {
        let len_of_len = (b - 0xb7) as usize;
        let mut len = 0usize;
        for &x in &data[pos + 1..pos + 1 + len_of_len] {
            len = (len << 8) | x as usize;
        }
        let start = pos + 1 + len_of_len;
        let end = start + len;
        Ok((&data[pos..end], end))
    } else if b < 0xf8 {
        let len = (b - 0xc0) as usize;
        let end = pos + 1 + len;
        Ok((&data[pos..end], end))
    } else {
        let len_of_len = (b - 0xf7) as usize;
        let mut len = 0usize;
        for &x in &data[pos + 1..pos + 1 + len_of_len] {
            len = (len << 8) | x as usize;
        }
        let start = pos + 1 + len_of_len;
        let end = start + len;
        Ok((&data[pos..end], end))
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

pub fn decode_private_key(s: &str) -> eyre::Result<[u8; 32]> {
    let s = s.trim();
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let bytes = hex::decode(hex).map_err(|e| eyre!("invalid private key hex: {}", e))?;
    if bytes.len() != 32 {
        return Err(eyre!(
            "private key must be 32 bytes, got {}",
            bytes.len()
        ));
    }
    Ok(bytes.try_into().unwrap())
}

fn decode_address_bytes(s: &str) -> eyre::Result<[u8; 20]> {
    let s = s.trim();
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let bytes = hex::decode(hex).map_err(|e| eyre!("invalid address hex: {}", e))?;
    if bytes.len() != 20 {
        return Err(eyre!("address must be 20 bytes, got {}", bytes.len()));
    }
    Ok(bytes.try_into().unwrap())
}

fn decode_32_bytes(s: &str) -> eyre::Result<[u8; 32]> {
    let s = s.trim();
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    // Pad to 64 hex chars (32 bytes) if shorter
    let padded = if hex.len() < 64 {
        format!("{:0>64}", hex)
    } else {
        hex.to_string()
    };
    let bytes = hex::decode(&padded).map_err(|e| eyre!("invalid 32-byte hex: {}", e))?;
    if bytes.len() != 32 {
        return Err(eyre!("salt must be 32 bytes"));
    }
    Ok(bytes.try_into().unwrap())
}

fn decode_hex_data(s: &str) -> eyre::Result<Vec<u8>> {
    let s = s.trim();
    if s.is_empty() || s == "0x" || s == "0X" {
        return Ok(vec![]);
    }
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    hex::decode(hex).map_err(|e| eyre!("invalid hex data: {}", e))
}

fn parse_value_u128(s: &str) -> eyre::Result<u128> {
    let s = s.trim();
    if s.is_empty() || s == "0" {
        return Ok(0);
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u128::from_str_radix(hex, 16).map_err(|e| eyre!("invalid hex value: {}", e))
    } else {
        s.parse::<u128>().map_err(|e| eyre!("invalid value: {}", e))
    }
}

fn u64_from_hex(s: &str) -> eyre::Result<u64> {
    let s = s.trim();
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u64::from_str_radix(hex, 16).map_err(|e| eyre!("invalid hex u64 '{}': {}", s, e))
}

fn u128_from_hex(s: &str) -> eyre::Result<u128> {
    let s = s.trim();
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    u128::from_str_radix(hex, 16).map_err(|e| eyre!("invalid hex u128 '{}': {}", s, e))
}

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

/// Sign a 32-byte pre-hashed digest with a recoverable signature.
fn sign_prehash(
    key: &SigningKey,
    digest: &[u8; 32],
) -> eyre::Result<(Signature, RecoveryId)> {
    key.sign_prehash_recoverable(digest)
        .map_err(|e| eyre!("signing failed: {}", e))
}
