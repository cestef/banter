//! Gates: checks a connection must pass before its session starts.
//!
//! A gate talks to the peer through the same [`Session`] as everything else, so it
//! can challenge, rate-limit, authenticate or just print a notice. Returning an error
//! ends the session; [`Error::Denied`](crate::Error::Denied) is reported to the peer
//! and treated as a normal close.

use crate::{error::Result, session::Session};

/// A check run on each connection before its session starts.
pub trait Gate: Send + Sync {
    /// Let the session through, or return why not.
    fn admit(&self, session: &mut Session) -> Result<()>;
}

impl<F> Gate for F
where
    F: Fn(&mut Session) -> Result<()> + Send + Sync,
{
    fn admit(&self, session: &mut Session) -> Result<()> {
        self(session)
    }
}
