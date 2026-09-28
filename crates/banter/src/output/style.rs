//! How output is decorated.
//!
//! Every piece of text a session writes carries a [`Role`]. A [`Style`] decides what
//! a role looks like on the wire. [`Plain`] leaves text alone, which is what scripted
//! clients want; with the `style` feature, [`Ansi`] colours it for humans. Implement
//! [`Style`] for anything else: markup, escaping, a teletype that only has capitals.

use std::borrow::Cow;

/// What a piece of output is, independent of how it looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    /// Ordinary prose.
    Text,
    /// The input prompt.
    Prompt,
    /// Titles and banners.
    Title,
    /// Left column of key-value output, command names in help.
    Key,
    /// Values handed to the peer: hex, numbers, identifiers.
    Value,
    /// Secondary text: notes, descriptions, hints.
    Dim,
    /// Something went right.
    Ok,
    /// Something went wrong but the session goes on.
    Warn,
    /// Something went wrong.
    Error,
    /// The one thing the peer came for.
    Accent,
    /// Application-defined; give each one a named constant.
    Custom(u16),
}

/// Decides what each [`Role`] looks like on the wire.
pub trait Style: Send + Sync {
    /// `text` dressed as `role`.
    fn paint<'a>(&self, role: Role, text: &'a str) -> Cow<'a, str>;
}

impl<F> Style for F
where
    F: Fn(Role, &str) -> String + Send + Sync,
{
    fn paint<'a>(&self, role: Role, text: &'a str) -> Cow<'a, str> {
        Cow::Owned(self(role, text))
    }
}

/// Text passes through unchanged.
#[derive(Clone, Copy, Debug, Default)]
pub struct Plain;

impl Style for Plain {
    fn paint<'a>(&self, _role: Role, text: &'a str) -> Cow<'a, str> {
        Cow::Borrowed(text)
    }
}

#[cfg(feature = "style")]
pub use ansi::{Ansi, Attr, Base, Color, Paint, Palette};

#[cfg(feature = "style")]
mod ansi;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closures_are_styles() {
        let upper = |_: Role, text: &str| text.to_uppercase();
        assert_eq!(upper.paint(Role::Text, "hi"), "HI");
    }

    #[cfg(feature = "style")]
    #[test]
    fn palettes_cover_custom_roles() {
        const FLAG: Role = Role::Custom(0);
        let ansi = Ansi(Palette::colour().set(FLAG, Paint::new().fg(Color::Magenta).bold()));
        assert_eq!(ansi.paint(Role::Ok, "yes"), "\x1b[32myes\x1b[0m");
        assert_eq!(ansi.paint(FLAG, "f"), "\x1b[1;35mf\x1b[0m");
        assert_eq!(ansi.paint(Role::Text, "as is"), "as is");
    }
}
