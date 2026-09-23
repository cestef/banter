//! Tiny framework for line-based interactive services.
//!
//! Describe a service as per-session state plus a command table; banter owns the
//! prompt loop, argument parsing, help, typo suggestions, error reporting and the
//! transport. The same [`App`] serves stdio, TCP, or an in-memory test client.
//!
//! ```no_run
//! use banter::{App, Command, Flow, ensure};
//!
//! let app = App::new(|_| Ok(42u32))
//!     .prompt("guess> ")
//!     .greet(|s, _| s.say("I picked a number below 100."))
//!     .command(
//!         Command::new("guess", |s, secret: &mut u32, args| {
//!             let n: u32 = args.get(0)?;
//!             ensure!(n < 100, "below 100, I said");
//!             if n == *secret {
//!                 s.ok("got it")?;
//!                 return Ok(Flow::Close);
//!             }
//!             s.say(if n < *secret { "higher" } else { "lower" })?;
//!             Ok(Flow::Continue)
//!         })
//!         .usage("<n>")
//!         .help("take a guess"),
//!     );
//!
//! banter::run(app);
//! ```
//!
//! # Features
//!
//! - `tcp` (default): [`tcp::Server`], and `--listen` in [`run`].
//! - `style`: [`style::Ansi`] colouring.
//! - `pow`: [`pow::Pow`], a proof-of-work [`Gate`].
//! - `testing`: [`testing::transcript`] and [`testing::Client`].

mod app;
pub mod args;
mod command;
mod error;
pub mod gate;
pub mod layout;
mod session;
pub mod style;

#[cfg(feature = "pow")]
pub mod pow;
#[cfg(feature = "tcp")]
pub mod tcp;
#[cfg(feature = "testing")]
pub mod testing;

pub use app::App;
pub use args::{Args, FromArg, Hex, HexArray, hex};
pub use command::{Command, Flow};
pub use error::{Error, Result};
pub use gate::Gate;
pub use layout::{Block, Cell, Table};
pub use session::{Conn, ConnInfo, MAX_LINE, Session};
pub use style::{Plain, Role, Style};

use std::process::ExitCode;

/// Serve `app` as the command line asks: one session on stdio by default, or TCP
/// with `--listen ADDR` (`1337`, `:1337` and `host:port` all work).
///
/// Also understood: `--max-conns N`, `--idle SECS` (0 disables) and `-h`.
pub fn run<S: 'static>(app: App<S>) -> ExitCode {
    match Cli::parse(std::env::args().skip(1)).and_then(|cli| cli.serve(app)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Cli {
    listen: Option<String>,
    max_conns: Option<usize>,
    idle: Option<u64>,
    help: bool,
}

impl Cli {
    const USAGE: &str = "usage: [--listen ADDR] [--max-conns N] [--idle SECS]\n\n\
        Without --listen, serves one session on stdin/stdout.";

    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut cli = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let (key, inline) = match arg.split_once('=') {
                Some((k, v)) => (k.to_string(), Some(v.to_string())),
                None => (arg, None),
            };
            let mut value = |name: &str| {
                inline
                    .clone()
                    .or_else(|| args.next())
                    .ok_or(format!("{name} needs a value\n{}", Self::USAGE))
            };
            let number = |raw: String| {
                raw.parse::<u64>()
                    .map_err(|_| format!("'{raw}' is not a number"))
            };
            match key.as_str() {
                "-l" | "--listen" => cli.listen = Some(value("--listen")?),
                "--max-conns" => cli.max_conns = Some(number(value("--max-conns")?)? as usize),
                "--idle" => cli.idle = Some(number(value("--idle")?)?),
                "-h" | "--help" => cli.help = true,
                other => return Err(format!("unknown argument '{other}'\n{}", Self::USAGE)),
            }
        }
        Ok(cli)
    }

    fn serve<S: 'static>(self, app: App<S>) -> Result<(), String> {
        if self.help {
            println!("{}", Self::USAGE);
            return Ok(());
        }
        match self.listen {
            None => app.serve(Conn::stdio()).map_err(|e| e.to_string()),
            #[cfg(feature = "tcp")]
            Some(addr) => {
                let addr = match addr.strip_prefix(':') {
                    Some(port) => format!("0.0.0.0:{port}"),
                    None if addr.bytes().all(|b| b.is_ascii_digit()) => format!("0.0.0.0:{addr}"),
                    None => addr,
                };
                let mut server = tcp::Server::new(app);
                if let Some(n) = self.max_conns {
                    server = server.max_conns(n);
                }
                if let Some(secs) = self.idle {
                    server = server
                        .idle_timeout((secs > 0).then(|| std::time::Duration::from_secs(secs)));
                }
                server.bind(&addr).map_err(|e| format!("{addr}: {e}"))
            }
            #[cfg(not(feature = "tcp"))]
            Some(_) => Err("built without the `tcp` feature".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> {
        Cli::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn cli_accepts_both_value_spellings() {
        assert_eq!(
            parse(&["--listen", ":1337", "--idle=0"]).unwrap(),
            Cli {
                listen: Some(":1337".into()),
                idle: Some(0),
                ..Cli::default()
            }
        );
        assert!(parse(&["--listen"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
    }
}
