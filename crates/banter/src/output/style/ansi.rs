//! ANSI terminal colouring.

use std::{borrow::Cow, fmt};

use super::{Role, Style};

/// The eight standard terminal colours, plus their bright variants and 256-colour
/// and truecolour escapes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Bright(Base),
    Indexed(u8),
    Rgb(u8, u8, u8),
}

/// A standard colour, for [`Color::Bright`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

impl Base {
    fn offset(self) -> u8 {
        self as u8
    }
}

impl Color {
    fn offset(self) -> Option<u8> {
        Some(match self {
            Self::Black => 0,
            Self::Red => 1,
            Self::Green => 2,
            Self::Yellow => 3,
            Self::Blue => 4,
            Self::Magenta => 5,
            Self::Cyan => 6,
            Self::White => 7,
            _ => return None,
        })
    }

    /// SGR parameters for this colour as foreground (`base` 30) or background (40).
    fn write(self, f: &mut impl fmt::Write, base: u8) -> fmt::Result {
        match self {
            Self::Bright(c) => write!(f, "{}", base + 60 + c.offset()),
            Self::Indexed(n) => write!(f, "{};5;{n}", base + 8),
            Self::Rgb(r, g, b) => write!(f, "{};2;{r};{g};{b}", base + 8),
            plain => write!(f, "{}", base + plain.offset().unwrap_or(0)),
        }
    }
}

/// Text attributes, applied in declaration order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attr {
    Bold = 1,
    Dim = 2,
    Italic = 3,
    Underline = 4,
    Blink = 5,
    Reverse = 7,
    Strike = 9,
}

/// How one role looks: attributes and optional colours.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Paint {
    attrs: Vec<Attr>,
    fg: Option<Color>,
    bg: Option<Color>,
}

impl Paint {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fg(mut self, color: Color) -> Self {
        self.fg = Some(color);
        self
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    pub fn attr(mut self, attr: Attr) -> Self {
        if !self.attrs.contains(&attr) {
            self.attrs.push(attr);
        }
        self
    }

    pub fn bold(self) -> Self {
        self.attr(Attr::Bold)
    }

    pub fn dim(self) -> Self {
        self.attr(Attr::Dim)
    }

    pub fn italic(self) -> Self {
        self.attr(Attr::Italic)
    }

    pub fn underline(self) -> Self {
        self.attr(Attr::Underline)
    }

    pub fn is_empty(&self) -> bool {
        self.attrs.is_empty() && self.fg.is_none() && self.bg.is_none()
    }

    /// `text` wrapped in this paint's escape and a reset.
    pub fn apply(&self, text: &str) -> String {
        format!("\x1b[{self}m{text}\x1b[0m")
    }
}

/// The SGR parameter list, e.g. `1;36`.
impl fmt::Display for Paint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut first = true;
        let mut sep = |f: &mut fmt::Formatter<'_>| {
            let out = if first { Ok(()) } else { f.write_str(";") };
            first = false;
            out
        };
        for attr in &self.attrs {
            sep(f)?;
            write!(f, "{}", *attr as u8)?;
        }
        if let Some(fg) = self.fg {
            sep(f)?;
            fg.write(f, 30)?;
        }
        if let Some(bg) = self.bg {
            sep(f)?;
            bg.write(f, 40)?;
        }
        Ok(())
    }
}

impl From<Color> for Paint {
    fn from(color: Color) -> Self {
        Self::new().fg(color)
    }
}

impl From<Attr> for Paint {
    fn from(attr: Attr) -> Self {
        Self::new().attr(attr)
    }
}

/// A [`Paint`] per role. Roles without one are left alone.
#[derive(Clone, Debug, Default)]
pub struct Palette {
    entries: Vec<(Role, Paint)>,
}

impl Palette {
    /// Nothing styled; build up with [`Self::set`].
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn colour() -> Self {
        Self::empty()
            .set(Role::Prompt, Paint::new().bold().fg(Color::Cyan))
            .set(Role::Title, Attr::Bold)
            .set(Role::Key, Color::Cyan)
            .set(Role::Value, Color::Yellow)
            .set(Role::Dim, Attr::Dim)
            .set(Role::Ok, Color::Green)
            .set(Role::Warn, Color::Yellow)
            .set(Role::Error, Paint::new().bold().fg(Color::Red))
            .set(Role::Accent, Paint::new().bold().fg(Color::Magenta))
    }

    /// Attributes only, no colour.
    pub fn mono() -> Self {
        Self::empty()
            .set(Role::Prompt, Attr::Bold)
            .set(Role::Title, Attr::Bold)
            .set(Role::Key, Attr::Bold)
            .set(Role::Dim, Attr::Dim)
            .set(Role::Ok, Attr::Bold)
            .set(Role::Warn, Attr::Bold)
            .set(Role::Error, Attr::Bold)
            .set(Role::Accent, Paint::new().bold().underline())
    }

    /// Set or replace how one role looks.
    pub fn set(mut self, role: Role, paint: impl Into<Paint>) -> Self {
        let paint = paint.into();
        match self.entries.iter_mut().find(|(r, _)| *r == role) {
            Some(entry) => entry.1 = paint,
            None => self.entries.push((role, paint)),
        }
        self
    }

    pub fn get(&self, role: Role) -> Option<&Paint> {
        self.entries
            .iter()
            .find(|(r, p)| *r == role && !p.is_empty())
            .map(|(_, p)| p)
    }
}

/// ANSI SGR colouring through a [`Palette`].
#[derive(Clone, Debug)]
pub struct Ansi(pub Palette);

impl Default for Ansi {
    fn default() -> Self {
        Self(Palette::colour())
    }
}

impl Style for Ansi {
    fn paint<'a>(&self, role: Role, text: &'a str) -> Cow<'a, str> {
        match self.0.get(role) {
            None => Cow::Borrowed(text),
            Some(paint) => Cow::Owned(paint.apply(text)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paints_render_sgr_parameters() {
        let paint = Paint::new()
            .bold()
            .fg(Color::Bright(Base::Red))
            .bg(Color::Indexed(8));
        assert_eq!(paint.to_string(), "1;91;48;5;8");
        assert_eq!(Paint::from(Color::Rgb(1, 2, 3)).to_string(), "38;2;1;2;3");
    }
}
