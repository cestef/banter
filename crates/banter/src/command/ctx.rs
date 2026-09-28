//! What a handler or hook sees.

use std::ops::{Deref, DerefMut};

use super::Command;
use crate::{
    app::App,
    error::{Error, Result},
    input::{Args, FromArg},
    session::Session,
};

/// One invocation: the session, the state, the parsed line, and the app it runs in.
///
/// Derefs to [`Session`], so `ctx.say(..)` and friends work directly.
pub struct Ctx<'a, S> {
    /// The session this line came from.
    pub session: &'a mut Session,
    /// The session's state.
    pub state: &'a mut S,
    /// The command name exactly as typed.
    pub name: &'a str,
    /// Everything after the command name.
    pub args: &'a Args,
    /// The matched command; `None` inside a fallback.
    pub command: Option<&'a Command<S>>,
    /// The app being served.
    pub app: &'a App<S>,
}

impl<S> Ctx<'_, S> {
    /// Required argument at `index`. Errors name the declared parameter.
    pub fn arg<T: FromArg>(&self, index: usize) -> Result<T> {
        self.name_param(self.args.get(index))
    }

    /// Optional argument at `index`.
    pub fn opt<T: FromArg>(&self, index: usize) -> Result<Option<T>> {
        self.name_param(self.args.opt(index))
    }

    /// Optional argument at `index`, or `fallback`.
    pub fn or<T: FromArg>(&self, index: usize, fallback: T) -> Result<T> {
        Ok(self.opt(index)?.unwrap_or(fallback))
    }

    /// Every argument from `from` on.
    pub fn all<T: FromArg>(&self, from: usize) -> Result<Vec<T>> {
        self.name_param(self.args.all(from))
    }

    /// Whether the matched command is tagged with `T`.
    pub fn tagged<T: 'static>(&self) -> bool {
        self.command.is_some_and(|c| c.spec().tagged::<T>())
    }

    fn name_param<T>(&self, result: Result<T>) -> Result<T> {
        result.map_err(|err| match err {
            Error::BadArg {
                index,
                param: None,
                kind,
                reason,
            } => Error::BadArg {
                index,
                param: self
                    .command
                    .and_then(|c| c.spec().param_at(index))
                    .map(|p| p.name),
                kind,
                reason,
            },
            other => other,
        })
    }
}

impl<S> Deref for Ctx<'_, S> {
    type Target = Session;

    fn deref(&self) -> &Session {
        self.session
    }
}

impl<S> DerefMut for Ctx<'_, S> {
    fn deref_mut(&mut self) -> &mut Session {
        self.session
    }
}
