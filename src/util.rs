use blake2::Digest;

use crate::types::Pubkey;

const SS58_PREFIX: u8 = 42;

/// Full SS58 address from a 32-byte public key.
pub fn ss58_from_pubkey(pubkey: &Pubkey) -> String {
    let mut payload = vec![SS58_PREFIX];
    payload.extend_from_slice(&pubkey.0);
    let hash = {
        let mut hasher = blake2::Blake2b512::new();
        hasher.update(b"SS58PRE");
        hasher.update(&payload);
        hasher.finalize()
    };
    payload.extend_from_slice(&hash[..2]);
    bs58_encode(&payload)
}

/// Shortened SS58 display: "5FHneW...94ty" (first 6 + ... + last 4 = 13 chars).
/// The first 6 chars are the human-memorable identifier (how people refer to wallets).
pub fn ss58_short(pubkey: &Pubkey) -> String {
    let full = ss58_from_pubkey(pubkey);
    if full.len() > 12 {
        format!("{}...{}", &full[..6], &full[full.len() - 4..])
    } else {
        full
    }
}

/// Truncate a string with ellipsis in the middle.
pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    if max < 10 {
        return s[..max].to_string();
    }
    format!("{}...{}", &s[..max - 7], &s[s.len() - 4..])
}

/// Format a balance in plancks to human-readable form. Uses τ symbol for TAO.
/// Full precision -- used for fee display.
pub fn format_balance(plancks: u128, decimals: u32, symbol: &str) -> String {
    format_balance_inner(plancks, decimals, symbol, None)
}

/// Format a balance with limited decimal places -- used for status bar display.
pub fn format_balance_short(plancks: u128, decimals: u32, symbol: &str) -> String {
    format_balance_inner(plancks, decimals, symbol, Some(4))
}

fn format_balance_inner(
    plancks: u128,
    decimals: u32,
    symbol: &str,
    max_frac: Option<usize>,
) -> String {
    let display_symbol = if symbol == "TAO" { "\u{03C4}" } else { symbol };
    if decimals == 0 {
        return format!("{} {display_symbol}", format_number(plancks as u64));
    }
    let divisor = 10u128.pow(decimals);
    let whole = plancks / divisor;
    let frac = plancks % divisor;
    let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
    let trimmed = match max_frac {
        Some(max) => {
            let capped = if frac_str.len() > max {
                &frac_str[..max]
            } else {
                &frac_str
            };
            capped.trim_end_matches('0')
        }
        None => frac_str.trim_end_matches('0'),
    };
    let whole_fmt = format_number(whole as u64);
    if trimmed.is_empty() {
        format!("{whole_fmt}.0 {display_symbol}")
    } else {
        format!("{whole_fmt}.{trimmed} {display_symbol}")
    }
}

/// Format a fee amount -- uses RAO for small amounts, TAO for large.
/// Picks the unit that produces the most readable number.
pub fn format_fee(plancks: u128, decimals: u32, symbol: &str) -> String {
    let divisor = 10u128.pow(decimals);
    if plancks < divisor / 1000 {
        // Less than 0.001 TAO -- display in RAO (whole numbers)
        let rao_symbol = if symbol == "TAO" { "RAO" } else { symbol };
        format!("{} {rao_symbol}", format_number(plancks as u64))
    } else {
        // 0.001 TAO or more -- display in TAO with full precision
        format_balance(plancks, decimals, symbol)
    }
}

pub fn format_number(n: u64) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(c);
    }
    result
}

/// Try to parse an SS58 address to a 32-byte public key. Returns None on failure.
pub fn pubkey_from_ss58(address: &str) -> Option<Pubkey> {
    ss58_decode(address).ok()
}

/// True if `body` contains `@<my_ss58>` at a word boundary.
pub fn body_mentions(body: &str, my_ss58: &str) -> bool {
    let target = my_ss58.as_bytes();
    if target.len() != 48 {
        return false;
    }
    let bytes = body.as_bytes();
    let window = 1 + target.len();
    if bytes.len() < window {
        return false;
    }
    for pos in 0..=(bytes.len() - window) {
        if bytes[pos] != b'@' {
            continue;
        }
        if pos > 0 && !bytes[pos - 1].is_ascii_whitespace() {
            continue;
        }
        if &bytes[pos + 1..pos + window] != target {
            continue;
        }
        let after = pos + window;
        if after == bytes.len() || !is_base58_byte(bytes[after]) {
            return true;
        }
    }
    false
}

fn is_base58_byte(b: u8) -> bool {
    matches!(b,
        b'1'..=b'9'
        | b'A'..=b'H'
        | b'J'..=b'N'
        | b'P'..=b'Z'
        | b'a'..=b'k'
        | b'm'..=b'z'
    )
}

/// Decode an SS58 address to a 32-byte public key.
pub fn ss58_decode(address: &str) -> Result<Pubkey, String> {
    let decoded = bs58_decode(address).map_err(|_| "Invalid base58")?;
    // Minimum: 1 (prefix) + 32 (pubkey) + 2 (checksum) = 35
    if decoded.len() < 35 {
        return Err("Address too short".into());
    }
    let prefix_len = if decoded[0] < 64 { 1 } else { 2 };
    let pubkey_end = prefix_len + 32;
    if decoded.len() < pubkey_end + 2 {
        return Err("Address too short".into());
    }
    // Verify checksum
    let payload = &decoded[..pubkey_end];
    let expected_checksum = &decoded[pubkey_end..pubkey_end + 2];
    let hash = {
        let mut hasher = blake2::Blake2b512::new();
        hasher.update(b"SS58PRE");
        hasher.update(payload);
        hasher.finalize()
    };
    if &hash[..2] != expected_checksum {
        return Err("Invalid checksum".into());
    }
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(&decoded[prefix_len..pubkey_end]);
    Ok(Pubkey(pubkey))
}

fn bs58_decode(input: &str) -> Result<Vec<u8>, ()> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut bytes = vec![0u8];
    for c in input.chars() {
        let idx = ALPHABET.iter().position(|&a| a == c as u8).ok_or(())?;
        let mut carry = idx;
        for b in bytes.iter_mut() {
            carry += *b as usize * 58;
            *b = (carry % 256) as u8;
            carry /= 256;
        }
        while carry > 0 {
            bytes.push((carry % 256) as u8);
            carry /= 256;
        }
    }
    // Leading '1' characters = leading zero bytes
    for c in input.chars() {
        if c == '1' {
            bytes.push(0);
        } else {
            break;
        }
    }
    bytes.reverse();
    Ok(bytes)
}

fn bs58_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    if data.is_empty() {
        return String::new();
    }
    let mut digits = vec![0u32];
    for &byte in data {
        let mut carry = byte as u32;
        for d in digits.iter_mut() {
            carry += *d * 256;
            *d = carry % 58;
            carry /= 58;
        }
        while carry > 0 {
            digits.push(carry % 58);
            carry /= 58;
        }
    }
    let mut result = String::new();
    for &b in data {
        if b == 0 {
            result.push(ALPHABET[0] as char);
        } else {
            break;
        }
    }
    for &d in digits.iter().rev() {
        result.push(ALPHABET[d as usize] as char);
    }
    result
}
