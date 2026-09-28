//! Turning a line into a command name and its arguments.

use crate::error::{Error, Result};

/// Splits one input line into tokens. The first token names the command.
///
/// Returning `Ok(None)` skips the line silently (blank lines, comments).
pub trait Lexer: Send + Sync {
    /// Tokens of `line`, or `None` to skip it.
    fn split(&self, line: &str) -> Result<Option<Vec<String>>>;
}

impl<F> Lexer for F
where
    F: Fn(&str) -> Result<Option<Vec<String>>> + Send + Sync,
{
    fn split(&self, line: &str) -> Result<Option<Vec<String>>> {
        self(line)
    }
}

/// Split on whitespace. Lines starting with the comment marker are skipped.
#[derive(Clone, Copy, Debug)]
pub struct Words {
    /// Lines starting with this are skipped.
    pub comment: Option<char>,
}

impl Default for Words {
    fn default() -> Self {
        Self { comment: Some('#') }
    }
}

impl Lexer for Words {
    fn split(&self, line: &str) -> Result<Option<Vec<String>>> {
        let line = line.trim();
        if line.is_empty() || self.comment.is_some_and(|c| line.starts_with(c)) {
            return Ok(None);
        }
        Ok(Some(line.split_whitespace().map(str::to_string).collect()))
    }
}

/// Shell-like: single and double quotes group words, backslash escapes the next
/// character outside single quotes. An unterminated quote is a recoverable error.
#[derive(Clone, Copy, Debug)]
pub struct Shell {
    /// Lines starting with this are skipped.
    pub comment: Option<char>,
}

impl Default for Shell {
    fn default() -> Self {
        Self { comment: Some('#') }
    }
}

impl Lexer for Shell {
    fn split(&self, line: &str) -> Result<Option<Vec<String>>> {
        let line = line.trim();
        if line.is_empty() || self.comment.is_some_and(|c| line.starts_with(c)) {
            return Ok(None);
        }

        let mut tokens = Vec::new();
        let mut current: Option<String> = None;
        let mut quote: Option<char> = None;
        let mut chars = line.chars();

        while let Some(c) = chars.next() {
            match (quote, c) {
                (Some(q), c) if c == q => quote = None,
                (Some('\''), c) => current.get_or_insert_default().push(c),
                (_, '\\') => {
                    let next = chars
                        .next()
                        .ok_or_else(|| Error::Syntax("trailing backslash".into()))?;
                    current.get_or_insert_default().push(next);
                }
                (None, '"' | '\'') => {
                    quote = Some(c);
                    current.get_or_insert_default();
                }
                (None, c) if c.is_whitespace() => tokens.extend(current.take()),
                (_, c) => current.get_or_insert_default().push(c),
            }
        }
        if let Some(q) = quote {
            return Err(Error::Syntax(format!("unterminated {q} quote")));
        }
        tokens.extend(current);
        Ok(Some(tokens))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell(line: &str) -> Vec<String> {
        Shell::default().split(line).unwrap().unwrap()
    }

    #[test]
    fn whitespace_skips_blanks_and_comments() {
        let t = Words::default();
        assert_eq!(t.split("  # hi").unwrap(), None);
        assert_eq!(t.split("   ").unwrap(), None);
        assert_eq!(t.split(" a  b ").unwrap().unwrap(), ["a", "b"]);
        let bare = Words { comment: None };
        assert_eq!(bare.split("# a").unwrap().unwrap(), ["#", "a"]);
    }

    #[test]
    fn quotes_group_and_escape() {
        assert_eq!(
            shell(r#"say "hello world" 'a "b"' c\ d"#),
            ["say", "hello world", r#"a "b""#, "c d"]
        );
        assert_eq!(shell(r#"put """#), ["put", ""]);
    }

    #[test]
    fn unterminated_quotes_are_recoverable() {
        let err = Shell::default().split("say \"oops").unwrap_err();
        assert!(err.recoverable());
    }
}
