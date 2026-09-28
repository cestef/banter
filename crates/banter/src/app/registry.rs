//! The command table.

use crate::command::{Builtin, Command};

/// Application commands, then built-ins. Lookup tries both in that order, so an
/// application command shadows a built-in of the same name.
pub struct Registry<S> {
    commands: Vec<Command<S>>,
    builtins: Vec<Command<S>>,
    fold: bool,
}

/// A help section: an optional heading and the commands under it.
pub type Section<'a, S> = (Option<&'static str>, Vec<&'a Command<S>>);

impl<S> Default for Registry<S> {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            builtins: vec![Builtin::help(), Builtin::exit()],
            fold: false,
        }
    }
}

impl<S> Registry<S> {
    /// Add a command, replacing any earlier one with the same name.
    pub fn add(&mut self, command: Command<S>) {
        Self::upsert(&mut self.commands, command);
    }

    /// Add a built-in, replacing any earlier one with the same name.
    pub fn builtin(&mut self, command: Command<S>) {
        Self::upsert(&mut self.builtins, command);
    }

    /// Drop every command and built-in with this name.
    pub fn remove(&mut self, name: &str) {
        self.commands.retain(|c| c.name() != name);
        self.builtins.retain(|c| c.name() != name);
    }

    /// Drop every built-in.
    pub fn clear_builtins(&mut self) {
        self.builtins.clear();
    }

    /// Match names case-insensitively.
    pub fn fold(&mut self, fold: bool) {
        self.fold = fold;
    }

    /// The command this token names, by name or alias.
    pub fn find(&self, token: &str) -> Option<&Command<S>> {
        self.all().find(|c| c.spec().answers(token, self.fold))
    }

    /// Every command, built-ins last.
    pub fn all(&self) -> impl Iterator<Item = &Command<S>> {
        self.commands.iter().chain(&self.builtins)
    }

    /// Every command not marked hidden.
    pub fn visible(&self) -> impl Iterator<Item = &Command<S>> {
        self.all().filter(|c| !c.spec().hidden)
    }

    /// Visible names, for typo suggestions.
    pub fn names(&self) -> Vec<&'static str> {
        self.visible().map(Command::name).collect()
    }

    /// Visible commands grouped by heading, in first-appearance order. Built-ins
    /// come last, in a section of their own.
    pub fn sections(&self) -> Vec<Section<'_, S>> {
        let mut sections: Vec<Section<'_, S>> = Vec::new();
        for command in self.commands.iter().filter(|c| !c.spec().hidden) {
            let group = command.spec().group;
            match sections.iter_mut().find(|(g, _)| *g == group) {
                Some((_, list)) => list.push(command),
                None => sections.push((group, vec![command])),
            }
        }
        let builtins: Vec<_> = self.builtins.iter().filter(|c| !c.spec().hidden).collect();
        if !builtins.is_empty() {
            sections.push((None, builtins));
        }
        sections
    }

    fn upsert(list: &mut Vec<Command<S>>, command: Command<S>) {
        match list.iter_mut().find(|c| c.name() == command.name()) {
            Some(slot) => *slot = command,
            None => list.push(command),
        }
    }
}
