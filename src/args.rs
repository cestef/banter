//! Typed access to a command's arguments.
//!
//! Handlers ask for the type they want and get a positioned [`Error::BadArg`] when
//! the peer sent something else. Implement [`FromArg`] for domain types to keep
//! parsing out of handlers entirely.

use crate::error::{Error, Result};

/// A type a single argument token can be parsed into.
pub trait FromArg: Sized {
    /// Name of the expected shape, shown in errors.
    const KIND: &'static str;

    fn from_arg(raw: &str) -> Result<Self, String>;
}

/// Whitespace-separated arguments following the command name.
#[derive(Clone, Debug, Default)]
pub struct Args<'a> {
    raw: &'a str,
    tokens: Vec<&'a str>,
}

impl<'a> Args<'a> {
    pub fn parse(raw: &'a str) -> Self {
        Self {
            raw: raw.trim(),
            tokens: raw.split_whitespace().collect(),
        }
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    /// Everything after the command name, untouched.
    pub fn raw(&self) -> &'a str {
        self.raw
    }

    pub fn token(&self, index: usize) -> Option<&'a str> {
        self.tokens.get(index).copied()
    }

    pub fn tokens(&self) -> &[&'a str] {
        &self.tokens
    }

    /// Required argument at `index`.
    pub fn get<T: FromArg>(&self, index: usize) -> Result<T> {
        let raw = self.token(index).ok_or(Error::BadArg {
            index,
            kind: T::KIND,
            reason: "missing".into(),
        })?;
        T::from_arg(raw).map_err(|reason| Error::BadArg {
            index,
            kind: T::KIND,
            reason,
        })
    }

    /// Optional argument at `index`: absent is `None`, malformed is an error.
    pub fn opt<T: FromArg>(&self, index: usize) -> Result<Option<T>> {
        match self.token(index) {
            None => Ok(None),
            Some(_) => self.get(index).map(Some),
        }
    }

    /// Optional argument at `index`, or `fallback` when absent.
    pub fn or<T: FromArg>(&self, index: usize, fallback: T) -> Result<T> {
        Ok(self.opt(index)?.unwrap_or(fallback))
    }

    /// Every token from `index` on, rejoined with single spaces.
    pub fn rest(&self, index: usize) -> String {
        self.tokens.get(index..).unwrap_or_default().join(" ")
    }
}

impl FromArg for String {
    const KIND: &'static str = "string";

    fn from_arg(raw: &str) -> Result<Self, String> {
        Ok(raw.to_string())
    }
}

impl FromArg for bool {
    const KIND: &'static str = "bool";

    fn from_arg(raw: &str) -> Result<Self, String> {
        match raw.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" | "on" => Ok(true),
            "0" | "false" | "no" | "n" | "off" => Ok(false),
            other => Err(format!("expected yes or no, got '{other}'")),
        }
    }
}

macro_rules! int_arg {
    ($($t:ty),*) => {$(
        impl FromArg for $t {
            const KIND: &'static str = "integer";

            /// Decimal, or hex and binary with `0x` and `0b` prefixes.
            fn from_arg(raw: &str) -> Result<Self, String> {
                let (digits, radix) = if let Some(rest) = raw.strip_prefix("0x") {
                    (rest, 16)
                } else if let Some(rest) = raw.strip_prefix("0b") {
                    (rest, 2)
                } else {
                    (raw, 10)
                };
                <$t>::from_str_radix(digits, radix).map_err(|e| format!("'{raw}': {e}"))
            }
        }
    )*};
}

int_arg!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

/// Variable-length hex blob, optional `0x` prefix.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Hex(pub Vec<u8>);

impl FromArg for Hex {
    const KIND: &'static str = "hex";

    fn from_arg(raw: &str) -> Result<Self, String> {
        hex::decode(raw).map(Self)
    }
}

/// Fixed-width hex blob, e.g. `HexArray<32>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HexArray<const N: usize>(pub [u8; N]);

impl<const N: usize> FromArg for HexArray<N> {
    const KIND: &'static str = "hex";

    fn from_arg(raw: &str) -> Result<Self, String> {
        let bytes = hex::decode(raw)?;
        let len = bytes.len();
        bytes
            .try_into()
            .map(Self)
            .map_err(|_| format!("expected {N} bytes, got {len}"))
    }
}

/// Hex without a dependency.
pub mod hex {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_whitespace_and_keeps_the_raw_tail() {
        let args = Args::parse("  aa   bb cc ");
        assert_eq!(args.len(), 3);
        assert_eq!(args.raw(), "aa   bb cc");
        assert_eq!(args.rest(1), "bb cc");
    }

    #[test]
    fn integers_in_three_radixes() {
        assert_eq!(Args::parse("0x10").get::<u64>(0).unwrap(), 16);
        assert_eq!(Args::parse("0b101").get::<u64>(0).unwrap(), 5);
        assert_eq!(Args::parse("42").get::<u64>(0).unwrap(), 42);
    }

    #[test]
    fn missing_argument_names_its_position() {
        let err = Args::parse("").get::<u64>(2).unwrap_err();
        assert!(matches!(err, Error::BadArg { index: 2, .. }));
    }

    #[test]
    fn optional_distinguishes_absent_from_malformed() {
        assert_eq!(Args::parse("").opt::<u64>(0).unwrap(), None);
        assert!(Args::parse("zz").opt::<u64>(0).is_err());
    }

    #[test]
    fn hex_round_trips() {
        assert_eq!(hex::decode("0xDEad").unwrap(), [0xde, 0xad]);
        assert_eq!(hex::encode([0xde, 0xad]), "dead");
        assert!(hex::decode("abc").is_err());
        assert!(hex::decode("zz").is_err());
    }

    #[test]
    fn fixed_width_hex_checks_length() {
        assert!(Args::parse("dead").get::<HexArray<2>>(0).is_ok());
        assert!(Args::parse("dead").get::<HexArray<3>>(0).is_err());
    }
}
