//! Everything about how output looks.

mod layout;
mod report;
mod style;

pub use layout::{Block, Cell, Layout, Table};
pub use report::{Report, Reporter};
#[cfg(feature = "style")]
pub use style::{Ansi, Attr, Base, Color, Paint, Palette};
pub use style::{Plain, Role, Style};
