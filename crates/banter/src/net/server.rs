//! Accept connections and serve each one on its own task.

use std::{
    borrow::Cow,
    io,
    sync::{
        Arc,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
};

use super::{Event, Listener, Log, Observer};
use crate::{app::App, output::Role, session::Session};

type Job = Box<dyn FnOnce() + Send>;

/// Runs a session somewhere. The default spawns an OS thread per connection.
pub trait Spawn: Send + Sync {
    fn spawn(&self, job: Job);
}

impl<F> Spawn for F
where
    F: Fn(Job) + Send + Sync,
{
    fn spawn(&self, job: Job) {
        self(job)
    }
}

/// One OS thread per connection.
#[derive(Clone, Copy, Debug, Default)]
pub struct Threads;

impl Spawn for Threads {
    fn spawn(&self, job: Job) {
        std::thread::spawn(job);
    }
}

pub struct Server<S, L> {
    app: Arc<App<S>>,
    listener: L,
    limit: usize,
    busy: Option<Cow<'static, str>>,
    observer: Arc<dyn Observer>,
    spawn: Box<dyn Spawn>,
}

impl<S: Send + 'static, L: Listener> Server<S, L> {
    pub const LIMIT: usize = 64;

    pub fn new(app: impl Into<Arc<App<S>>>, listener: L) -> Self {
        Self {
            app: app.into(),
            listener,
            limit: Self::LIMIT,
            busy: Some("busy, try again shortly".into()),
            observer: Arc::new(Log),
            spawn: Box::new(Threads),
        }
    }

    /// Concurrent sessions; past it, peers get the busy message and are dropped.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Written in [`Role::Error`] to peers over the limit. `None` just drops them.
    pub fn busy(mut self, message: Option<impl Into<Cow<'static, str>>>) -> Self {
        self.busy = message.map(Into::into);
        self
    }

    pub fn observer(mut self, observer: impl Observer + 'static) -> Self {
        self.observer = Arc::new(observer);
        self
    }

    pub fn spawn(mut self, spawn: impl Spawn + 'static) -> Self {
        self.spawn = Box::new(spawn);
        self
    }

    /// Accept forever. Only returns if the listener cannot be described.
    pub fn run(mut self) -> io::Result<()> {
        self.observer
            .event(Event::Listening(&self.listener.local()));
        let live = Arc::new(AtomicUsize::new(0));
        let next = AtomicU64::new(0);

        loop {
            let conn = match self.listener.accept() {
                Ok(conn) => conn.id(next.fetch_add(1, Ordering::Relaxed)),
                Err(err) => {
                    self.observer.event(Event::Failed(&err));
                    continue;
                }
            };
            let info = conn.info().clone();

            if live.fetch_add(1, Ordering::SeqCst) >= self.limit {
                live.fetch_sub(1, Ordering::SeqCst);
                self.observer.event(Event::Busy(&info));
                if let Some(message) = &self.busy {
                    let mut session = Session::new(conn, self.app.settings().clone());
                    let _ = session.styled(Role::Error, message);
                }
                continue;
            }

            self.observer.event(Event::Accepted(&info));
            let app = Arc::clone(&self.app);
            let live = Arc::clone(&live);
            let observer = Arc::clone(&self.observer);
            self.spawn.spawn(Box::new(move || {
                let outcome = app.serve(conn);
                observer.event(Event::Closed(&info, outcome.as_ref().err()));
                live.fetch_sub(1, Ordering::SeqCst);
            }));
        }
    }
}
