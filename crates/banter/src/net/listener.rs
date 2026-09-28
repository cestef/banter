//! Sources of connections.

use std::{
    io,
    net::{TcpListener, TcpStream, ToSocketAddrs},
    time::Duration,
};

use crate::session::{Conn, Peer};

/// Anything that yields connections, one blocking `accept` at a time.
pub trait Listener: Send {
    /// Wait for the next connection.
    fn accept(&mut self) -> io::Result<Conn>;

    /// Where it listens, for logs.
    fn local(&self) -> String;
}

/// TCP, with an idle timeout applied to every accepted stream.
pub struct Tcp {
    inner: TcpListener,
    idle: Option<Duration>,
}

impl Tcp {
    /// Default idle timeout.
    pub const IDLE: Duration = Duration::from_secs(120);

    /// Listen on `addr`, with the default idle timeout.
    pub fn bind(addr: impl ToSocketAddrs) -> io::Result<Self> {
        Ok(Self::from(TcpListener::bind(addr)?))
    }

    /// Drop a peer silent for this long. `None` waits forever.
    pub fn idle(mut self, idle: Option<Duration>) -> Self {
        self.idle = idle;
        self
    }

    /// Wrap an accepted stream, applying `idle` to reads and writes.
    pub fn conn(stream: TcpStream, idle: Option<Duration>) -> io::Result<Conn> {
        stream.set_nodelay(true)?;
        stream.set_read_timeout(idle)?;
        stream.set_write_timeout(idle)?;
        let peer = stream.peer_addr().map_or(Peer::Local, Peer::Tcp);
        Ok(Conn::new(stream.try_clone()?, stream, peer))
    }
}

impl From<TcpListener> for Tcp {
    fn from(inner: TcpListener) -> Self {
        Self {
            inner,
            idle: Some(Self::IDLE),
        }
    }
}

impl Listener for Tcp {
    fn accept(&mut self) -> io::Result<Conn> {
        let (stream, _) = self.inner.accept()?;
        Self::conn(stream, self.idle)
    }

    fn local(&self) -> String {
        self.inner
            .local_addr()
            .map_or_else(|_| "tcp".into(), |a| a.to_string())
    }
}

#[cfg(unix)]
pub use unix::Unix;

#[cfg(unix)]
mod unix {
    use std::{
        io,
        os::unix::net::{UnixListener, UnixStream},
        path::Path,
        time::Duration,
    };

    use super::{Listener, Tcp};
    use crate::session::{Conn, Peer};

    /// A Unix domain socket.
    pub struct Unix {
        inner: UnixListener,
        idle: Option<Duration>,
    }

    impl Unix {
        /// Listen on `path`, with the default idle timeout.
        pub fn bind(path: impl AsRef<Path>) -> io::Result<Self> {
            Ok(Self::from(UnixListener::bind(path)?))
        }

        /// Drop a peer silent for this long. `None` waits forever.
        pub fn idle(mut self, idle: Option<Duration>) -> Self {
            self.idle = idle;
            self
        }

        /// Wrap an accepted stream, applying `idle` to reads and writes.
        pub fn conn(stream: UnixStream, idle: Option<Duration>) -> io::Result<Conn> {
            stream.set_read_timeout(idle)?;
            stream.set_write_timeout(idle)?;
            let path = stream
                .peer_addr()
                .ok()
                .and_then(|a| a.as_pathname().map(Path::to_path_buf));
            Ok(Conn::new(stream.try_clone()?, stream, Peer::Unix(path)))
        }
    }

    impl From<UnixListener> for Unix {
        fn from(inner: UnixListener) -> Self {
            Self {
                inner,
                idle: Some(Tcp::IDLE),
            }
        }
    }

    impl Listener for Unix {
        fn accept(&mut self) -> io::Result<Conn> {
            let (stream, _) = self.inner.accept()?;
            Self::conn(stream, self.idle)
        }

        fn local(&self) -> String {
            self.inner
                .local_addr()
                .ok()
                .and_then(|a| a.as_pathname().map(|p| format!("unix:{}", p.display())))
                .unwrap_or_else(|| "unix".into())
        }
    }
}
