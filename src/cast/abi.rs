use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_primitives::{Address, I256, U256};
use eyre::eyre;
use sha3::{Digest, Keccak256};

// ── Public API ────────────────────────────────────────────────────────────────

/// ABI-encode `args` according to `sig`.
///
/// `sig` can be:
///   - `"foo(uint256,address)"` — function call: prepends 4-byte selector
///   - `"(uint256,address)"`    — raw tuple: no selector
pub fn abi_encode(sig: &str, args: &[String]) -> eyre::Result<String> {
    let sig = sig.trim();

    // Split into optional function name and parameter list
    let (func_name, params_str) = split_sig(sig)?;
    let types = parse_param_types(params_str)?;

    if types.len() != args.len() {
        return Err(eyre!(
            "signature has {} parameter(s) but {} argument(s) were provided",
            types.len(),
            args.len()
        ));
    }

    // Encode each arg
    let values: Vec<DynSolValue> = types
        .iter()
        .zip(args.iter())
        .map(|(t, a)| parse_value(t, a))
        .collect::<eyre::Result<_>>()?;

    // Build tuple value and ABI-encode
    let tuple = DynSolValue::Tuple(values);
    let encoded = tuple.abi_encode_params();

    let hex_data = hex::encode(&encoded);

    if let Some(name) = func_name {
        // Prepend 4-byte selector: keccak256 of "name(type1,type2,...)"
        let selector = compute_selector(name, params_str);
        Ok(format!("0x{}{}", hex::encode(selector), hex_data))
    } else {
        Ok(format!("0x{}", hex_data))
    }
}

/// ABI-decode `data` (0x-prefixed hex) according to `types_str`
/// (comma-separated Solidity types like `"uint256,address"`).
/// Returns human-readable output, one decoded value per line.
pub fn abi_decode(types_str: &str, data: &str) -> eyre::Result<String> {
    let data = data.trim();
    let hex_str = data
        .strip_prefix("0x")
        .or_else(|| data.strip_prefix("0X"))
        .unwrap_or(data);
    let bytes = hex::decode(hex_str).map_err(|e| eyre!("invalid hex data: {}", e))?;

    let types = parse_param_types(types_str)?;

    // Build a tuple type for decoding
    let tuple_type = DynSolType::Tuple(types);
    let decoded = tuple_type
        .abi_decode_params(&bytes)
        .map_err(|e| eyre!("ABI decode error: {}", e))?;

    let DynSolValue::Tuple(values) = decoded else {
        return Err(eyre!("unexpected decoded value type"));
    };

    let lines: Vec<String> = values.iter().map(format_value).collect();
    Ok(lines.join("\n"))
}

/// Compute the 4-byte function selector for `sig` (e.g. `"foo(uint256,address)"`).
/// Returns `"0x"` + 4 hex bytes.
pub fn function_selector(sig: &str) -> String {
    let sig = sig.trim();
    // Normalise: remove spaces around commas and parentheses
    let normalised = normalise_sig(sig);
    let hash = Keccak256::digest(normalised.as_bytes());
    format!("0x{}", hex::encode(&hash[..4]))
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Split `"foo(uint256,address)"` into `(Some("foo"), "uint256,address")`.
/// For `"(uint256,address)"` returns `(None, "uint256,address")`.
fn split_sig(sig: &str) -> eyre::Result<(Option<&str>, &str)> {
    let open = sig
        .find('(')
        .ok_or_else(|| eyre!("invalid signature (no opening parenthesis): {}", sig))?;
    let close = sig
        .rfind(')')
        .ok_or_else(|| eyre!("invalid signature (no closing parenthesis): {}", sig))?;
    if close < open {
        return Err(eyre!("invalid signature: {}", sig));
    }
    let inner = &sig[open + 1..close];
    let name_part = &sig[..open];
    let name = if name_part.is_empty() {
        None
    } else {
        Some(name_part)
    };
    Ok((name, inner))
}

/// Parse a comma-separated list of Solidity types (without outer parentheses).
fn parse_param_types(params: &str) -> eyre::Result<Vec<DynSolType>> {
    if params.trim().is_empty() {
        return Ok(vec![]);
    }
    // Split on top-level commas (respecting nested parens)
    let parts = split_top_level(params);
    parts
        .iter()
        .map(|p| {
            let p = p.trim();
            parse_single_type(p)
        })
        .collect()
}

/// Split a string on commas that are not inside parentheses.
fn split_top_level(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth = depth.saturating_sub(1);
                current.push(ch);
            }
            ',' if depth == 0 => {
                result.push(current.trim().to_string());
                current = String::new();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        result.push(current.trim().to_string());
    }
    result
}

/// Parse a single Solidity type string into a DynSolType.
fn parse_single_type(t: &str) -> eyre::Result<DynSolType> {
    t.parse::<DynSolType>()
        .map_err(|e| eyre!("unknown type '{}': {}", t, e))
}

