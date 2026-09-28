//! An interactive peer for an in-memory session.

use std::{
    io::{Read, Write},
    sync::Arc,
    thread::JoinHandle,
    time::Duration,
};

use super::pipe::{Pipe, Reader, Writer};
use crate::{
    app::App,
    error::{Error, Result},
    session::{Conn, Peer},
};

/// Serves one session on a background thread and talks to it line by line, for
/// when the next command depends on the last reply.
pub struct Client {
    reader: Reader,
    writer: Option<Writer>,
    prompt: String,
    greeting: String,
    server: Option<JoinHandle<Result<()>>>,
}

impl Client {
    /// How long one read may wait before failing instead of hanging a test.
    pub const TIMEOUT: Duration = Duration::from_secs(30);

    /// Start a session and read up to the first prompt. Needs a static prompt;
    /// otherwise use [`Self::open`] and [`Self::expect`].
    pub fn connect<S: Send + 'static>(app: impl Into<Arc<App<S>>>) -> Result<Self> {
        Self::open(app, Peer::Local).greet()
    }

    /// Start a session without reading anything, e.g. to answer a gate first.
    pub fn open<S: Send + 'static>(app: impl Into<Arc<App<S>>>, peer: Peer) -> Self {
        let app = app.into();
        let (up, down) = (Pipe::default(), Pipe::default());
        let conn = Conn::new(up.reader(None), down.writer(), peer);
        let prompt = app.wire_prompt().unwrap_or_default();
        let server = std::thread::spawn(move || app.serve(conn));
        Self {
            reader: down.reader(Some(Self::TIMEOUT)),
            writer: Some(up.writer()),
            prompt,
            greeting: String::new(),
            server: Some(server),
        }
    }

    /// The prompt to wait for, as it appears on the wire.
    pub fn expect(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Read the greeting, up to the first prompt.
    pub fn greet(mut self) -> Result<Self> {
        self.greeting = self.prompt()?;
        Ok(self)
    }

    /// What the session wrote before its first prompt.
    pub fn greeting(&self) -> &str {
        &self.greeting
    }

    /// Write one line to the session.
    pub fn send(&mut self, line: &str) -> Result<()> {
        let writer = self.writer.as_mut().ok_or(Error::Eof)?;
        writer.write_all(line.as_bytes())?;
        writer.write_all(b"\n")?;
        Ok(())
    }

    /// Read until `needle`, returning what came before it.
    pub fn until(&mut self, needle: &str) -> Result<String> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        while !buf.ends_with(needle.as_bytes()) {
            if self.reader.read(&mut byte)? == 0 {
                return Err(Error::Eof);
            }
            buf.push(byte[0]);
        }
        buf.truncate(buf.len() - needle.len());
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Read up to the next prompt.
    pub fn prompt(&mut self) -> Result<String> {
        if self.prompt.is_empty() {
            return Err(Error::other("no prompt to wait for; call expect()"));
        }
        let prompt = self.prompt.clone();
        self.until(&prompt)
    }

    /// Send a command, return its reply up to the next prompt.
    pub fn cmd(&mut self, line: &str) -> Result<String> {
        self.send(line)?;
        self.prompt()
    }

    /// Send a last command, return everything after it, and wait for the server
    /// side to finish.
    pub fn finish(mut self, line: &str) -> Result<String> {
        self.send(line)?;
        self.writer = None;
        let mut rest = String::new();
        self.reader.read_to_string(&mut rest)?;
        self.server
            .take()
            .expect("joined once")
            .join()
            .map_err(|_| Error::other("session panicked"))??;
        Ok(rest)
    }
}
