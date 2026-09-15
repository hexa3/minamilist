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
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
}

#[derive(Clone, Debug)]
struct Snapshot {
    text: Rope,
    cursors: Vec<Cursor>,
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
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn from_text(text: impl AsRef<str>) -> Self {
        Self {
            text: Rope::from_str(text.as_ref()),
            cursors: vec![Cursor::default()],
            path: None,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
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
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        })
    }

    pub fn insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.checkpoint();
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
        self.checkpoint();
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

    pub fn delete_forward(&mut self) {
        let mut positions: Vec<usize> = self
            .cursors
            .iter()
            .map(|cursor| cursor.char_index)
            .filter(|position| *position < self.text.len_chars())
            .collect();
        positions.sort_unstable();
        positions.dedup();
        if positions.is_empty() {
            return;
        }
        self.checkpoint();
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

    pub fn move_up(&mut self) {
        let text = &self.text;
        for cursor in &mut self.cursors {
            let line = text.char_to_line(cursor.char_index);
            if line == 0 {
                continue;
            }
            let column = cursor.char_index - text.line_to_char(line);
            let target_line = line - 1;
            cursor.char_index =
                text.line_to_char(target_line) + column.min(line_content_len(text, target_line));
        }
    }

    pub fn move_down(&mut self) {
        let text = &self.text;
        for cursor in &mut self.cursors {
            let line = text.char_to_line(cursor.char_index);
            if line + 1 >= text.len_lines() {
                continue;
            }
            let column = cursor.char_index - text.line_to_char(line);
            let target_line = line + 1;
            cursor.char_index =
                text.line_to_char(target_line) + column.min(line_content_len(text, target_line));
        }
    }

    pub fn move_to_line_start(&mut self) {
        let text = &self.text;
        for cursor in &mut self.cursors {
            cursor.char_index = text.line_to_char(text.char_to_line(cursor.char_index));
        }
    }

    pub fn move_to_line_end(&mut self) {
        let text = &self.text;
        for cursor in &mut self.cursors {
            let line = text.char_to_line(cursor.char_index);
            cursor.char_index = text.line_to_char(line) + line_content_len(text, line);
        }
    }

    pub fn undo(&mut self) -> bool {
        let Some(snapshot) = self.undo_stack.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.restore(snapshot);
        self.redo_stack.push(current);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(snapshot) = self.redo_stack.pop() else {
            return false;
        };
        let current = self.snapshot();
        self.restore(snapshot);
        self.undo_stack.push(current);
        true
    }

    pub fn text(&self) -> String {
        self.text.to_string()
    }

    pub fn len_chars(&self) -> usize {
        self.text.len_chars()
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

    fn checkpoint(&mut self) {
        const HISTORY_LIMIT: usize = 1_000;
        if self.undo_stack.len() == HISTORY_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.snapshot());
        self.redo_stack.clear();
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            cursors: self.cursors.clone(),
            dirty: self.dirty,
        }
    }

    fn restore(&mut self, snapshot: Snapshot) {
        self.text = snapshot.text;
        self.cursors = snapshot.cursors;
        self.dirty = snapshot.dirty;
    }
}

fn line_content_len(text: &Rope, line: usize) -> usize {
    let slice = text.line(line);
    let mut length = slice.len_chars();
    if length > 0 && slice.char(length - 1) == '\n' {
        length -= 1;
        if length > 0 && slice.char(length - 1) == '\r' {
            length -= 1;
        }
    }
    length
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

    #[test]
    fn undo_and_redo_restore_text_cursors_and_dirty_state() {
        let mut buffer = Buffer::from_text("one");
        buffer.set_cursor(3);
        buffer.insert(" two");
        assert!(buffer.is_dirty());
        assert!(buffer.undo());
        assert_eq!(buffer.text(), "one");
        assert_eq!(buffer.cursor().char_index, 3);
        assert!(!buffer.is_dirty());
        assert!(buffer.redo());
        assert_eq!(buffer.text(), "one two");
        assert!(buffer.is_dirty());
    }

    #[test]
    fn vertical_navigation_clamps_to_shorter_lines() {
        let mut buffer = Buffer::from_text("alpha\nx\nomega");
        buffer.set_cursor(4);
        buffer.move_down();
        assert_eq!(buffer.cursor().char_index, 7);
        buffer.move_down();
        assert_eq!(buffer.cursor().char_index, 9);
        buffer.move_to_line_end();
        assert_eq!(buffer.cursor().char_index, buffer.len_chars());
        buffer.move_to_line_start();
        assert_eq!(buffer.cursor().char_index, 8);
        buffer.move_up();
        assert_eq!(buffer.cursor().char_index, 6);
    }

    #[test]
    fn forward_delete_handles_multiple_cursors() {
        let mut buffer = Buffer::from_text("abcd");
        buffer.set_cursor(0);
        buffer.add_cursor(2);
        buffer.delete_forward();
        assert_eq!(buffer.text(), "bd");
        assert_eq!(
            buffer.cursors(),
            &[Cursor { char_index: 0 }, Cursor { char_index: 1 }]
        );
    }
}
