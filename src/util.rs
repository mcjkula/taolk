use blake2::Digest;

use crate::types::Pubkey;

const SS58_PREFIX: u8 = 42;

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

/// "5FHneW...94ty" — first 6 chars are the conventional human-memorable handle.
pub fn ss58_short(pubkey: &Pubkey) -> String {
    let full = ss58_from_pubkey(pubkey);
    if full.len() > 12 {
        format!("{}...{}", &full[..6], &full[full.len() - 4..])
    } else {
        full
    }
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    if max < 10 {
        return s[..max].to_string();
    }
    format!("{}...{}", &s[..max - 7], &s[s.len() - 4..])
}

pub fn format_balance(plancks: u128, decimals: u32, symbol: &str) -> String {
    format_balance_inner(plancks, decimals, symbol, None)
}

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
        return format!("{} {display_symbol}", format_number(plancks));
    }
    let divisor = 10u128.pow(decimals);
    let whole = plancks / divisor;
    let frac = plancks % divisor;
    let width = usize::try_from(decimals).unwrap_or(0);
    let frac_str = format!("{:0>width$}", frac, width = width);
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
    let whole_fmt = format_number(whole);
    if trimmed.is_empty() {
        format!("{whole_fmt}.0 {display_symbol}")
    } else {
        format!("{whole_fmt}.{trimmed} {display_symbol}")
    }
}

/// Below 0.001 TAO, display as integer RAO; above, as TAO.
pub fn format_fee(plancks: u128, decimals: u32, symbol: &str) -> String {
    let divisor = 10u128.pow(decimals);
    if plancks < divisor / 1000 {
        let rao_symbol = if symbol == "TAO" { "RAO" } else { symbol };
        format!("{} {rao_symbol}", format_number(plancks))
    } else {
        format_balance(plancks, decimals, symbol)
    }
}

pub fn format_number(n: u128) -> String {
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

pub fn pubkey_from_ss58(address: &str) -> Option<Pubkey> {
    ss58_decode(address).ok()
}

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

pub fn ss58_decode(address: &str) -> Result<Pubkey, crate::error::AddressError> {
    use crate::error::AddressError;
    let decoded = bs58_decode(address).map_err(|_| AddressError::InvalidBase58)?;
    if decoded.len() < 35 {
        return Err(AddressError::TooShort);
    }
    let prefix_len = if decoded[0] < 64 { 1 } else { 2 };
    let pubkey_end = prefix_len + 32;
    if decoded.len() < pubkey_end + 2 {
        return Err(AddressError::TooShort);
    }
    let payload = &decoded[..pubkey_end];
    let expected_checksum = &decoded[pubkey_end..pubkey_end + 2];
    let hash = {
        let mut hasher = blake2::Blake2b512::new();
        hasher.update(b"SS58PRE");
        hasher.update(payload);
        hasher.finalize()
    };
    if &hash[..2] != expected_checksum {
        return Err(AddressError::BadChecksum);
    }
    let mut pubkey = [0u8; 32];
    pubkey.copy_from_slice(&decoded[prefix_len..pubkey_end]);
    Ok(Pubkey(pubkey))
}

pub fn copy_to_clipboard(text: &str) -> bool {
    if write_osc52(text) && term_supports_osc52() {
        return true;
    }
    #[cfg(target_os = "macos")]
    if try_pipe("pbcopy", &[], text) {
        return true;
    }
    if std::env::var("WAYLAND_DISPLAY").is_ok() && try_pipe("wl-copy", &[], text) {
        return true;
    }
    if std::env::var("DISPLAY").is_ok() && try_pipe("xclip", &["-selection", "clipboard"], text) {
        return true;
    }
    if try_pipe("xsel", &["-b", "-i"], text) {
        return true;
    }
    false
}

fn write_osc52(text: &str) -> bool {
    use std::io::{Write, stdout};
    let encoded = b64_encode(text.as_bytes());
    let mut out = stdout().lock();
    out.write_all(b"\x1b]52;c;").is_ok()
        && out.write_all(encoded.as_bytes()).is_ok()
        && out.write_all(b"\x07").is_ok()
        && out.flush().is_ok()
}

fn term_supports_osc52() -> bool {
    if std::env::var("TMUX").is_ok() {
        return true;
    }
    matches!(
        std::env::var("TERM_PROGRAM").as_deref(),
        Ok("iTerm.app" | "WezTerm" | "Alacritty" | "kitty" | "ghostty")
    )
}

fn try_pipe(cmd: &str, args: &[&str], text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new(cmd)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    if let Some(mut stdin) = child.stdin.take()
        && stdin.write_all(text.as_bytes()).is_err()
    {
        return false;
    }
    matches!(child.wait(), Ok(s) if s.success())
}

fn b64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = u32::from(chunk.get(1).copied().unwrap_or(0));
        let b2 = u32::from(chunk.get(2).copied().unwrap_or(0));
        let n = (b0 << 16) | (b1 << 8) | b2;
        // SECURITY: `& 0x3f` masks each index to 0..=63, always in bounds for the 64-byte ALPHABET.
        let i0 = ((n >> 18) & 0x3f) as usize; // SECURITY: bounded 0..=63
        let i1 = ((n >> 12) & 0x3f) as usize; // SECURITY: bounded 0..=63
        let i2 = ((n >> 6) & 0x3f) as usize; // SECURITY: bounded 0..=63
        let i3 = (n & 0x3f) as usize; // SECURITY: bounded 0..=63
        out.push(char::from(ALPHABET[i0]));
        out.push(char::from(ALPHABET[i1]));
        out.push(if chunk.len() > 1 {
            char::from(ALPHABET[i2])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(ALPHABET[i3])
        } else {
            '='
        });
    }
    out
}

fn bs58_decode(input: &str) -> Result<Vec<u8>, ()> {
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let mut bytes = vec![0u8];
    for c in input.chars() {
        // SECURITY: only ASCII chars in the ALPHABET; non-ASCII fails the position check.
        let byte = u8::try_from(u32::from(c)).map_err(|_| ())?;
        let idx = ALPHABET.iter().position(|&a| a == byte).ok_or(())?;
        let mut carry = idx;
        for b in bytes.iter_mut() {
            carry += usize::from(*b) * 58;
            // SECURITY: `carry % 256` ⊆ 0..=255, always fits in u8.
            *b = u8::try_from(carry % 256).unwrap_or(0);
            carry /= 256;
        }
        while carry > 0 {
            bytes.push(u8::try_from(carry % 256).unwrap_or(0));
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
        let mut carry = u32::from(byte);
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
            result.push(char::from(ALPHABET[0]));
        } else {
            break;
        }
    }
    for &d in digits.iter().rev() {
        let idx = d as usize; // SECURITY: digits are computed `% 58`, always in 0..=57.
        result.push(char::from(ALPHABET[idx]));
    }
    result
}
