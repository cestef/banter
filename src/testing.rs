//! Drive an [`App`] in-process, for tests and reference clients.
//!
//! [`transcript`] feeds a fixed script and returns everything written. [`Client`]
//! serves a real session on a background thread and talks to it line by line, for
//! when the next command depends on the last reply. Neither touches the network.

use std::{
    collections::VecDeque,
    io::{Read, Write},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
    time::Duration,
};

use crate::{
    app::App,
    error::{Error, Result},
    session::{Conn, ConnInfo},
};

/// Run one session on `input` and return what it wrote.
pub fn transcript<S: 'static>(app: &App<S>, input: &str) -> Result<String> {
    let out = Pipe::default();
    let conn = Conn::new(
        std::io::Cursor::new(input.as_bytes().to_vec()),
        Writer(out.clone()),
        ConnInfo::default(),
    );
    app.serve(conn)?;
    out.close();
    let mut text = String::new();
    Reader(out, None).read_to_string(&mut text)?;
    Ok(text)
}

/// A peer on the other end of an in-memory session.
pub struct Client {
    reader: Reader,
    writer: Option<Writer>,
    prompt: String,
    greeting: String,
    server: Option<JoinHandle<Result<()>>>,
}

impl Client {
    /// How long a single read may wait before the test fails instead of hanging.
    pub const TIMEOUT: Duration = Duration::from_secs(30);

    /// Start a session and read up to the first prompt.
    pub fn connect<S: 'static>(app: impl Into<Arc<App<S>>>) -> Result<Self> {
        let mut client = Self::open(app, ConnInfo::default());
        client.greeting = client.until(&client.prompt.clone())?;
        Ok(client)
    }

    /// Start a session without reading anything, for driving a gate by hand.
    pub fn open<S: 'static>(app: impl Into<Arc<App<S>>>, info: ConnInfo) -> Self {
        let app = app.into();
        let (up, down) = (Pipe::default(), Pipe::default());
        let conn = Conn::new(Reader(up.clone(), None), Writer(down.clone()), info);
        let prompt = app.painted_prompt();
        let server = std::thread::spawn(move || app.serve(conn));
        Self {
            reader: Reader(down, Some(Self::TIMEOUT)),
            writer: Some(Writer(up)),
            prompt,
            greeting: String::new(),
            server: Some(server),
        }
    }

    /// Read up to the next prompt.
    pub fn prompt(&mut self) -> Result<String> {
        let prompt = self.prompt.clone();
        self.until(&prompt)
    }

    /// Everything before the first prompt.
    pub fn greeting(&self) -> &str {
        &self.greeting
    }

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

    /// Send a command, return its reply up to the next prompt.
    pub fn cmd(&mut self, line: &str) -> Result<String> {
        self.send(line)?;
        self.prompt()
    }

    /// Send a command that ends the session, return everything after it, and wait
    /// for the server side to finish.
    pub fn finish(mut self, line: &str) -> Result<String> {
        self.send(line)?;
        self.writer = None;
        let mut rest = String::new();
        self.reader.read_to_string(&mut rest)?;
        if let Some(server) = self.server.take() {
            server
                .join()
                .map_err(|_| Error::Fatal("session panicked".into()))??;
        }
        Ok(rest)
    }
}

/// A one-way in-memory byte channel. Reads block until data arrives or every
/// writer is gone.
#[derive(Clone, Default)]
struct Pipe(Arc<(Mutex<Buffer>, Condvar)>);

/// Bytes in flight, and whether the writing end is gone.
type Buffer = (VecDeque<u8>, bool);

impl Pipe {
    fn close(&self) {
        self.0.0.lock().unwrap().1 = true;
        self.0.1.notify_all();
    }
}

struct Reader(Pipe, Option<Duration>);

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let (lock, ready) = &*self.0.0;
        let mut guard = lock.lock().unwrap();
        while guard.0.is_empty() && !guard.1 {
            guard = match self.1 {
                None => ready.wait(guard).unwrap(),
                Some(timeout) => {
                    let (guard, waited) = ready.wait_timeout(guard, timeout).unwrap();
                    if waited.timed_out() {
                        return Err(std::io::ErrorKind::TimedOut.into());
                    }
                    guard
                }
            };
        }
        let n = buf.len().min(guard.0.len());
        for (slot, byte) in buf.iter_mut().zip(guard.0.drain(..n)) {
            *slot = byte;
        }
        Ok(n)
    }
}

struct Writer(Pipe);

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.0.0.lock().unwrap().0.extend(buf);
        self.0.0.1.notify_all();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.0.close();
    }
}
