//! Thread-per-connection TCP server.
//!
//! Threads keep the crate free of an async runtime, and at interactive-service
//! concurrency (tens of peers) they cost nothing worth measuring.

use std::{
    io::Write,
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use crate::{app::App, session::Conn};

pub struct Server<S> {
    app: Arc<App<S>>,
    max_conns: usize,
    idle: Option<Duration>,
    busy: String,
    quiet: bool,
}

impl<S: 'static> Server<S> {
    pub fn new(app: impl Into<Arc<App<S>>>) -> Self {
        Self {
            app: app.into(),
            max_conns: 64,
            idle: Some(Duration::from_secs(120)),
            busy: "busy, try again shortly\n".into(),
            quiet: false,
        }
    }

    /// Concurrent sessions; further peers get [`Self::busy_message`] and are dropped.
    pub fn max_conns(mut self, max_conns: usize) -> Self {
        self.max_conns = max_conns;
        self
    }

    /// Drop a peer silent for this long. `None` waits forever.
    pub fn idle_timeout(mut self, idle: Option<Duration>) -> Self {
        self.idle = idle;
        self
    }

    pub fn busy_message(mut self, message: impl Into<String>) -> Self {
        self.busy = message.into();
        self
    }

    /// No operator logging on stderr.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    pub fn bind(self, addr: impl ToSocketAddrs) -> std::io::Result<()> {
        self.serve(TcpListener::bind(addr)?)
    }

    /// Accept forever on an already bound listener.
    pub fn serve(self, listener: TcpListener) -> std::io::Result<()> {
        self.log(format_args!("listening on {}", listener.local_addr()?));
        let live = Arc::new(AtomicUsize::new(0));
        let next = AtomicU64::new(0);

        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(stream) => stream,
                Err(err) => {
                    self.log(format_args!("accept failed: {err}"));
                    continue;
                }
            };
            if live.fetch_add(1, Ordering::SeqCst) >= self.max_conns {
                live.fetch_sub(1, Ordering::SeqCst);
                let _ = stream.write_all(self.busy.as_bytes());
                continue;
            }

            let id = next.fetch_add(1, Ordering::Relaxed);
            let app = Arc::clone(&self.app);
            let live = Arc::clone(&live);
            let idle = self.idle;
            let quiet = self.quiet;
            std::thread::spawn(move || {
                if let Err(err) = Self::session(&app, stream, id, idle)
                    && !quiet
                {
                    eprintln!("session {id}: {err}");
                }
                live.fetch_sub(1, Ordering::SeqCst);
            });
        }
        Ok(())
    }

    fn session(
        app: &App<S>,
        stream: TcpStream,
        id: u64,
        idle: Option<Duration>,
    ) -> crate::Result<()> {
        app.serve(Conn::tcp(stream, id, idle)?)
    }

    fn log(&self, message: std::fmt::Arguments) {
        if !self.quiet {
            eprintln!("{message}");
        }
    }
}
