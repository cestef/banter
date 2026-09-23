//! What a server reports about itself.

use std::io;

use crate::{error::Error, session::Info};

/// Something a server did.
#[derive(Debug)]
#[non_exhaustive]
pub enum Event<'a> {
    Listening(&'a str),
    Accepted(&'a Info),
    /// Turned away at the connection limit.
    Busy(&'a Info),
    /// A session ended; the error is present when it was not a normal end.
    Closed(&'a Info, Option<&'a Error>),
    /// `accept` itself failed; the server keeps going.
    Failed(&'a io::Error),
}

pub trait Observer: Send + Sync {
    fn event(&self, event: Event<'_>);
}

impl<F> Observer for F
where
    F: Fn(Event<'_>) + Send + Sync,
{
    fn event(&self, event: Event<'_>) {
        self(event)
    }
}

/// Startup, failures and turned-away peers on stderr. The default.
#[derive(Clone, Copy, Debug, Default)]
pub struct Log;

impl Observer for Log {
    fn event(&self, event: Event<'_>) {
        match event {
            Event::Listening(addr) => eprintln!("listening on {addr}"),
            Event::Busy(info) => eprintln!("#{} {}: busy", info.id, info.peer),
            Event::Closed(info, Some(err)) => eprintln!("#{} {}: {err}", info.id, info.peer),
            Event::Failed(err) => eprintln!("accept: {err}"),
            _ => {}
        }
    }
}

/// Says nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Silent;

impl Observer for Silent {
    fn event(&self, _: Event<'_>) {}
}
