//! An application: how sessions start, what they can do, and how they look.

mod hooks;
mod prompt;
mod registry;
mod suggest;

pub use hooks::Hooks;
pub use prompt::Prompt;
pub use registry::{Registry, Section};
pub use suggest::{Distance, Suggest};

use std::sync::Arc;

use hooks::Init;

use crate::{
    command::{Command, Commands, Ctx, Flow, Outcome, Spec},
    error::{Error, Result},
    gate::Gate,
    input::{Args, Lexer, Words},
    output::{Layout, Report, Reporter, Style},
    session::{Conn, Newline, Session, Settings},
};

/// Everything that defines a service, built once and shared by every session.
///
/// ```
/// use banter::{App, Flow, Spec};
///
/// let app = App::new(|_| Ok(0u64))
///     .prompt("counter> ")
///     .command(Spec::new("inc").help("add one"), |ctx| {
///         *ctx.state += 1;
///         let n = ctx.state.to_string();
///         ctx.kv("n", n)?;
///         Ok(Flow::Continue)
///     });
/// ```
pub struct App<S> {
    init: Init<S>,
    registry: Registry<S>,
    hooks: Hooks<S>,
    gates: Vec<Box<dyn Gate>>,
    prompt: Prompt<S>,
    lexer: Box<dyn Lexer>,
    suggest: Option<Box<dyn Suggest>>,
    reporter: Box<dyn Reporter>,
    settings: Settings,
}

