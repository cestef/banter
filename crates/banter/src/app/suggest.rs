//! Typo suggestions for unknown commands.

/// Picks the command a mistyped token most likely meant.
pub trait Suggest: Send + Sync {
    fn suggest(&self, token: &str, names: &[&str]) -> Option<String>;
}

impl<F> Suggest for F
where
    F: Fn(&str, &[&str]) -> Option<String> + Send + Sync,
{
    fn suggest(&self, token: &str, names: &[&str]) -> Option<String> {
        self(token, names)
    }
}

/// Closest name by edit distance, if within `len / ratio` edits (at least `floor`).
#[derive(Clone, Copy, Debug)]
pub struct Distance {
    pub ratio: usize,
    pub floor: usize,
}

impl Default for Distance {
    fn default() -> Self {
        Self { ratio: 3, floor: 1 }
    }
}

impl Distance {
    /// Levenshtein distance over chars.
    pub fn between(a: &str, b: &str) -> usize {
        let b: Vec<char> = b.chars().collect();
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut diag = row[0];
            row[0] = i + 1;
            for (j, &cb) in b.iter().enumerate() {
                let next = (diag + usize::from(ca != cb))
                    .min(row[j] + 1)
                    .min(row[j + 1] + 1);
                diag = row[j + 1];
                row[j + 1] = next;
            }
        }
        row[b.len()]
    }
}

impl Suggest for Distance {
    fn suggest(&self, token: &str, names: &[&str]) -> Option<String> {
        let limit = (token.chars().count() / self.ratio.max(1)).max(self.floor);
        names
            .iter()
            .map(|name| (Self::between(token, name), name))
            .filter(|&(d, _)| d <= limit)
            .min_by_key(|&(d, _)| d)
            .map(|(_, name)| name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggests_near_misses_only() {
        let names = ["open", "sign", "submit", "help"];
        let d = Distance::default();
        assert_eq!(d.suggest("sigm", &names).as_deref(), Some("sign"));
        assert_eq!(d.suggest("submti", &names).as_deref(), Some("submit"));
        assert_eq!(d.suggest("xyzzy", &names), None);
    }
}
