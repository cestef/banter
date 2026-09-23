//! Typed access to a command's arguments.
//!
//! Handlers ask for the type they want and get a positioned [`Error::BadArg`] when
//! the peer sent something else. Implement [`FromArg`] for domain types to keep
//! parsing out of handlers entirely.

use super::hex;
use crate::error::{Error, Result};

/// A type a single argument token can be parsed into.
pub trait FromArg: Sized {
    /// Name of the expected shape, shown in errors.
    const KIND: &'static str;

    fn from_arg(raw: &str) -> Result<Self, String>;
}

/// The tokens following the command name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Args {
    tokens: Vec<String>,
}

impl Args {
    pub fn new(tokens: Vec<String>) -> Self {
        Self { tokens }
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn token(&self, index: usize) -> Option<&str> {
        self.tokens.get(index).map(String::as_str)
    }

    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }

    /// Required argument at `index`.
    pub fn get<T: FromArg>(&self, index: usize) -> Result<T> {
        let raw = self.token(index).ok_or(Error::BadArg {
            index,
            param: None,
            kind: T::KIND,
            reason: "missing".into(),
        })?;
        T::from_arg(raw).map_err(|reason| Error::BadArg {
            index,
            param: None,
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

    /// Every argument from `index` on, parsed.
    pub fn all<T: FromArg>(&self, from: usize) -> Result<Vec<T>> {
        (from..self.len()).map(|i| self.get(i)).collect()
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

macro_rules! parse_arg {
    ($($t:ty => $kind:literal),*) => {$(
        impl FromArg for $t {
            const KIND: &'static str = $kind;

            fn from_arg(raw: &str) -> Result<Self, String> {
                raw.parse().map_err(|e| format!("'{raw}': {e}"))
            }
        }
    )*};
}

parse_arg!(
    f32 => "number",
    f64 => "number",
    char => "character",
    std::net::IpAddr => "ip address",
    std::net::SocketAddr => "socket address",
    std::path::PathBuf => "path"
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

#[cfg(test)]
mod tests {
    use super::*;

    fn args(raw: &str) -> Args {
        Args::new(raw.split_whitespace().map(str::to_string).collect())
    }

    #[test]
    fn rest_rejoins_the_tail() {
        assert_eq!(args("aa bb cc").rest(1), "bb cc");
    }

    #[test]
    fn integers_in_three_radixes() {
        assert_eq!(args("0x10").get::<u64>(0).unwrap(), 16);
        assert_eq!(args("0b101").get::<u64>(0).unwrap(), 5);
        assert_eq!(args("42").get::<u64>(0).unwrap(), 42);
    }

    #[test]
    fn missing_argument_names_its_position() {
        let err = args("").get::<u64>(2).unwrap_err();
        assert!(matches!(err, Error::BadArg { index: 2, .. }));
    }

    #[test]
    fn optional_distinguishes_absent_from_malformed() {
        assert_eq!(args("").opt::<u64>(0).unwrap(), None);
        assert!(args("zz").opt::<u64>(0).is_err());
    }

    #[test]
    fn all_parses_the_tail() {
        assert_eq!(args("x 1 2 3").all::<u8>(1).unwrap(), [1, 2, 3]);
        assert!(args("x 1 z").all::<u8>(1).is_err());
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
        assert!(args("dead").get::<HexArray<2>>(0).is_ok());
        assert!(args("dead").get::<HexArray<3>>(0).is_err());
    }
}
