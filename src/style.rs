//! How output is decorated.
//!
//! Every piece of text a session writes carries a [`Role`]. A [`Style`] decides what
//! a role looks like on the wire. [`Plain`] leaves text alone, which is what scripted
//! clients want; with the `style` feature, [`Ansi`] colours it for humans. Implement
//! [`Style`] for anything else (markup, JSON-escaped, uppercase for a teletype).

use std::borrow::Cow;

/// What a piece of output is, independent of how it looks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Role {
    /// Ordinary prose.
    Text,
    /// The input prompt.
    Prompt,
    /// Titles and banners.
    Title,
    /// The left column of key-value output, command names in help.
    Key,
    /// Values handed to the peer: hex, numbers, identifiers.
    Value,
    /// Secondary text: notes, help descriptions.
    Dim,
    /// Something went right.
    Ok,
    /// Something went wrong.
    Error,
    /// The one thing the peer came for.
    Accent,
}

pub trait Style: Send + Sync {
    fn paint<'a>(&self, role: Role, text: &'a str) -> Cow<'a, str>;
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
pub use ansi::{Ansi, Palette};

#[cfg(feature = "style")]
mod ansi {
    use super::*;

    /// SGR parameters per role, e.g. `"1;36"`. Empty means unstyled.
    #[derive(Clone, Copy, Debug)]
    pub struct Palette {
        pub text: &'static str,
        pub prompt: &'static str,
        pub title: &'static str,
        pub key: &'static str,
        pub value: &'static str,
        pub dim: &'static str,
        pub ok: &'static str,
        pub error: &'static str,
        pub accent: &'static str,
    }

    impl Palette {
        pub const DEFAULT: Self = Self {
            text: "",
            prompt: "1;36",
            title: "1",
            key: "36",
            value: "33",
            dim: "2",
            ok: "32",
            error: "1;31",
            accent: "1;35",
        };

        /// Bold and dim only, for terminals where colour is unwelcome.
        pub const MONO: Self = Self {
            text: "",
            prompt: "1",
            title: "1",
            key: "1",
            value: "",
            dim: "2",
            ok: "1",
            error: "1",
            accent: "1;4",
        };

        fn get(&self, role: Role) -> &'static str {
            match role {
                Role::Text => self.text,
                Role::Prompt => self.prompt,
                Role::Title => self.title,
                Role::Key => self.key,
                Role::Value => self.value,
                Role::Dim => self.dim,
                Role::Ok => self.ok,
                Role::Error => self.error,
                Role::Accent => self.accent,
            }
        }
    }

    impl Default for Palette {
        fn default() -> Self {
            Self::DEFAULT
        }
    }

    /// ANSI SGR colouring.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Ansi(pub Palette);

    impl Style for Ansi {
        fn paint<'a>(&self, role: Role, text: &'a str) -> Cow<'a, str> {
            match self.0.get(role) {
                "" => Cow::Borrowed(text),
                sgr => Cow::Owned(format!("\x1b[{sgr}m{text}\x1b[0m")),
            }
        }
    }
}

#[cfg(all(test, feature = "style"))]
mod tests {
    use super::*;

    #[test]
    fn ansi_wraps_and_resets() {
        assert_eq!(Ansi::default().paint(Role::Ok, "yes"), "\x1b[32myes\x1b[0m");
        assert_eq!(Ansi::default().paint(Role::Text, "as is"), "as is");
    }
}