/// Parse a string value according to its DynSolType.
fn parse_value(ty: &DynSolType, s: &str) -> eyre::Result<DynSolValue> {
    let s = s.trim();
    match ty {
        DynSolType::Uint(bits) => {
            let n = parse_u256_str(s)?;
            Ok(DynSolValue::Uint(n, *bits))
        }
        DynSolType::Int(bits) => {
            let n = parse_i256_str(s)?;
            Ok(DynSolValue::Int(n, *bits))
        }
        DynSolType::Address => {
            let hex = s
                .strip_prefix("0x")
                .or_else(|| s.strip_prefix("0X"))
                .unwrap_or(s);
            let bytes = hex::decode(hex).map_err(|e| eyre!("invalid address hex: {}", e))?;
            if bytes.len() != 20 {
                return Err(eyre!("address must be 20 bytes, got {}", bytes.len()));
            }
            let addr = Address::from_slice(&bytes);
            Ok(DynSolValue::Address(addr))
        }
        DynSolType::Bool => match s {
            "true" | "1" => Ok(DynSolValue::Bool(true)),
            "false" | "0" => Ok(DynSolValue::Bool(false)),
            _ => Err(eyre!("invalid bool value: {}", s)),
        },
        DynSolType::FixedBytes(n) => {
            let hex = s
                .strip_prefix("0x")
                .or_else(|| s.strip_prefix("0X"))
                .unwrap_or(s);
            let bytes = hex::decode(hex).map_err(|e| eyre!("invalid bytes hex: {}", e))?;
            if bytes.len() != *n {
                return Err(eyre!("bytes{} needs {} bytes, got {}", n, n, bytes.len()));
            }
            let mut arr = [0u8; 32];
            arr[..*n].copy_from_slice(&bytes);
            Ok(DynSolValue::FixedBytes(
                alloy_primitives::FixedBytes::from_slice(&arr[..*n]),
                *n,
            ))
        }
        DynSolType::Bytes => {
            let hex = s
                .strip_prefix("0x")
                .or_else(|| s.strip_prefix("0X"))
                .unwrap_or(s);
            let bytes = hex::decode(hex).map_err(|e| eyre!("invalid bytes hex: {}", e))?;
            Ok(DynSolValue::Bytes(bytes))
        }
        DynSolType::String => Ok(DynSolValue::String(s.to_string())),
        DynSolType::Array(inner) => {
            // Expect "[a,b,c]" format
            let s = s.trim_start_matches('[').trim_end_matches(']');
            let parts = split_top_level(s);
            let values: Vec<DynSolValue> = parts
                .iter()
                .map(|p| parse_value(inner, p.trim()))
                .collect::<eyre::Result<_>>()?;
            Ok(DynSolValue::Array(values))
        }
        DynSolType::FixedArray(inner, size) => {
            let s = s.trim_start_matches('[').trim_end_matches(']');
            let parts = split_top_level(s);
            if parts.len() != *size {
                return Err(eyre!("fixed array size {}, got {}", size, parts.len()));
            }
            let values: Vec<DynSolValue> = parts
                .iter()
                .map(|p| parse_value(inner, p.trim()))
                .collect::<eyre::Result<_>>()?;
            Ok(DynSolValue::FixedArray(values))
        }
        DynSolType::Tuple(inner_types) => {
            // Expect "(a,b,c)" format
            let s = s.trim_start_matches('(').trim_end_matches(')');
            let parts = split_top_level(s);
            if parts.len() != inner_types.len() {
                return Err(eyre!(
                    "tuple has {} fields but {} values given",
                    inner_types.len(),
                    parts.len()
                ));
            }
            let values: Vec<DynSolValue> = inner_types
                .iter()
                .zip(parts.iter())
                .map(|(t, p)| parse_value(t, p.trim()))
                .collect::<eyre::Result<_>>()?;
            Ok(DynSolValue::Tuple(values))
        }
        _ => Err(eyre!("unsupported type for encoding: {:?}", ty)),
    }
}

fn format_value(v: &DynSolValue) -> String {
    match v {
        DynSolValue::Uint(n, _) => n.to_string(),
        DynSolValue::Int(n, _) => n.to_string(),
        DynSolValue::Address(a) => format!("{:?}", a),
        DynSolValue::Bool(b) => b.to_string(),
        DynSolValue::Bytes(b) => format!("0x{}", hex::encode(b)),
        DynSolValue::FixedBytes(b, _) => format!("0x{}", hex::encode(b.as_slice())),
        DynSolValue::String(s) => s.clone(),
        DynSolValue::Array(items) | DynSolValue::FixedArray(items) => {
            let inner: Vec<String> = items.iter().map(format_value).collect();
            format!("[{}]", inner.join(", "))
        }
        DynSolValue::Tuple(items) => {
            let inner: Vec<String> = items.iter().map(format_value).collect();
            format!("({})", inner.join(", "))
        }
        _ => format!("{:?}", v),
    }
}

fn parse_u256_str(s: &str) -> eyre::Result<U256> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        U256::from_str_radix(hex, 16).map_err(|e| eyre!("invalid hex u256 '{}': {}", s, e))
    } else {
        s.parse::<U256>()
            .map_err(|e| eyre!("invalid decimal u256 '{}': {}", s, e))
    }
}

fn parse_i256_str(s: &str) -> eyre::Result<I256> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        let u = U256::from_str_radix(hex, 16).map_err(|e| eyre!("invalid hex i256 '{}': {}", s, e))?;
        Ok(I256::from_raw(u))
    } else {
        I256::from_dec_str(s).map_err(|e| eyre!("invalid decimal i256 '{}': {}", s, e))
    }
}

fn compute_selector(name: &str, params: &str) -> [u8; 4] {
    let canonical = format!("{}({})", name, normalise_params(params));
    let hash = Keccak256::digest(canonical.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

fn normalise_sig(sig: &str) -> String {
    // Remove spaces
    sig.chars().filter(|c| !c.is_whitespace()).collect()
}

fn normalise_params(params: &str) -> String {
    params.chars().filter(|c| !c.is_whitespace()).collect()
}
