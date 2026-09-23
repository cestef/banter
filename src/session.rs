//! One peer's connection, and everything a handler writes through.

use std::{
    io::{BufRead, BufReader, Read, Write},
    net::SocketAddr,
    sync::Arc,
};

use crate::{
    error::{Error, Result},
    layout::{Block, Cell, Table},
    style::{Plain, Role, Style},
};

/// Default cap on one line of input, before any parsing happens.
pub const MAX_LINE: usize = 4096;

/// What is known about the far end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConnInfo {
    /// Per-process connection number, starting at zero.
    pub id: u64,
    pub peer: Option<SocketAddr>,
}

/// A byte stream in each direction plus its [`ConnInfo`].
pub struct Conn {
    reader: Box<dyn BufRead + Send>,
    writer: Box<dyn Write + Send>,
    info: ConnInfo,
}

impl Conn {
    pub fn new(
        reader: impl Read + Send + 'static,
        writer: impl Write + Send + 'static,
        info: ConnInfo,
    ) -> Self {
        Self {
            reader: Box::new(BufReader::new(reader)),
            writer: Box::new(writer),
            info,
        }
    }

    pub fn stdio() -> Self {
        Self::new(std::io::stdin(), std::io::stdout(), ConnInfo::default())
    }

    /// One TCP stream. `idle` bounds how long a read or write may block.
    #[cfg(feature = "tcp")]
    pub fn tcp(
        stream: std::net::TcpStream,
        id: u64,
        idle: Option<std::time::Duration>,
    ) -> std::io::Result<Self> {
        stream.set_nodelay(true)?;
        stream.set_read_timeout(idle)?;
        stream.set_write_timeout(idle)?;
        let info = ConnInfo {
            id,
            peer: stream.peer_addr().ok(),
        };
        Ok(Self::new(stream.try_clone()?, stream, info))
    }

    pub fn info(&self) -> ConnInfo {
        self.info
    }
}

/// The handle every handler, gate and greeting writes through.
///
/// Every write flushes: a peer is waiting on the other end, and a buffered prompt
/// looks like a hang.
pub struct Session {
    conn: Conn,
    style: Arc<dyn Style>,
    max_line: usize,
}

impl Session {
    pub fn new(conn: Conn) -> Self {
        Self {
            conn,
            style: Arc::new(Plain),
            max_line: MAX_LINE,
        }
    }

    pub fn with_style(mut self, style: Arc<dyn Style>) -> Self {
        self.style = style;
        self
    }

    pub fn with_max_line(mut self, max_line: usize) -> Self {
        self.max_line = max_line;
        self
    }

    pub fn info(&self) -> ConnInfo {
        self.conn.info
    }

    pub fn style(&self) -> &dyn Style {
        &*self.style
    }

    /// Swap the style for the rest of the session, e.g. from a `color off` command.
    pub fn set_style(&mut self, style: Arc<dyn Style>) {
        self.style = style;
    }

    pub fn paint(&self, role: Role, text: &str) -> String {
        self.style.paint(role, text).into_owned()
    }

    // -- input ------------------------------------------------------------

    /// Read one line without its terminator. A peer that never sends a newline
    /// cannot make this allocate past the line cap.
    pub fn line(&mut self) -> Result<String> {
        let mut buf = Vec::with_capacity(64);
        let cap = self.max_line as u64 + 1;
        let read = (&mut self.conn.reader)
            .take(cap)
            .read_until(b'\n', &mut buf)?;
        if read == 0 {
            return Err(Error::Eof);
        }
        if buf.len() > self.max_line {
            return Err(Error::LineTooLong(self.max_line));
        }
        let text = String::from_utf8_lossy(&buf);
        Ok(text.trim_end_matches(['\n', '\r']).to_string())
    }

    /// Print `prompt` without a newline, then read the answer.
    pub fn ask(&mut self, prompt: &str) -> Result<String> {
        let painted = self.paint(Role::Prompt, prompt);
        self.write(&painted)?;
        self.line()
    }

    // -- output -----------------------------------------------------------

    /// Raw text, no newline, no styling.
    pub fn write(&mut self, text: &str) -> Result<()> {
        self.conn.writer.write_all(text.as_bytes())?;
        self.conn.writer.flush()?;
        Ok(())
    }

    /// One line of prose.
    pub fn say(&mut self, line: impl AsRef<str>) -> Result<()> {
        self.styled(Role::Text, line)
    }

    /// One line in the given role.
    pub fn styled(&mut self, role: Role, line: impl AsRef<str>) -> Result<()> {
        let mut out = self.paint(role, line.as_ref());
        out.push('\n');
        self.write(&out)
    }

    pub fn blank(&mut self) -> Result<()> {
        self.write("\n")
    }

    pub fn ok(&mut self, message: impl AsRef<str>) -> Result<()> {
        self.styled(Role::Ok, message)
    }

    pub fn note(&mut self, message: impl AsRef<str>) -> Result<()> {
        self.styled(Role::Dim, message)
    }

    /// `error: message`, plus a hint line when the error has one.
    pub fn report(&mut self, err: &Error) -> Result<()> {
        self.styled(Role::Error, format!("error: {err}"))?;
        if let Some(hint) = err.hint() {
            self.note(format!("  {hint}"))?;
        }
        Ok(())
    }

    /// One `key  value` line.
    pub fn kv(&mut self, key: impl Into<String>, value: impl Into<Cell>) -> Result<()> {
        self.block(&Block::new().entry(key, value))
    }

    pub fn block(&mut self, block: &Block) -> Result<()> {
        self.lines(block.render(&*self.style))
    }

    pub fn table(&mut self, table: &Table) -> Result<()> {
        self.lines(table.render(&*self.style))
    }

    fn lines(&mut self, lines: Vec<String>) -> Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let mut out = lines.join("\n");
        out.push('\n');
        self.write(&out)
    }
}
