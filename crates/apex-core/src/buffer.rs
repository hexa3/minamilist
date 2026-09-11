use std::{
    fs, io,
    path::{Path, PathBuf},
};

use ropey::Rope;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Cursor {
    pub char_index: usize,
}

#[derive(Debug)]
pub struct Buffer {
    text: Rope,
    cursors: Vec<Cursor>,
    path: Option<PathBuf>,
    dirty: bool,
}

#[derive(Debug, Error)]
pub enum BufferError {
    #[error("failed to access {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            text: Rope::new(),
            cursors: vec![Cursor::default()],
            path: None,
            dirty: false,
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, BufferError> {
        let path = path.as_ref().to_path_buf();
        let file = fs::File::open(&path).map_err(|source| BufferError::Io {
            path: path.clone(),
            source,
        })?;
        let text = Rope::from_reader(file).map_err(|source| BufferError::Io {
            path: path.clone(),
            source,
        })?;
        Ok(Self {
            text,
            cursors: vec![Cursor::default()],
            path: Some(path),
            dirty: false,
        })
    }

    pub fn insert(&mut self, text: &str) {
        let inserted = text.chars().count();
        let mut positions: Vec<usize> = self
            .cursors
            .iter()
            .map(|cursor| cursor.char_index)
            .collect();
        positions.sort_unstable();
        positions.dedup();
        for position in positions.iter().rev() {
            self.text.insert(*position, text);
        }
        for cursor in &mut self.cursors {
            let preceding = positions
                .iter()
                .filter(|position| **position <= cursor.char_index)
                .count();
            cursor.char_index += inserted * preceding;
        }
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        let mut positions: Vec<usize> = self
            .cursors
            .iter()
            .filter_map(|cursor| cursor.char_index.checked_sub(1))
            .collect();
        positions.sort_unstable();
        positions.dedup();
        if positions.is_empty() {
            return;
        }
        for position in positions.iter().rev() {
            self.text.remove(*position..*position + 1);
        }
        for cursor in &mut self.cursors {
            let removed_before = positions
                .iter()
                .filter(|position| **position < cursor.char_index)
                .count();
            cursor.char_index = cursor.char_index.saturating_sub(removed_before);
        }
        self.dirty = true;
    }

    pub fn move_left(&mut self) {
        for cursor in &mut self.cursors {
            cursor.char_index = cursor.char_index.saturating_sub(1);
        }
    }

    pub fn move_right(&mut self) {
        for cursor in &mut self.cursors {
            cursor.char_index = (cursor.char_index + 1).min(self.text.len_chars());
        }
    }

    pub fn text(&self) -> String {
        self.text.to_string()
    }

    pub fn cursor(&self) -> Cursor {
        self.cursors[0]
    }

    pub fn cursors(&self) -> &[Cursor] {
        &self.cursors
    }

    pub fn set_cursor(&mut self, char_index: usize) {
        self.cursors.clear();
        self.cursors.push(Cursor {
            char_index: char_index.min(self.text.len_chars()),
        });
    }

    pub fn add_cursor(&mut self, char_index: usize) {
        let cursor = Cursor {
            char_index: char_index.min(self.text.len_chars()),
        };
        if !self.cursors.contains(&cursor) {
            self.cursors.push(cursor);
            self.cursors
                .sort_unstable_by_key(|cursor| cursor.char_index);
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn save(&mut self) -> Result<(), BufferError> {
        let Some(path) = self.path.clone() else {
            return Ok(());
        };
        let file = fs::File::create(&path).map_err(|source| BufferError::Io {
            path: path.clone(),
            source,
        })?;
        self.text
            .write_to(file)
            .map_err(|source| BufferError::Io { path, source })?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_unicode_by_character_not_byte() {
        let mut buffer = Buffer::new();
        buffer.insert("Apex 🦀");
        buffer.backspace();
        assert_eq!(buffer.text(), "Apex ");
        assert_eq!(buffer.cursor().char_index, 5);
    }

    #[test]
    fn cursor_stays_inside_buffer() {
        let mut buffer = Buffer::new();
        buffer.move_right();
        assert_eq!(buffer.cursor().char_index, 0);
    }

    #[test]
    fn multi_cursor_edits_are_applied_atomically() {
        let mut buffer = Buffer::new();
        buffer.insert("abcd");
        buffer.set_cursor(1);
        buffer.add_cursor(3);
        buffer.insert("X");
        assert_eq!(buffer.text(), "aXbcXd");
        assert_eq!(
            buffer.cursors(),
            &[Cursor { char_index: 2 }, Cursor { char_index: 5 }]
        );
        buffer.backspace();
        assert_eq!(buffer.text(), "abcd");
    }
}
