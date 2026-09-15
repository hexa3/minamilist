use std::borrow::Cow;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CommandId(pub &'static str);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Command {
    pub id: CommandId,
    pub title: &'static str,
    pub shortcut: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct CommandRegistry {
    commands: Vec<Command>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

impl CommandRegistry {
    pub fn with_builtins() -> Self {
        let commands = vec![
            Command {
                id: CommandId("file.open"),
                title: "File: Open",
                shortcut: Some("Ctrl+O"),
            },
            Command {
                id: CommandId("file.save"),
                title: "File: Save",
                shortcut: Some("Ctrl+S"),
            },
            Command {
                id: CommandId("file.find"),
                title: "Go to File",
                shortcut: Some("Ctrl+P"),
            },
            Command {
                id: CommandId("palette.open"),
                title: "Show Command Palette",
                shortcut: Some("Ctrl+Shift+P"),
            },
            Command {
                id: CommandId("editor.undo"),
                title: "Edit: Undo",
                shortcut: Some("Ctrl+Z"),
            },
            Command {
                id: CommandId("editor.redo"),
                title: "Edit: Redo",
                shortcut: Some("Ctrl+Shift+Z"),
            },
        ];
        Self { commands }
    }

    pub fn register(&mut self, command: Command) {
        if let Some(existing) = self.commands.iter_mut().find(|item| item.id == command.id) {
            *existing = command;
        } else {
            self.commands.push(command);
        }
    }

    pub fn commands(&self) -> &[Command] {
        &self.commands
    }

    pub fn search(&self, query: &str) -> Vec<&Command> {
        let mut matches: Vec<_> = self
            .commands
            .iter()
            .filter_map(|command| fuzzy_score(query, command.title).map(|score| (score, command)))
            .collect();
        matches.sort_unstable_by_key(|(score, command)| (std::cmp::Reverse(*score), command.title));
        matches.into_iter().map(|(_, command)| command).collect()
    }
}

pub(crate) fn fuzzy_score(needle: &str, haystack: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let needle: Cow<'_, str> = Cow::Owned(needle.to_lowercase());
    let haystack: Cow<'_, str> = Cow::Owned(haystack.to_lowercase());
    let mut score = 0;
    let mut from = 0;
    let mut previous = None;
    for wanted in needle.chars() {
        let (offset, _) = haystack[from..]
            .char_indices()
            .find(|(_, candidate)| *candidate == wanted)?;
        let found = from + offset;
        score += 10;
        if previous.is_some_and(|index| found == index + 1) {
            score += 8;
        }
        if found == 0
            || haystack
                .as_bytes()
                .get(found.wrapping_sub(1))
                .is_some_and(|byte| matches!(byte, b'/' | b'_' | b'-' | b' '))
        {
            score += 5;
        }
        previous = Some(found);
        from = found + wanted.len_utf8();
    }
    score -= haystack.len().saturating_sub(needle.len()) as i32;
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_prioritizes_compact_matches() {
        let registry = CommandRegistry::default();
        let results = registry.search("save");
        assert_eq!(results[0].id, CommandId("file.save"));
    }
}
