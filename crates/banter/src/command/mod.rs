//! Commands: a [`Spec`] describing one, and the handler behind it.

mod builtin;
mod ctx;
mod spec;

pub use builtin::Builtin;
pub use ctx::Ctx;
pub use spec::{Kind, Param, Signature, Spec};

use crate::error::{Error, Result};

/// What the loop does after a command returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Prompt again.
    Continue,
    /// End the session.
    Close,
}

/// What a handler may return: a [`Flow`], or `()` to carry on.
pub trait Outcome {
    /// The flow this return value asks for.
    fn flow(self) -> Flow;
}

impl Outcome for Flow {
    fn flow(self) -> Flow {
        self
    }
}

impl Outcome for () {
    fn flow(self) -> Flow {
        Flow::Continue
    }
}

/// A state type that brings its own command table, usually derived with
/// `#[banter::commands]` on an `impl` block.
pub trait Commands: Sized {
    /// Every command, in listing order.
    fn commands() -> Vec<Command<Self>>;
}

type Handler<S> = Box<dyn Fn(&mut Ctx<'_, S>) -> Result<Flow> + Send + Sync>;

/// A [`Spec`] bound to its handler.
pub struct Command<S> {
    spec: Spec,
    handler: Handler<S>,
}

impl<S> Command<S> {
    /// Bind `handler` to `spec`.
    pub fn new<F, O>(spec: impl Into<Spec>, handler: F) -> Self
    where
        F: Fn(&mut Ctx<'_, S>) -> Result<O> + Send + Sync + 'static,
        O: Outcome,
    {
        Self {
            spec: spec.into(),
            handler: Box::new(move |ctx| handler(ctx).map(Outcome::flow)),
        }
    }

    /// What this command is called and takes.
    pub fn spec(&self) -> &Spec {
        &self.spec
    }

    /// Adjust the spec after the fact, e.g. to rename a built-in.
    pub fn respec(mut self, edit: impl FnOnce(Spec) -> Spec) -> Self {
        self.spec = edit(self.spec);
        self
    }

    /// The primary name.
    pub fn name(&self) -> &'static str {
        self.spec.name
    }

    /// Check arity, then run the handler.
    pub fn call(&self, ctx: &mut Ctx<'_, S>) -> Result<Flow> {
        if ctx.args.len() < self.spec.min() {
            return Err(Error::Usage(Box::new(self.spec.clone())));
        }
        (self.handler)(ctx)
    }
}
