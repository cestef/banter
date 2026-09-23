//! Serving an app the way its command line and environment ask.
//!
//! ```text
//! --listen ADDR    serve instead of one stdio session: 1337, :1337, host:port, unix:PATH
//! --limit N        concurrent sessions
//! --idle SECS      drop silent peers, 0 disables
//! ```
//!
//! Each flag also reads from `{PREFIX}_LISTEN`, `{PREFIX}_LIMIT` and `{PREFIX}_IDLE`
//! when an env prefix is set; flags win.

use std::{path::PathBuf, process::ExitCode, time::Duration};

use crate::{app::App, session::Conn};

/// Where to serve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Addr {
    Tcp(String),
    Unix(PathBuf),
}

impl Addr {
    /// `1337` and `:1337` mean every interface; `unix:` selects a socket path.
    pub fn parse(raw: &str) -> Self {
        if let Some(path) = raw.strip_prefix("unix:") {
            return Self::Unix(path.into());
        }
        let port = raw.strip_prefix(':').unwrap_or(raw);
        if port.bytes().all(|b| b.is_ascii_digit()) {
            Self::Tcp(format!("0.0.0.0:{port}"))
        } else {
            Self::Tcp(raw.to_string())
        }
    }
}

/// A setting that can come from a flag or the environment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key {
    Listen,
    Limit,
    Idle,
}

impl Key {
    const ALL: [Self; 3] = [Self::Listen, Self::Limit, Self::Idle];

    fn flag(self) -> &'static str {
        match self {
            Self::Listen => "--listen",
            Self::Limit => "--limit",
            Self::Idle => "--idle",
        }
    }

    fn var(self, prefix: &str) -> String {
        format!(
            "{prefix}_{}",
            self.flag().trim_start_matches('-').to_uppercase()
        )
    }

    fn from_flag(flag: &str) -> Option<Self> {
        match flag {
            "-l" => Some(Self::Listen),
            _ => Self::ALL.into_iter().find(|k| k.flag() == flag),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub listen: Option<Addr>,
    pub limit: Option<usize>,
    pub idle: Option<Option<Duration>>,
    pub help: bool,
}

impl Options {
    pub const USAGE: &str = "usage: [--listen ADDR] [--limit N] [--idle SECS]\n\n\
        Without --listen, serves one session on stdin/stdout.\n\
        ADDR is 1337, :1337, host:port or unix:PATH.";

    /// Parse flags. `--flag value` and `--flag=value` both work.
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let (key, inline) = match arg.split_once('=') {
                Some((k, v)) => (k.to_string(), Some(v.to_string())),
                None => (arg, None),
            };
            let mut value = || {
                inline
                    .clone()
                    .or_else(|| args.next())
                    .ok_or(format!("{key} needs a value\n{}", Self::USAGE))
            };
            match (key.as_str(), Key::from_flag(&key)) {
                ("-h" | "--help", _) => options.help = true,
                (_, Some(k)) => options.set(k, &value()?)?,
                (other, None) => {
                    return Err(format!("unknown argument '{other}'\n{}", Self::USAGE));
                }
            }
        }
        Ok(options)
    }

    /// Fill anything not yet set from `{prefix}_LISTEN`, `_LIMIT` and `_IDLE`.
    pub fn env(mut self, prefix: &str) -> Result<Self, String> {
        for key in Key::ALL {
            let var = key.var(prefix);
            if let (false, Ok(value)) = (self.has(key), std::env::var(&var)) {
                self.set(key, &value).map_err(|e| format!("{var}: {e}"))?;
            }
        }
        Ok(self)
    }

    /// Flags from this process's command line.
    pub fn args() -> Result<Self, String> {
        Self::parse(std::env::args().skip(1))
    }

    fn has(&self, key: Key) -> bool {
        match key {
            Key::Listen => self.listen.is_some(),
            Key::Limit => self.limit.is_some(),
            Key::Idle => self.idle.is_some(),
        }
    }

    fn set(&mut self, key: Key, value: &str) -> Result<(), String> {
        let number = || {
            value
                .parse::<u64>()
                .map_err(|_| format!("'{value}' is not a number"))
        };
        match key {
            Key::Listen => self.listen = Some(Addr::parse(value)),
            Key::Limit => self.limit = Some(number()? as usize),
            Key::Idle => {
                self.idle = Some(Some(number()?).filter(|&s| s > 0).map(Duration::from_secs));
            }
        }
        Ok(())
    }

    /// Serve `app` accordingly, printing any startup error to stderr.
    pub fn run<S: Send + 'static>(self, app: App<S>) -> ExitCode {
        match self.serve(app) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::FAILURE
            }
        }
    }

    fn serve<S: Send + 'static>(self, app: App<S>) -> Result<(), String> {
        if self.help {
            println!("{}", Self::USAGE);
            return Ok(());
        }
        match self.listen.clone() {
            None => app.serve(Conn::stdio()).map_err(|e| e.to_string()),
            #[cfg(feature = "net")]
            Some(addr) => self.bind(app, addr),
            #[cfg(not(feature = "net"))]
            Some(_) => Err("built without the `net` feature".into()),
        }
    }

    #[cfg(feature = "net")]
    fn bind<S: Send + 'static>(&self, app: App<S>, addr: Addr) -> Result<(), String> {
        use crate::net::{Server, Tcp};

        let idle = self.idle.unwrap_or(Some(Tcp::IDLE));
        let limit = self.limit.unwrap_or(Server::<S, Tcp>::LIMIT);
        let failed = |e: std::io::Error| format!("{addr:?}: {e}");
        match &addr {
            Addr::Tcp(host) => {
                let listener = Tcp::bind(host).map_err(failed)?.idle(idle);
                Server::new(app, listener)
                    .limit(limit)
                    .run()
                    .map_err(failed)
            }
            #[cfg(unix)]
            Addr::Unix(path) => {
                let listener = crate::net::Unix::bind(path).map_err(failed)?.idle(idle);
                Server::new(app, listener)
                    .limit(limit)
                    .run()
                    .map_err(failed)
            }
            #[cfg(not(unix))]
            Addr::Unix(_) => Err("unix sockets need a unix".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn both_value_spellings_work() {
        assert_eq!(
            parse(&["--listen", ":1337", "--idle=0", "--limit", "8"]).unwrap(),
            Options {
                listen: Some(Addr::Tcp("0.0.0.0:1337".into())),
                limit: Some(8),
                idle: Some(None),
                help: false,
            }
        );
        assert!(parse(&["--listen"]).is_err());
        assert!(parse(&["--frobnicate"]).is_err());
    }

    #[test]
    fn env_fills_what_flags_left_unset() {
        // SAFETY: tests touching this variable are confined to this one.
        unsafe { std::env::set_var("BANTER_TEST_LIMIT", "3") };
        unsafe { std::env::set_var("BANTER_TEST_IDLE", "9") };
        let options = parse(&["--idle", "1"]).unwrap().env("BANTER_TEST").unwrap();
        assert_eq!(options.limit, Some(3));
        assert_eq!(options.idle, Some(Some(Duration::from_secs(1))));
    }

    #[test]
    fn addresses_parse() {
        assert_eq!(Addr::parse("1337"), Addr::Tcp("0.0.0.0:1337".into()));
        assert_eq!(Addr::parse("[::1]:80"), Addr::Tcp("[::1]:80".into()));
        assert_eq!(Addr::parse("unix:/tmp/s"), Addr::Unix("/tmp/s".into()));
    }
}
