//! Tiny framework for line-based interactive services.
//!
//! Describe a service as per-session state plus a command table; banter owns the
//! prompt loop, argument parsing, help, typo suggestions, error reporting and the
//! transport. The same [`App`] serves stdio, TCP, a Unix socket, or an in-memory
//! test client. Every policy is a trait with a default: [`Lexer`], [`Suggest`],
//! [`Reporter`], [`Style`], [`Gate`], [`net::Listener`], [`net::Observer`],
//! [`net::Spawn`].
//!
//! ```no_run
//! use banter::{App, Flow, Result, Session, ensure};
//!
//! struct Game {
//!     secret: u32,
//! }
//!
//! #[banter::commands]
//! impl Game {
//!     /// take a guess
//!     fn guess(&mut self, s: &mut Session, n: u32) -> Result<Flow> {
//!         ensure!(n < 100, "below 100, I said");
//!         if n == self.secret {
//!             s.ok("got it")?;
//!             return Ok(Flow::Close);
//!         }
//!         s.say(if n < self.secret { "higher" } else { "lower" })?;
//!         Ok(Flow::Continue)
//!     }
//! }
//!
//! let app = App::new(|_| Ok(Game { secret: 42 }))
//!     .prompt("guess> ")
//!     .greet(|s, _| s.say("I picked a number below 100."))
//!     .commands();
//!
//! banter::run(app);
//! ```
//!
//! Without the macro, the same command is
//! `app.command(Spec::new("guess").arg("n").help("take a guess"), |ctx| ...)`, with
//! arguments read through [`Ctx::arg`].
//!
//! # Features
//!
//! - `net` (default): [`net::Server`] over TCP or Unix sockets, and `--listen`.
//! - `macros` (default): [`commands`] and [`derive@FromArg`].
//! - `style`: [`output::Ansi`] colouring with [`output::Palette`]s.
//! - `pow`: [`pow::Pow`], a proof-of-work [`Gate`].
//! - `testing`: [`testing::Script`] and [`testing::Client`].

pub mod app;
pub mod cli;
pub mod command;
mod error;
pub mod gate;
pub mod input;
pub mod output;
pub mod session;

#[cfg(feature = "net")]
pub mod net;
#[cfg(feature = "pow")]
pub mod pow;
#[cfg(feature = "testing")]
pub mod testing;

pub use app::{App, Distance, Prompt, Suggest};
pub use command::{Builtin, Command, Commands, Ctx, Flow, Kind, Outcome, Param, Spec};
pub use error::{Error, Hint, Result};
pub use gate::Gate;
pub use input::{Args, FromArg, Hex, HexArray, Lexer, Shell, Words, hex};
pub use output::{Block, Cell, Layout, Plain, Report, Reporter, Role, Style, Table};
pub use session::{Conn, Info, Newline, Peer, Session, Settings, Store};

#[cfg(feature = "macros")]
pub use banter_macros::{FromArg, commands};

/// Serve `app` as the command line asks; see [`cli`].
pub fn run<S: Send + 'static>(app: App<S>) -> std::process::ExitCode {
    match cli::Options::args() {
        Ok(options) => options.run(app),
        Err(message) => {
            eprintln!("{message}");
            std::process::ExitCode::FAILURE
        }
    }
}
