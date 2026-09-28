//! One peer's session: its connection, settings and store.

mod conn;
mod settings;
mod store;

pub use conn::{Conn, Info, Peer};
pub use settings::{MAX_LINE, Newline, Settings};
pub use store::Store;

use std::io::{BufRead, Read};

use crate::{
    error::{Error, Result},
    output::{Block, Cell, Role, Style, Table},
};

/// The handle every handler, gate and hook reads and writes through.
///
/// Every write flushes: a peer is waiting on the other end, and a buffered prompt
/// looks like a hang.
pub struct Session {
    conn: Conn,
    settings: Settings,
    store: Store,
}

impl Session {
    /// A fresh session over `conn`, with an empty store.
    pub fn new(conn: Conn, settings: Settings) -> Self {
        Self {
            conn,
            settings,
            store: Store::new(),
        }
    }

    /// Identity of the connection.
    pub fn info(&self) -> &Info {
        &self.conn.info
    }

    /// Typed storage shared by gates, hooks and handlers for this session.
    pub fn store(&self) -> &Store {
        &self.store
    }

    /// Mutable access to the [`Store`].
    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    /// Current settings.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// Change style, layout, line ending or line cap from here on.
    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }

    /// The current style.
    pub fn style(&self) -> &dyn Style {
        &*self.settings.style
    }

    /// Swap the style from here on.
    pub fn restyle(&mut self, style: impl Style + 'static) {
        self.settings.style = std::sync::Arc::new(style);
    }

    /// `text` painted as `role`, without writing it.
    pub fn paint(&self, role: Role, text: &str) -> String {
        self.settings.style.paint(role, text).into_owned()
    }

    // -- input ------------------------------------------------------------

    /// One line without its terminator. A peer that never sends a newline cannot
    /// make this allocate past the line cap.
    pub fn line(&mut self) -> Result<String> {
        let max = self.settings.max_line;
        let mut buf = Vec::with_capacity(64);
        let read = (&mut self.conn.reader)
            .take(max as u64 + 1)
            .read_until(b'\n', &mut buf)?;
        if read == 0 {
            return Err(Error::Eof);
        }
        if buf.len() > max {
            return Err(Error::LineTooLong(max));
        }
        let text = String::from_utf8_lossy(&buf);
        Ok(text.trim_end_matches(['\n', '\r']).to_string())
    }

    /// Print `prompt` without a line ending, then read the answer.
    pub fn ask(&mut self, prompt: &str) -> Result<String> {
        let painted = self.paint(Role::Prompt, prompt);
        self.write(&painted)?;
        self.line()
    }

    // -- output -----------------------------------------------------------

    /// Raw text: no line ending, no styling.
    pub fn write(&mut self, text: &str) -> Result<()> {
        self.bytes(text.as_bytes())
    }

    /// Raw bytes, for protocols that are not all text.
    pub fn bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.conn.writer.write_all(bytes)?;
        self.conn.writer.flush()?;
        Ok(())
    }

    /// One line in the given role.
    pub fn styled(&mut self, role: Role, line: impl AsRef<str>) -> Result<()> {
        let painted = self.paint(role, line.as_ref());
        self.lines(&[painted])
    }

    /// One line of [`Role::Text`].
    pub fn say(&mut self, line: impl AsRef<str>) -> Result<()> {
        self.styled(Role::Text, line)
    }

    /// One line of [`Role::Ok`].
    pub fn ok(&mut self, line: impl AsRef<str>) -> Result<()> {
        self.styled(Role::Ok, line)
    }

    /// One line of [`Role::Warn`].
    pub fn warn(&mut self, line: impl AsRef<str>) -> Result<()> {
        self.styled(Role::Warn, line)
    }

    /// One line of [`Role::Dim`].
    pub fn note(&mut self, line: impl AsRef<str>) -> Result<()> {
        self.styled(Role::Dim, line)
    }

    /// An empty line.
    pub fn blank(&mut self) -> Result<()> {
        self.lines(&[String::new()])
    }

    /// One `key  value` line.
    pub fn kv(&mut self, key: impl Into<String>, value: impl Into<Cell>) -> Result<()> {
        self.block(&Block::new().entry(key, value))
    }

    /// A [`Block`], laid out by the session's [`Layout`](crate::Layout).
    pub fn block(&mut self, block: &Block) -> Result<()> {
        let lines = block.render(self.style(), self.settings.layout);
        self.lines(&lines)
    }

    /// A [`Table`], laid out by the session's [`Layout`](crate::Layout).
    pub fn table(&mut self, table: &Table) -> Result<()> {
        let lines = table.render(self.style(), self.settings.layout);
        self.lines(&lines)
    }

    /// Already-painted lines, each followed by the line ending, in one write.
    pub fn lines(&mut self, lines: &[String]) -> Result<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let newline = self.settings.newline.as_str();
        let out: String = lines.iter().flat_map(|l| [l.as_str(), newline]).collect();
        self.write(&out)
    }
}
