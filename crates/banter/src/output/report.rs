//! How errors reach the peer.

use std::borrow::Cow;

use super::Role;
use crate::{
    error::{Error, Result},
    session::Session,
};

/// Renders an error to the peer. The app decides afterwards whether the session
/// continues, so a reporter only ever writes.
pub trait Reporter: Send + Sync {
    /// Write `error` to the peer.
    fn report(&self, session: &mut Session, error: &Error) -> Result<()>;
}

impl<F> Reporter for F
where
    F: Fn(&mut Session, &Error) -> Result<()> + Send + Sync,
{
    fn report(&self, session: &mut Session, error: &Error) -> Result<()> {
        self(session, error)
    }
}

/// The default reporter: a labelled line in [`Role::Error`], then the error's
/// [`Hint`](crate::Hint), if any, indented in [`Role::Dim`]. Failures that are not
/// the peer's are replaced by a generic line; the real error stays server-side.
#[derive(Clone, Debug)]
pub struct Report {
    label: Cow<'static, str>,
    fatal: Cow<'static, str>,
    hints: bool,
}

impl Default for Report {
    fn default() -> Self {
        Self {
            label: "error: ".into(),
            fatal: "internal error".into(),
            hints: true,
        }
    }
}

impl Report {
    /// The default reporter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Prefix of every error line.
    pub fn label(mut self, label: impl Into<Cow<'static, str>>) -> Self {
        self.label = label.into();
        self
    }

    /// What the peer sees for failures that are not theirs.
    pub fn fatal(mut self, message: impl Into<Cow<'static, str>>) -> Self {
        self.fatal = message.into();
        self
    }

    /// Show or hide hint lines.
    pub fn hints(mut self, hints: bool) -> Self {
        self.hints = hints;
        self
    }
}

impl Reporter for Report {
    fn report(&self, session: &mut Session, error: &Error) -> Result<()> {
        let message = if error.recoverable() || error.benign() {
            error.to_string()
        } else {
            self.fatal.to_string()
        };
        let indent = " ".repeat(session.settings().layout.indent);
        let mut lines = vec![session.paint(Role::Error, &[&*self.label, &message].concat())];
        if let Some(hint) = error.hint().filter(|_| self.hints) {
            lines.push(session.paint(Role::Dim, &[indent, hint.to_string()].concat()));
        }
        session.lines(&lines)
    }
}
