//! Commands: a name, a signature, and a handler.

use crate::{
    args::Args,
    error::{Error, Result},
    session::Session,
};

/// What the loop does after a command returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Prompt again.
    Continue,
    /// End the session.
    Close,
}

type Handler<S> = Box<dyn Fn(&mut Session, &mut S, &Args) -> Result<Flow> + Send + Sync>;

/// One entry in an [`App`](crate::App)'s command table.
pub struct Command<S> {
    pub(crate) name: &'static str,
    pub(crate) aliases: Vec<&'static str>,
    pub(crate) usage: &'static str,
    pub(crate) help: &'static str,
    arity: Option<usize>,
    pub(crate) hidden: bool,
    handler: Handler<S>,
}

impl<S> Command<S> {
    pub fn new<F>(name: &'static str, handler: F) -> Self
    where
        F: Fn(&mut Session, &mut S, &Args) -> Result<Flow> + Send + Sync + 'static,
    {
        Self {
            name,
            aliases: Vec::new(),
            usage: "",
            help: "",
            arity: None,
            hidden: false,
            handler: Box::new(handler),
        }
    }

    pub fn alias(mut self, alias: &'static str) -> Self {
        self.aliases.push(alias);
        self
    }

    /// Argument signature, e.g. `"<id> <msg-hex> [count]"`. Each `<required>` slot
    /// counts toward the arity checked before the handler runs.
    pub fn usage(mut self, usage: &'static str) -> Self {
        self.usage = usage;
        self
    }

    pub fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }

    /// Override the minimum argument count derived from [`Self::usage`].
    pub fn arity(mut self, arity: usize) -> Self {
        self.arity = Some(arity);
        self
    }

    /// Callable, but left out of `help`.
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// `name <args>`, as shown in help and usage errors.
    pub fn signature(&self) -> String {
        if self.usage.is_empty() {
            self.name.to_string()
        } else {
            format!("{} {}", self.name, self.usage)
        }
    }

    fn min_args(&self) -> usize {
        self.arity
            .unwrap_or_else(|| self.usage.matches('<').count())
    }

    pub(crate) fn matches(&self, token: &str) -> bool {
        self.name == token || self.aliases.contains(&token)
    }

    pub(crate) fn invoke(&self, session: &mut Session, state: &mut S, args: &Args) -> Result<Flow> {
        if args.len() < self.min_args() {
            return Err(Error::Usage(self.signature()));
        }
        (self.handler)(session, state, args)
    }
}

/// The candidate closest to `token`, if any is close enough to be a typo.
pub(crate) fn suggest<'a>(
    token: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let limit = (token.chars().count() / 3).max(1);
    candidates
        .into_iter()
        .map(|c| (distance(token, c), c))
        .filter(|&(d, _)| d <= limit)
        .min_by_key(|&(d, _)| d)
        .map(|(_, c)| c)
}

fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let next = (prev + usize::from(ca != cb))
                .min(row[j] + 1)
                .min(row[j + 1] + 1);
            prev = row[j + 1];
            row[j + 1] = next;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arity_comes_from_the_usage_string() {
        let cmd =
            Command::<()>::new("sign", |_, _, _| Ok(Flow::Continue)).usage("<sid> <msg> [extra]");
        assert_eq!(cmd.min_args(), 2);
        assert_eq!(cmd.arity(5).min_args(), 5);
    }

    #[test]
    fn suggests_near_misses_only() {
        let names = ["open", "sign", "submit", "help"];
        assert_eq!(suggest("sigm", names), Some("sign"));
        assert_eq!(suggest("submti", names), Some("submit"));
        assert_eq!(suggest("xyzzy", names), None);
    }
}
