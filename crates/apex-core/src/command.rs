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
                id: CommandId("file.save_as"),
                title: "File: Save As",
                shortcut: Some("Ctrl+Shift+S"),
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
            Command {
                id: CommandId("editor.find"),
                title: "Edit: Find",
                shortcut: Some("Ctrl+F"),
            },
            Command {
                id: CommandId("editor.select_all"),
                title: "Edit: Select All",
                shortcut: Some("Ctrl+A"),
            },
            Command {
                id: CommandId("editor.copy"),
                title: "Edit: Copy",
                shortcut: Some("Ctrl+C"),
            },
            Command {
                id: CommandId("editor.cut"),
                title: "Edit: Cut",
                shortcut: Some("Ctrl+X"),
            },
            Command {
                id: CommandId("editor.paste"),
                title: "Edit: Paste",
                shortcut: Some("Ctrl+V"),
            },
            Command {
                id: CommandId("python.run"),
                title: "Python: Run File",
                shortcut: Some("F5"),
            },
            Command {
                id: CommandId("python.stop"),
                title: "Python: Stop",
                shortcut: Some("Shift+F5"),
            },
            Command {
                id: CommandId("output.toggle"),
                title: "View: Toggle Output",
                shortcut: Some("Ctrl+J"),
            },
            Command {
                id: CommandId("output.clear"),
                title: "Output: Clear",
                shortcut: None,
            },
            Command {
                id: CommandId("terminal.toggle"),
                title: "Terminal: Toggle",
                shortcut: Some("Ctrl+`"),
            },
            Command {
                id: CommandId("terminal.clear"),
                title: "Terminal: Clear",
                shortcut: None,
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