impl<S: 'static> App<S> {
    /// `init` builds one session's state, after every gate has passed. It sees the
    /// session, so it can read what gates put in the [`Store`](crate::Store).
    pub fn new<F>(init: F) -> Self
    where
        F: Fn(&mut Session) -> Result<S> + Send + Sync + 'static,
    {
        Self {
            init: Box::new(init),
            registry: Registry::default(),
            hooks: Hooks::default(),
            gates: Vec::new(),
            prompt: "> ".into(),
            lexer: Box::new(Words::default()),
            suggest: Some(Box::new(Distance::default())),
            reporter: Box::new(Report::default()),
            settings: Settings::default(),
        }
    }

    // -- commands -----------------------------------------------------------

    /// Add a command, replacing any earlier one with the same name. The handler
    /// sees this app's state type, so closures need no annotations.
    pub fn command<F, O>(self, spec: impl Into<Spec>, handler: F) -> Self
    where
        F: Fn(&mut Ctx<'_, S>) -> Result<O> + Send + Sync + 'static,
        O: Outcome,
    {
        self.mount(Command::new(spec, handler))
    }

    /// Add every command the state type declares.
    pub fn commands(self) -> Self
    where
        S: Commands,
    {
        S::commands().into_iter().fold(self, Self::mount)
    }

    /// Add a prebuilt command, replacing any earlier one with the same name.
    pub fn mount(mut self, command: Command<S>) -> Self {
        self.registry.add(command);
        self
    }

    /// Add or replace a built-in, listed after the application's commands.
    pub fn builtin(mut self, command: Command<S>) -> Self {
        self.registry.builtin(command);
        self
    }

    /// Drop the default `help` and `exit`.
    pub fn bare(mut self) -> Self {
        self.registry.clear_builtins();
        self
    }

    /// Drop any command or built-in by name.
    pub fn remove(mut self, name: &str) -> Self {
        self.registry.remove(name);
        self
    }

    /// Match command names case-insensitively.
    pub fn fold(mut self) -> Self {
        self.registry.fold(true);
        self
    }

    // -- lifecycle ----------------------------------------------------------

    /// Runs before `init`, in order. Any error ends the session.
    pub fn gate(mut self, gate: impl Gate + 'static) -> Self {
        self.gates.push(Box::new(gate));
        self
    }

    /// Runs after `init`, before the first prompt.
    pub fn greet<F>(mut self, hook: F) -> Self
    where
        F: Fn(&mut Session, &mut S) -> Result<()> + Send + Sync + 'static,
    {
        self.hooks.greet.push(Box::new(hook));
        self
    }

    /// Runs before every matched command; an error refuses it.
    pub fn before<F>(mut self, hook: F) -> Self
    where
        F: Fn(&mut Ctx<'_, S>) -> Result<()> + Send + Sync + 'static,
    {
        self.hooks.before.push(Box::new(hook));
        self
    }

    /// Runs after every matched command, with its outcome.
    pub fn after<F>(mut self, hook: F) -> Self
    where
        F: Fn(&mut Ctx<'_, S>, &Result<Flow>) -> Result<()> + Send + Sync + 'static,
    {
        self.hooks.after.push(Box::new(hook));
        self
    }

    /// Handles lines naming no command, instead of the unknown-command error.
    pub fn fallback<F>(mut self, hook: F) -> Self
    where
        F: Fn(&mut Ctx<'_, S>) -> Result<Flow> + Send + Sync + 'static,
    {
        self.hooks.fallback = Some(Box::new(hook));
        self
    }

    /// Runs when the loop ends for any reason after `init` succeeded.
    pub fn close<F>(mut self, hook: F) -> Self
    where
        F: Fn(&mut Session, &mut S) -> Result<()> + Send + Sync + 'static,
    {
        self.hooks.close.push(Box::new(hook));
        self
    }

    // -- parsing and presentation --------------------------------------------

    /// Static text, or [`Prompt::dynamic`]. Include any trailing space.
    pub fn prompt(mut self, prompt: impl Into<Prompt<S>>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// How lines split into tokens. Defaults to [`Words`].
    pub fn lexer(mut self, lexer: impl Lexer + 'static) -> Self {
        self.lexer = Box::new(lexer);
        self
    }

    /// `None` turns typo suggestions off.
    pub fn suggest(mut self, suggest: Option<impl Suggest + 'static>) -> Self {
        self.suggest = suggest.map(|s| Box::new(s) as Box<dyn Suggest>);
        self
    }

    /// How errors reach the peer. Defaults to [`Report`].
    pub fn reporter(mut self, reporter: impl Reporter + 'static) -> Self {
        self.reporter = Box::new(reporter);
        self
    }

    /// How output is painted. Defaults to [`Plain`](crate::Plain).
    pub fn style(mut self, style: impl Style + 'static) -> Self {
        self.settings.style = Arc::new(style);
        self
    }

    /// How blocks and tables are laid out.
    pub fn layout(mut self, layout: Layout) -> Self {
        self.settings.layout = layout;
        self
    }

    /// The line ending written after each line.
    pub fn newline(mut self, newline: Newline) -> Self {
        self.settings.newline = newline;
        self
    }

    /// The longest line accepted from the peer, in bytes.
    pub fn max_line(mut self, max_line: usize) -> Self {
        self.settings.max_line = max_line;
        self
    }

    /// Replace every session setting at once.
    pub fn configure(mut self, settings: Settings) -> Self {
        self.settings = settings;
        self
    }
}

impl<S> App<S> {
    /// The command table.
    pub fn registry(&self) -> &Registry<S> {
        &self.registry
    }

    /// The settings every session starts with.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    /// The prompt exactly as it goes on the wire, when it is static.
    pub fn wire_prompt(&self) -> Option<String> {
        let text = self.prompt.text()?;
        Some(
            self.settings
                .style
                .paint(crate::Role::Prompt, text)
                .into_owned(),
        )
    }

    // -- running ------------------------------------------------------------

    /// Serve one connection to completion. The peer leaving, stalling or failing a
    /// gate is a normal end and returns `Ok`; anything else is reported to the
    /// peer and returned.
    pub fn serve(&self, conn: Conn) -> Result<()> {
        let mut session = Session::new(conn, self.settings.clone());
        match self.run(&mut session) {
            Ok(_) | Err(Error::Eof) => Ok(()),
            Err(err) => {
                let _ = self.reporter.report(&mut session, &err);
                if err.benign() { Ok(()) } else { Err(err) }
            }
        }
    }

    /// Gates, init, greeting, loop, close hooks.
    pub fn run(&self, session: &mut Session) -> Result<Flow> {
        for gate in &self.gates {
            gate.admit(session)?;
        }
        let mut state = (self.init)(session)?;
        let flow = self
            .hooks
            .greet(session, &mut state)
            .and_then(|()| self.repl(session, &mut state));
        let closed = self.hooks.close(session, &mut state);
        let flow = flow?;
        closed.map(|()| flow)
    }

    /// Prompt, read, handle, until a command closes or the peer leaves. Errors the
    /// peer can recover from are reported in place.
    pub fn repl(&self, session: &mut Session, state: &mut S) -> Result<Flow> {
        loop {
            let prompt = self.prompt.render(session, state);
            session.write(&prompt)?;
            let line = session.line()?;
            match self.handle(session, state, &line) {
                Ok(Flow::Continue) => {}
                Ok(flow) => return Ok(flow),
                Err(err) if err.recoverable() => self.reporter.report(session, &err)?,
                Err(err) => return Err(err),
            }
        }
    }

    /// Lex one line and run it: hooks and the matched command, or the fallback.
    pub fn handle(&self, session: &mut Session, state: &mut S, line: &str) -> Result<Flow> {
        let Some(mut tokens) = self.lexer.split(line)? else {
            return Ok(Flow::Continue);
        };
        if tokens.is_empty() {
            return Ok(Flow::Continue);
        }
        let name = tokens.remove(0);
        let args = Args::new(tokens);
        let command = self.registry.find(&name);
        let mut ctx = Ctx {
            session,
            state,
            name: &name,
            args: &args,
            command,
            app: self,
        };

        let Some(command) = command else {
            return self
                .hooks
                .fallback(&mut ctx)
                .unwrap_or_else(|| Err(self.unknown(&name)));
        };
        self.hooks.before(&mut ctx)?;
        let outcome = command.call(&mut ctx);
        self.hooks.after(&mut ctx, &outcome)?;
        outcome
    }

    fn unknown(&self, name: &str) -> Error {
        let suggestion = self
            .suggest
            .as_ref()
            .and_then(|s| s.suggest(name, &self.registry.names()));
        Error::UnknownCommand {
            got: name.to_string(),
            suggestion,
        }
    }
}
