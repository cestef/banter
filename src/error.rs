//! One error type for the whole session.
//!
//! Errors split in two by [`Error::recoverable`]: the ones the peer caused and can
//! fix (a typo, a bad argument, a refused request) are rendered in place and the
//! session goes on; everything else ends it.

use std::{fmt, io};

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The handler refused the request. Shown to the peer, session continues.
    Reject(String),
    /// No command by that name.
    UnknownCommand {
        got: String,
        suggestion: Option<&'static str>,
    },
    /// Too few arguments; carries the command's signature.
    Usage(String),
    /// An argument failed to parse.
    BadArg {
        index: usize,
        kind: &'static str,
        reason: String,
    },
    /// A gate refused the connection before the session began.
    Denied(String),
    /// The peer sent a line longer than the configured cap.
    LineTooLong(usize),
    /// The peer went quiet for longer than the idle timeout.
    Timeout,
    /// The peer hung up.
    Eof,
    Io(io::Error),
    /// Anything else that should end the session.
    Fatal(String),
}

impl Error {
    /// Whether the command loop should report this and keep going.
    pub fn recoverable(&self) -> bool {
        matches!(
            self,
            Self::Reject(_) | Self::UnknownCommand { .. } | Self::Usage(_) | Self::BadArg { .. }
        )
    }

    /// Whether this ends the session without anything having gone wrong server-side.
    pub fn benign(&self) -> bool {
        matches!(
            self,
            Self::Eof | Self::Timeout | Self::LineTooLong(_) | Self::Denied(_)
        )
    }

    /// A follow-up line worth showing under the error, if any.
    pub fn hint(&self) -> Option<String> {
        match self {
            Self::UnknownCommand {
                suggestion: Some(s),
                ..
            } => Some(format!("did you mean '{s}'?")),
            Self::UnknownCommand { .. } => Some("try 'help'".into()),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reject(msg) | Self::Denied(msg) | Self::Fatal(msg) => f.write_str(msg),
            Self::UnknownCommand { got, .. } => write!(f, "unknown command '{got}'"),
            Self::Usage(signature) => write!(f, "usage: {signature}"),
            Self::BadArg {
                index,
                kind,
                reason,
            } => write!(f, "argument {index} ({kind}): {reason}"),
            Self::LineTooLong(max) => write!(f, "line exceeds {max} bytes"),
            Self::Timeout => f.write_str("idle for too long"),
            Self::Eof => f.write_str("connection closed"),
            Self::Io(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        match err.kind() {
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => Self::Timeout,
            io::ErrorKind::UnexpectedEof => Self::Eof,
            _ => Self::Io(err),
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Refuse the current request and keep the session alive.
///
/// ```
/// # fn f(i: u32) -> banter::Result<()> {
/// if i > 63 {
///     banter::reject!("index {i} is out of range");
/// }
/// # Ok(()) }
/// ```
#[macro_export]
macro_rules! reject {
    ($($arg:tt)*) => {
        return ::core::result::Result::Err($crate::Error::Reject(format!($($arg)*)))
    };
}

/// [`reject!`] unless a condition holds.
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $($arg:tt)*) => {
        if !($cond) {
            $crate::reject!($($arg)*);
        }
    };
}
