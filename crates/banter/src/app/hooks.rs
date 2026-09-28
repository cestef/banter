//! Lifecycle hooks around sessions and commands.

use crate::{
    command::{Ctx, Flow},
    error::Result,
    session::Session,
};

pub(crate) type Init<S> = Box<dyn Fn(&mut Session) -> Result<S> + Send + Sync>;
type Greet<S> = Box<dyn Fn(&mut Session, &mut S) -> Result<()> + Send + Sync>;
type Before<S> = Box<dyn Fn(&mut Ctx<'_, S>) -> Result<()> + Send + Sync>;
type After<S> = Box<dyn Fn(&mut Ctx<'_, S>, &Result<Flow>) -> Result<()> + Send + Sync>;
type Fallback<S> = Box<dyn Fn(&mut Ctx<'_, S>) -> Result<Flow> + Send + Sync>;
type Close<S> = Box<dyn Fn(&mut Session, &mut S) -> Result<()> + Send + Sync>;

/// Every hook an [`App`](crate::App) runs, in registration order within each kind.
pub struct Hooks<S> {
    pub(crate) greet: Vec<Greet<S>>,
    pub(crate) before: Vec<Before<S>>,
    pub(crate) after: Vec<After<S>>,
    pub(crate) fallback: Option<Fallback<S>>,
    pub(crate) close: Vec<Close<S>>,
}

impl<S> Default for Hooks<S> {
    fn default() -> Self {
        Self {
            greet: Vec::new(),
            before: Vec::new(),
            after: Vec::new(),
            fallback: None,
            close: Vec::new(),
        }
    }
}

impl<S> Hooks<S> {
    /// Run every greeting in order, stopping at the first error.
    pub fn greet(&self, session: &mut Session, state: &mut S) -> Result<()> {
        self.greet.iter().try_for_each(|hook| hook(session, state))
    }

    /// Any error refuses the command before it runs.
    pub fn before(&self, ctx: &mut Ctx<'_, S>) -> Result<()> {
        self.before.iter().try_for_each(|hook| hook(ctx))
    }

    /// Run every after hook with the command's outcome.
    pub fn after(&self, ctx: &mut Ctx<'_, S>, outcome: &Result<Flow>) -> Result<()> {
        self.after.iter().try_for_each(|hook| hook(ctx, outcome))
    }

    /// `None` when no fallback is installed.
    pub fn fallback(&self, ctx: &mut Ctx<'_, S>) -> Option<Result<Flow>> {
        self.fallback.as_ref().map(|hook| hook(ctx))
    }

    /// Every close hook runs even if one fails; the first error is returned.
    pub fn close(&self, session: &mut Session, state: &mut S) -> Result<()> {
        let mut first = Ok(());
        for hook in &self.close {
            let outcome = hook(session, state);
            if first.is_ok() {
                first = outcome;
            }
        }
        first
    }
}
