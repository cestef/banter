//! A one-way in-memory byte channel.

use std::{
    collections::VecDeque,
    io::{self, Read, Write},
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

/// Bytes in flight, and whether the writing end is gone.
#[derive(Default)]
struct Buffer {
    bytes: VecDeque<u8>,
    closed: bool,
}

#[derive(Clone, Default)]
pub(crate) struct Pipe(Arc<(Mutex<Buffer>, Condvar)>);

impl Pipe {
    pub fn reader(&self, timeout: Option<Duration>) -> Reader {
        Reader {
            pipe: self.clone(),
            timeout,
        }
    }

    pub fn writer(&self) -> Writer {
        Writer(self.clone())
    }

    fn close(&self) {
        self.0.0.lock().unwrap().closed = true;
        self.0.1.notify_all();
    }
}

/// Blocks until bytes arrive, the writer is dropped, or the timeout passes.
pub(crate) struct Reader {
    pipe: Pipe,
    timeout: Option<Duration>,
}

impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let (lock, ready) = &*self.pipe.0;
        let mut guard = lock.lock().unwrap();
        while guard.bytes.is_empty() && !guard.closed {
            guard = match self.timeout {
                None => ready.wait(guard).unwrap(),
                Some(timeout) => {
                    let (guard, waited) = ready.wait_timeout(guard, timeout).unwrap();
                    if waited.timed_out() {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    guard
                }
            };
        }
        let n = buf.len().min(guard.bytes.len());
        for (slot, byte) in buf.iter_mut().zip(guard.bytes.drain(..n)) {
            *slot = byte;
        }
        Ok(n)
    }
}

/// Closes the pipe when dropped.
pub(crate) struct Writer(Pipe);

impl Write for Writer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.0.0.lock().unwrap().bytes.extend(buf);
        self.0.0.1.notify_all();
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.0.close();
    }
}
