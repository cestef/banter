//! What a command is called, what it takes, and how it is listed.

use std::{any::TypeId, fmt};

/// How many values a parameter takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Exactly one, always present.
    Required,
    /// Zero or one.
    Optional,
    /// Everything left, possibly nothing.
    Rest,
}

/// One named parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Param {
    /// Shown in help and errors.
    pub name: &'static str,
    /// How many values it takes.
    pub kind: Kind,
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            Kind::Required => write!(f, "<{}>", self.name),
            Kind::Optional => write!(f, "[{}]", self.name),
            Kind::Rest => write!(f, "[{}...]", self.name),
        }
    }
}

/// A command's name followed by its parameters, as shown in help and usage errors.
#[derive(Clone, Copy, Debug)]
pub struct Signature<'a>(&'a Spec);

impl fmt::Display for Signature<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.name)?;
        self.0.params.iter().try_for_each(|p| write!(f, " {p}"))
    }
}

/// Everything about a command except its behaviour. Read by dispatch, help, and
/// hooks that want to reason about the command table.
///
/// ```
/// use banter::Spec;
///
/// struct Paid;
///
/// let spec = Spec::new("sign")
///     .alias("s")
///     .arg("sid")
///     .arg("msg")
///     .opt("count")
///     .help("sign a message")
///     .tag::<Paid>();
/// assert_eq!(spec.signature().to_string(), "sign <sid> <msg> [count]");
/// assert_eq!(spec.min(), 2);
/// assert!(spec.tagged::<Paid>());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Spec {
    /// The primary name.
    pub name: &'static str,
    /// Other names it answers to.
    pub aliases: Vec<&'static str>,
    /// Parameters, in order.
    pub params: Vec<Param>,
    /// One line for the help listing.
    pub help: &'static str,
    /// Heading this command is listed under in help.
    pub group: Option<&'static str>,
    /// Marker types hooks can test for with [`Spec::tagged`].
    pub tags: Vec<TypeId>,
    /// Callable, but left out of help.
    pub hidden: bool,
}

impl Spec {
    /// A command called `name`, taking nothing.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            ..Self::default()
        }
    }

    /// Answer to `alias` too.
    pub fn alias(mut self, alias: &'static str) -> Self {
        self.aliases.push(alias);
        self
    }

    /// Append a parameter.
    pub fn param(mut self, name: &'static str, kind: Kind) -> Self {
        self.params.push(Param { name, kind });
        self
    }

    /// A required parameter.
    pub fn arg(self, name: &'static str) -> Self {
        self.param(name, Kind::Required)
    }

    /// An optional parameter.
    pub fn opt(self, name: &'static str) -> Self {
        self.param(name, Kind::Optional)
    }

    /// Whatever is left of the line.
    pub fn rest(self, name: &'static str) -> Self {
        self.param(name, Kind::Rest)
    }

    /// Set the help line.
    pub fn help(mut self, help: &'static str) -> Self {
        self.help = help;
        self
    }

    /// List under this heading in help.
    pub fn group(mut self, group: &'static str) -> Self {
        self.group = Some(group);
        self
    }

    /// Mark with any type; `before` and `after` hooks can then check for it.
    pub fn tag<T: 'static>(mut self) -> Self {
        self.tags.push(TypeId::of::<T>());
        self
    }

    /// Leave out of help.
    pub fn hidden(mut self) -> Self {
        self.hidden = true;
        self
    }

    /// Name and parameters, for display.
    pub fn signature(&self) -> Signature<'_> {
        Signature(self)
    }

    /// Arguments that must be present.
    pub fn min(&self) -> usize {
        self.params
            .iter()
            .filter(|p| p.kind == Kind::Required)
            .count()
    }

    /// The parameter an argument position falls under.
    pub fn param_at(&self, index: usize) -> Option<&Param> {
        self.params
            .get(index)
            .or_else(|| self.params.last().filter(|p| p.kind == Kind::Rest))
    }

    /// Whether [`Spec::tag`] was called with `T`.
    pub fn tagged<T: 'static>(&self) -> bool {
        self.tags.contains(&TypeId::of::<T>())
    }

    /// Whether `token` names this command, by name or alias.
    pub fn answers(&self, token: &str, fold: bool) -> bool {
        let same = |name: &&str| {
            if fold {
                name.eq_ignore_ascii_case(token)
            } else {
                *name == token
            }
        };
        same(&self.name) || self.aliases.iter().any(same)
    }
}

impl From<&'static str> for Spec {
    fn from(name: &'static str) -> Self {
        Self::new(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_parameters_absorb_trailing_positions() {
        let spec = Spec::new("say").arg("to").rest("words");
        assert_eq!(spec.signature().to_string(), "say <to> [words...]");
        assert_eq!(spec.param_at(5).map(|p| p.name), Some("words"));
        assert_eq!(Spec::new("x").arg("a").param_at(1), None);
    }

    #[test]
    fn names_and_aliases_answer_with_optional_folding() {
        let spec = Spec::new("Open").alias("o");
        assert!(spec.answers("Open", false));
        assert!(spec.answers("o", false));
        assert!(!spec.answers("open", false));
        assert!(spec.answers("OPEN", true));
    }
}
