//! Per-session presentation and limits.

use std::sync::Arc;

use crate::output::{Layout, Plain, Style};

/// Default cap on one line of input, before any parsing happens.
pub const MAX_LINE: usize = 4096;

/// Line terminator written after every line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Newline {
    #[default]
    Lf,
    /// For telnet-ish peers.
    CrLf,
}

impl Newline {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }
}

/// What a [`Session`](super::Session) starts with. An [`App`](crate::App) holds
/// one and hands every session a copy, which the session may then change.
#[derive(Clone)]
pub struct Settings {
    pub style: Arc<dyn Style>,
    pub layout: Layout,
    pub newline: Newline,
    /// Longest input line accepted, in bytes. Longer ends the session.
    pub max_line: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            style: Arc::new(Plain),
            layout: Layout::default(),
            newline: Newline::Lf,
            max_line: MAX_LINE,
        }
    }
}
