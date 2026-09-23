//! Aligned output: key-value blocks and tables.
//!
//! Widths are measured on the unstyled text and padding happens before painting, so
//! columns line up whatever the [`Style`] does.

use super::{Role, Style};

/// Spacing defaults a session applies to tables and blocks that do not set their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    /// Spaces before the first column.
    pub indent: usize,
    /// Spaces between columns.
    pub gap: usize,
}

impl Default for Layout {
    fn default() -> Self {
        Self { indent: 2, gap: 2 }
    }
}

/// One cell: text, a role, and an alignment.
#[derive(Clone, Debug)]
pub struct Cell {
    text: String,
    role: Role,
    right: bool,
}

impl Cell {
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            role,
            right: false,
        }
    }

    /// Right-align within the column, for numbers.
    pub fn right(mut self) -> Self {
        self.right = true;
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn role(&self) -> Role {
        self.role
    }

    fn width(&self) -> usize {
        self.text.chars().count()
    }
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Self::new(Role::Value, text)
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Self::new(Role::Value, text)
    }
}

/// Rows of cells, columns padded to their widest entry.
#[derive(Clone, Debug, Default)]
pub struct Table {
    rows: Vec<Vec<Cell>>,
    indent: Option<usize>,
    gap: Option<usize>,
}

impl Table {
    pub fn new() -> Self {
        Self::default()
    }

    /// Override the session's [`Layout::indent`] for this table.
    pub fn indent(mut self, indent: usize) -> Self {
        self.indent = Some(indent);
        self
    }

    /// Override the session's [`Layout::gap`] for this table.
    pub fn gap(mut self, gap: usize) -> Self {
        self.gap = Some(gap);
        self
    }

    pub fn row<I, C>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = C>,
        C: Into<Cell>,
    {
        self.push(cells);
        self
    }

    pub fn push<I, C>(&mut self, cells: I)
    where
        I: IntoIterator<Item = C>,
        C: Into<Cell>,
    {
        self.rows.push(cells.into_iter().map(Into::into).collect());
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn rows(&self) -> &[Vec<Cell>] {
        &self.rows
    }

    /// One string per row. The last cell of a row is never padded.
    pub fn render(&self, style: &dyn Style, layout: Layout) -> Vec<String> {
        let indent = " ".repeat(self.indent.unwrap_or(layout.indent));
        let gap = " ".repeat(self.gap.unwrap_or(layout.gap));
        let columns = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        let widths: Vec<usize> = (0..columns)
            .map(|c| {
                self.rows
                    .iter()
                    .filter_map(|row| row.get(c))
                    .map(Cell::width)
                    .max()
                    .unwrap_or(0)
            })
            .collect();

        self.rows
            .iter()
            .map(|row| {
                let mut line = indent.clone();
                for (c, cell) in row.iter().enumerate() {
                    let pad = " ".repeat(widths[c] - cell.width());
                    let last = c + 1 == row.len();
                    if cell.right {
                        line.push_str(&pad);
                    }
                    line.push_str(&style.paint(cell.role, &cell.text));
                    if !last {
                        if !cell.right {
                            line.push_str(&pad);
                        }
                        line.push_str(&gap);
                    }
                }
                line
            })
            .collect()
    }
}

/// `key  value` lines with the keys padded to one column.
#[derive(Clone, Debug, Default)]
pub struct Block {
    table: Table,
}

impl Block {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn indent(mut self, indent: usize) -> Self {
        self.table = self.table.indent(indent);
        self
    }

    pub fn gap(mut self, gap: usize) -> Self {
        self.table = self.table.gap(gap);
        self
    }

    pub fn entry(mut self, key: impl Into<String>, value: impl Into<Cell>) -> Self {
        self.push(key, value);
        self
    }

    pub fn push(&mut self, key: impl Into<String>, value: impl Into<Cell>) {
        self.table.push([Cell::new(Role::Key, key), value.into()]);
    }

    pub fn render(&self, style: &dyn Style, layout: Layout) -> Vec<String> {
        self.table.render(style, layout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::Plain;

    #[test]
    fn columns_align_and_the_last_cell_is_not_padded() {
        let table = Table::new()
            .indent(0)
            .row([Cell::new(Role::Value, "1").right(), "aa".into(), "x".into()])
            .row([Cell::new(Role::Value, "10").right(), "b".into(), "y".into()]);
        assert_eq!(
            table.render(&Plain, Layout::default()),
            [" 1  aa  x", "10  b   y"]
        );
    }

    #[test]
    fn blocks_follow_the_session_layout_unless_overridden() {
        let block = Block::new().entry("a", "1").entry("long", "2");
        let tight = Layout { indent: 0, gap: 1 };
        assert_eq!(block.render(&Plain, tight), ["a    1", "long 2"]);
        assert_eq!(block.indent(4).render(&Plain, tight)[0], "    a    1");
    }
}
