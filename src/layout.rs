//! Aligned output: key-value blocks and tables.
//!
//! Widths are measured on the unstyled text and padding happens before painting, so
//! columns line up whatever the [`Style`] does.

use crate::style::{Role, Style};

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
    indent: usize,
    gap: usize,
}

impl Table {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            indent: 2,
            gap: 2,
        }
    }

    pub fn indent(mut self, indent: usize) -> Self {
        self.indent = indent;
        self
    }

    pub fn gap(mut self, gap: usize) -> Self {
        self.gap = gap;
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

    pub fn render(&self, style: &dyn Style) -> Vec<String> {
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
                let mut line = " ".repeat(self.indent);
                for (c, cell) in row.iter().enumerate() {
                    let last = c + 1 == row.len();
                    let pad = widths[c] - cell.width();
                    if cell.right {
                        line.push_str(&" ".repeat(pad));
                    }
                    line.push_str(&style.paint(cell.role, &cell.text));
                    if !last {
                        if !cell.right {
                            line.push_str(&" ".repeat(pad));
                        }
                        line.push_str(&" ".repeat(self.gap));
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
        Self {
            table: Table::new(),
        }
    }

    pub fn entry(mut self, key: impl Into<String>, value: impl Into<Cell>) -> Self {
        self.push(key, value);
        self
    }

    pub fn push(&mut self, key: impl Into<String>, value: impl Into<Cell>) {
        self.table.push([Cell::new(Role::Key, key), value.into()]);
    }

    pub fn render(&self, style: &dyn Style) -> Vec<String> {
        self.table.render(style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Plain;

    #[test]
    fn columns_align_and_trailing_space_is_trimmed() {
        let table = Table::new()
            .indent(0)
            .row([Cell::new(Role::Value, "1").right(), "aa".into(), "x".into()])
            .row([Cell::new(Role::Value, "10").right(), "b".into(), "y".into()]);
        assert_eq!(table.render(&Plain), [" 1  aa  x", "10  b   y"]);
    }

    #[test]
    fn blocks_pad_the_key_column() {
        let block = Block::new().entry("a", "1").entry("long", "2");
        assert_eq!(block.render(&Plain), ["  a     1", "  long  2"]);
    }
}
