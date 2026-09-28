//! What the peer sees before each line it types.

use crate::{output::Role, session::Session};

type Render<S> = Box<dyn Fn(&Session, &S) -> String + Send + Sync>;

/// The prompt written before every read.
pub enum Prompt<S> {
    /// The same text every time.
    Text(String),
    /// Computed from the session and state before every read.
    Dynamic(Render<S>),
}

impl<S> Prompt<S> {
    /// Compute the prompt from the session and state.
    pub fn dynamic<F>(render: F) -> Self
    where
        F: Fn(&Session, &S) -> String + Send + Sync + 'static,
    {
        Self::Dynamic(Box::new(render))
    }

    /// Painted in [`Role::Prompt`], ready to write.
    pub fn render(&self, session: &Session, state: &S) -> String {
        match self {
            Self::Text(text) => session.paint(Role::Prompt, text),
            Self::Dynamic(render) => session.paint(Role::Prompt, &render(session, state)),
        }
    }

    /// The raw text, when it does not depend on state.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            Self::Dynamic(_) => None,
        }
    }
}

impl<S> From<&str> for Prompt<S> {
    fn from(text: &str) -> Self {
        Self::Text(text.into())
    }
}

impl<S> From<String> for Prompt<S> {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}
