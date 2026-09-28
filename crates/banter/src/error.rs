//! One error type for the whole session.
//!
//! Errors split in two by [`Error::recoverable`]: the ones the peer caused and can
//! fix (a typo, a bad argument, a refused request) are reported in place and the
//! session goes on; everything else ends it. How any of them look on the wire is up
//! to the app's [`Reporter`](crate::Reporter).

use std::{fmt, io};

use crate::command::Spec;

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
#[non_exhaustive]
/// Everything that can go wrong in a session.
pub enum Error {
    /// A handler refused the request. Session continues.
    Reject(String),
    /// No command by that name.
    UnknownCommand {
        /// The token as typed.
        got: String,
        /// A close match, if any.
        suggestion: Option<String>,
    },
    /// Too few arguments for this command.
    Usage(Box<Spec>),
    /// An argument failed to parse.
    BadArg {
        /// Position among the arguments, from zero.
        index: usize,
        /// The parameter's name, when the command declares one there.
        param: Option<&'static str>,
        /// The expected shape, from [`FromArg::KIND`](crate::FromArg::KIND).
        kind: &'static str,
        /// Why it failed.
        reason: String,
    },
    /// The line could not be tokenized, e.g. an unterminated quote.
    Syntax(String),
    /// A gate refused the connection before the session began.
    Denied(String),
    /// The peer sent a line longer than the configured cap.
    LineTooLong(usize),
    /// The peer went quiet for longer than the idle timeout.
    Timeout,
    /// The peer hung up.
    Eof,
    /// Reading or writing failed. Ends the session.
    Io(io::Error),
    /// Anything else. Ends the session.
    Other(BoxError),
}

impl Error {
    /// [`Error::Reject`] with this message.
    pub fn reject(message: impl Into<String>) -> Self {
        Self::Reject(message.into())
    }

    /// [`Error::Denied`] with this message.
    pub fn denied(message: impl Into<String>) -> Self {
        Self::Denied(message.into())
    }

    /// Wrap any error, or a plain message, as a session-ending failure.
    pub fn other(err: impl Into<BoxError>) -> Self {
        Self::Other(err.into())
    }

    /// A follow-up for the peer, if this error has an obvious next step.
    pub fn hint(&self) -> Option<Hint> {
        match self {
            Self::UnknownCommand {
                suggestion: Some(name),
                ..
            } => Some(Hint::Suggest(name.clone())),
            Self::UnknownCommand { .. } => Some(Hint::Help),
            _ => None,
        }
    }

    /// Whether the loop should report this and keep going.
    pub fn recoverable(&self) -> bool {
        matches!(
            self,
            Self::Reject(_)
                | Self::UnknownCommand { .. }
                | Self::Usage(_)
                | Self::BadArg { .. }
                | Self::Syntax(_)
        )
    }

    /// Whether this ends the session without anything having gone wrong
    /// server-side: the peer left, stalled, flooded, or failed a gate.
    pub fn benign(&self) -> bool {
        matches!(
            self,
            Self::Eof | Self::Timeout | Self::LineTooLong(_) | Self::Denied(_)
        )
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reject(msg) | Self::Denied(msg) | Self::Syntax(msg) => f.write_str(msg),
            Self::UnknownCommand { got, .. } => write!(f, "unknown command '{got}'"),
            Self::Usage(spec) => write!(f, "usage: {}", spec.signature()),
            Self::BadArg {
                param: Some(name),
                kind,
                reason,
                ..
            } => write!(f, "{name} ({kind}): {reason}"),
            Self::BadArg {
                index,
                param: None,
                kind,
                reason,
            } => write!(f, "argument {index} ({kind}): {reason}"),
            Self::LineTooLong(max) => write!(f, "line exceeds {max} bytes"),
            Self::Timeout => f.write_str("idle for too long"),
            Self::Eof => f.write_str("connection closed"),
            Self::Io(err) => err.fmt(f),
            Self::Other(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::Other(err) => Some(&**err),
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

/// What to try next after an error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Hint {
    /// A command close to what was typed.
    Suggest(String),
    /// Nothing close; point at the command list.
    Help,
}

impl fmt::Display for Hint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Suggest(name) => write!(f, "did you mean '{name}'?"),
            Self::Help => f.write_str("try 'help'"),
        }
    }
}

/// Result defaulting to [`Error`].
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
