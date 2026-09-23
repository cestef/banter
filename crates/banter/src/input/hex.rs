//! Hex encoding without a dependency.

const DIGITS: &[u8; 16] = b"0123456789abcdef";

pub fn encode(bytes: impl AsRef<[u8]>) -> String {
    let bytes = bytes.as_ref();
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(DIGITS[usize::from(b >> 4)] as char);
        out.push(DIGITS[usize::from(b & 15)] as char);
    }
    out
}

/// Decode, accepting an optional `0x` prefix and either case.
pub fn decode(raw: &str) -> Result<Vec<u8>, String> {
    let body = raw.strip_prefix("0x").unwrap_or(raw).as_bytes();
    if body.len() % 2 != 0 {
        return Err("odd number of hex digits".into());
    }
    body.chunks_exact(2)
        .map(|pair| Ok(nibble(pair[0])? << 4 | nibble(pair[1])?))
        .collect()
}

fn nibble(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit '{}'", c as char)),
    }
}
