//! A fixed input, run to the end.

use std::io::{Cursor, Read};

use super::pipe::Pipe;
use crate::{
    app::App,
    error::Result,
    session::{Conn, Peer},
};

/// Feed a whole input to one session and collect what it wrote.
///
/// ```
/// # use banter::{App, testing::Script};
/// let app = App::new(|_| Ok(()));
/// let out = Script::new("help\n").run(&app).unwrap();
/// assert!(out.contains("exit"));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Script {
    input: String,
    peer: Peer,
    id: u64,
}

impl Script {
    /// Feed `input` as is; end every line with `\n`.
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            ..Self::default()
        }
    }

    /// Pretend to come from somewhere.
    pub fn peer(mut self, peer: Peer) -> Self {
        self.peer = peer;
        self
    }

    /// Pretend to be this connection number.
    pub fn id(mut self, id: u64) -> Self {
        self.id = id;
        self
    }

    /// Serve one session over the input and return everything it wrote.
    pub fn run<S>(&self, app: &App<S>) -> Result<String> {
        let out = Pipe::default();
        let conn = Conn::new(
            Cursor::new(self.input.clone().into_bytes()),
            out.writer(),
            self.peer.clone(),
        )
        .id(self.id);
        app.serve(conn)?;
        let mut text = String::new();
        out.reader(None).read_to_string(&mut text)?;
        Ok(text)
    }
}
