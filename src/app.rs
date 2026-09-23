//! An application: per-session state, a command table, and the loop that runs it.

use std::sync::Arc;

use crate::{
    args::Args,
    command::{Command, Flow, suggest},
    error::{Error, Result},
    gate::Gate,
    layout::{Cell, Table},
    session::{Conn, ConnInfo, MAX_LINE, Session},
    style::{Plain, Role, Style},
};

type Init<S> = Box<dyn Fn(&ConnInfo) -> Result<S> + Send + Sync>;
type Greet<S> = Box<dyn Fn(&mut Session, &mut S) -> Result<()> + Send + Sync>;

/// Commands every app answers unless [`App::without_builtins`] is set.
const BUILTINS: [(&str, &str); 2] = [("help", "show this list"), ("exit", "close the session")];

/// Everything that defines a service, built once and shared by every session.
///
/// ```
/// use banter::{App, Command, Flow};
///
/// let app = App::new(|_| Ok(0u64))
///     .prompt("counter> ")
///     .command(Command::new("inc", |s, n: &mut u64, _| {
///         *n += 1;
///         s.kv("n", n.to_string())?;
///         Ok(Flow::Continue)
///     }).help("add one"));
/// ```
pub struct App<S> {
    init: Init<S>,
    greet: Option<Greet<S>>,
    commands: Vec<Command<S>>,
    gates: Vec<Box<dyn Gate>>,
    style: Arc<dyn Style>,
    prompt: String,
    max_line: usize,
    builtins: bool,
}

impl<S: 'static> App<S> {
    /// `init` builds one session's state; it runs after every gate has passed.
    pub fn new<F>(init: F) -> Self
    where
        F: Fn(&ConnInfo) -> Result<S> + Send + Sync + 'static,
    {
        Self {
            init: Box::new(init),
            greet: None,
            commands: Vec::new(),
            gates: Vec::new(),
            style: Arc::new(Plain),
            prompt: "> ".into(),
            max_line: MAX_LINE,
            builtins: true,
        }
    }

    /// Printed before every read, verbatim. Include the trailing space.
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// Runs once per session, after `init` and before the first prompt.
    pub fn greet<F>(mut self, greet: F) -> Self
    where
        F: Fn(&mut Session, &mut S) -> Result<()> + Send + Sync + 'static,
    {
        self.greet = Some(Box::new(greet));
        self
    }

    pub fn command(mut self, command: Command<S>) -> Self {
        self.commands.push(command);
        self
    }

    /// Shorthand for a command with help text and no arguments.
    pub fn cmd<F>(self, name: &'static str, help: &'static str, handler: F) -> Self
    where
        F: Fn(&mut Session, &mut S, &Args) -> Result<Flow> + Send + Sync + 'static,
    {
        self.command(Command::new(name, handler).help(help))
    }

    /// Runs before `init`, in the order added. Any error ends the session.
    pub fn gate(mut self, gate: impl Gate + 'static) -> Self {
        self.gates.push(Box::new(gate));
        self
    }

    pub fn style(mut self, style: impl Style + 'static) -> Self {
        self.style = Arc::new(style);
        self
    }

    pub fn max_line(mut self, max_line: usize) -> Self {
        self.max_line = max_line;
        self
    }

    /// Drop the built-in `help` and `exit`.
    pub fn without_builtins(mut self) -> Self {
        self.builtins = false;
        self
    }

    pub fn prompt_text(&self) -> &str {
        &self.prompt
    }

    /// The prompt exactly as it goes on the wire.
    pub fn painted_prompt(&self) -> String {
        self.style.paint(Role::Prompt, &self.prompt).into_owned()
    }

    pub fn commands(&self) -> &[Command<S>] {
        &self.commands
    }

    /// Serve one session to completion. A peer hanging up, timing out or failing a
    /// gate is a normal end and returns `Ok`.
    pub fn serve(&self, conn: Conn) -> Result<()> {
        let mut session = Session::new(conn)
            .with_style(Arc::clone(&self.style))
            .with_max_line(self.max_line);
        match self.run(&mut session) {
            Ok(_) | Err(Error::Eof) => Ok(()),
            Err(err) if err.benign() => {
                let _ = session.report(&err);
                Ok(())
            }
            Err(err) => {
                let _ = session.report(&Error::Fatal("internal error".into()));
                Err(err)
            }
        }
    }

    /// Gates, init, greeting, then the command loop.
    pub fn run(&self, session: &mut Session) -> Result<Flow> {
        for gate in &self.gates {
            gate.admit(session)?;
        }
        let mut state = (self.init)(&session.info())?;
        if let Some(greet) = &self.greet {
            greet(session, &mut state)?;
        }
        self.repl(session, &mut state)
    }

    /// Prompt, read, dispatch, until a command closes or the peer leaves.
    pub fn repl(&self, session: &mut Session, state: &mut S) -> Result<Flow> {
        let prompt = self.painted_prompt();
        loop {
            session.write(&prompt)?;
            let line = session.line()?;
            match self.dispatch(session, state, &line) {
                Ok(Flow::Continue) => {}
                Ok(flow) => return Ok(flow),
                Err(err) if err.recoverable() => session.report(&err)?,
                Err(err) => return Err(err),
            }
        }
    }

    /// Run one line. Blank lines and `#` comments are ignored so scripts can be
    /// annotated.
    pub fn dispatch(&self, session: &mut Session, state: &mut S, line: &str) -> Result<Flow> {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return Ok(Flow::Continue);
        }
        let (name, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));

        if let Some(command) = self.commands.iter().find(|c| c.matches(name)) {
            return command.invoke(session, state, &Args::parse(rest));
        }
        if self.builtins {
            match name {
                "help" | "?" => {
                    self.help(session)?;
                    return Ok(Flow::Continue);
                }
                "exit" | "quit" => return Ok(Flow::Close),
                _ => {}
            }
        }

        let builtins = self.builtins.then_some(BUILTINS.map(|(n, _)| n));
        let names = self
            .commands
            .iter()
            .filter(|c| !c.hidden)
            .map(|c| c.name)
            .chain(builtins.into_iter().flatten());
        Err(Error::UnknownCommand {
            got: name.to_string(),
            suggestion: suggest(name, names),
        })
    }

    /// One aligned row per visible command.
    pub fn help(&self, session: &mut Session) -> Result<()> {
        let mut table = Table::new();
        for command in self.commands.iter().filter(|c| !c.hidden) {
            table.push([
                Cell::new(Role::Key, command.signature()),
                Cell::new(Role::Dim, command.help),
            ]);
        }
        if self.builtins {
            for (name, help) in BUILTINS {
                table.push([Cell::new(Role::Key, name), Cell::new(Role::Dim, help)]);
            }
        }
        session.table(&table)
    }
}
