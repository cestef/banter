//! A connection: bytes in each direction and what is known about the far end.

use std::{
    fmt,
    io::{BufRead, BufReader, Read, Write},
    net::SocketAddr,
    path::PathBuf,
};

/// Where a connection came from.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Peer {
    /// stdio, an in-memory pipe, or anything else without an address.
    #[default]
    Local,
    /// A TCP client.
    Tcp(SocketAddr),
    /// A Unix socket client; unnamed clients have no path.
    Unix(Option<PathBuf>),
    /// Whatever a custom listener wants to call it.
    Other(String),
}

impl fmt::Display for Peer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Local => f.write_str("local"),
            Self::Tcp(addr) => addr.fmt(f),
            Self::Unix(Some(path)) => write!(f, "unix:{}", path.display()),
            Self::Unix(None) => f.write_str("unix"),
            Self::Other(text) => f.write_str(text),
        }
    }
}

/// Identity of one connection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Info {
    /// Per-server connection number, starting at zero.
    pub id: u64,
    /// Where it came from.
    pub peer: Peer,
}

/// A byte stream in each direction, and who is on the other end.
pub struct Conn {
    pub(crate) reader: Box<dyn BufRead + Send>,
    pub(crate) writer: Box<dyn Write + Send>,
    pub(crate) info: Info,
}

impl Conn {
    /// Buffer `reader` and pair it with `writer`, with id zero.
    pub fn new(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        peer: Peer,
    ) -> Self {
        Self {
            reader: Box::new(BufReader::new(reader)),
            writer: Box::new(writer),
            info: Info { id: 0, peer },
        }
    }

    /// This process's stdin and stdout.
    pub fn stdio() -> Self {
        Self::new(std::io::stdin(), std::io::stdout(), Peer::Local)
    }

    /// Set the connection number.
    pub fn id(mut self, id: u64) -> Self {
        self.info.id = id;
        self
    }

    /// Identity of this connection.
    pub fn info(&self) -> &Info {
        &self.info
    }
}
