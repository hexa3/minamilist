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
    #[error("cannot save an untitled buffer; choose a path first")]
    Untitled,
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

    pub fn new_at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: Some(path.into()),
            ..Self::new()
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, BufferError> {
        let path = path.as_ref().to_path_buf();
        let file = fs::File::open(&path).map_err(|source| BufferError::Io {
            path: path.to_path_buf(),
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

    pub fn move_to_document_start(&mut self) {
        for cursor in &mut self.cursors {
            cursor.char_index = 0;
        }
    }

    pub fn move_to_document_end(&mut self) {
        let end = self.text.len_chars();
        for cursor in &mut self.cursors {
            cursor.char_index = end;
        }
    }

    pub fn move_word_left(&mut self) {
        let text = &self.text;
        for cursor in &mut self.cursors {
            let mut index = cursor.char_index;
            while index > 0 && text.char(index - 1).is_whitespace() {
                index -= 1;
            }
            while index > 0 && is_word_character(text.char(index - 1)) {
                index -= 1;
            }
            cursor.char_index = index;
        }
    }

    pub fn move_word_right(&mut self) {
        let text = &self.text;
        let end = text.len_chars();
        for cursor in &mut self.cursors {
            let mut index = cursor.char_index;
            while index < end && is_word_character(text.char(index)) {
                index += 1;
            }
            while index < end && text.char(index).is_whitespace() {
                index += 1;
            }
            cursor.char_index = index;
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

    pub fn line_column(&self) -> (usize, usize) {
        let cursor = self.cursor().char_index;
        let line = self.text.char_to_line(cursor);
        (line, cursor - self.text.line_to_char(line))
    }

    pub fn save(&mut self) -> Result<(), BufferError> {
        let path = self.path.clone().ok_or(BufferError::Untitled)?;
        self.write_to_path(&path)?;
        self.dirty = false;
        Ok(())
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<(), BufferError> {
        let path = path.as_ref().to_path_buf();
        self.write_to_path(&path)?;
        self.path = Some(path);
        self.dirty = false;
        Ok(())
    }

    fn write_to_path(&self, path: &Path) -> Result<(), BufferError> {
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        let mut temporary =
            tempfile::NamedTempFile::new_in(directory).map_err(|source| BufferError::Io {
                path: directory.to_path_buf(),
                source,
            })?;
        self.text
            .write_to(temporary.as_file_mut())
            .map_err(|source| BufferError::Io {
                path: temporary.path().to_path_buf(),
                source,
            })?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|source| BufferError::Io {
                path: temporary.path().to_path_buf(),
                source,
            })?;
        if let Ok(metadata) = fs::metadata(path) {
            temporary
                .as_file()
                .set_permissions(metadata.permissions())
                .map_err(|source| BufferError::Io {
                    path: temporary.path().to_path_buf(),
                    source,
                })?;
        }
        temporary.persist(path).map_err(|error| BufferError::Io {
            path: path.to_path_buf(),
            source: error.error,
        })?;
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

fn is_word_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "minamilist-buffer-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn edits_unicode_by_character_not_byte() {
        let mut buffer = Buffer::new();
        buffer.insert("code 🦀");
        buffer.backspace();
        assert_eq!(buffer.text(), "code ");
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

    #[test]
    fn word_and_document_navigation_are_unicode_safe() {
        let mut buffer = Buffer::from_text("one café\nthree");
        buffer.move_to_document_end();
        assert_eq!(buffer.line_column(), (1, 5));
        buffer.move_word_left();
        assert_eq!(buffer.line_column(), (1, 0));
        buffer.move_word_left();
        assert_eq!(buffer.line_column(), (0, 4));
        buffer.move_word_right();
        assert_eq!(buffer.line_column(), (1, 0));
        buffer.move_to_document_start();
        assert_eq!(buffer.cursor().char_index, 0);
    }

    #[test]
    fn untitled_buffers_require_save_as() {
        let mut buffer = Buffer::from_text("draft");
        assert!(matches!(buffer.save(), Err(BufferError::Untitled)));
    }

    #[test]
    fn new_file_can_be_assigned_before_its_first_save() {
        let path = test_path("new.py");
        let mut buffer = Buffer::new_at(&path);
        buffer.insert("print('hello')\n");
        buffer.save().unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "print('hello')\n");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn save_as_assigns_a_path_and_clears_dirty_state() {
        let path = test_path("save-as.txt");
        let mut buffer = Buffer::from_text("hello");
        buffer.set_cursor(buffer.len_chars());
        buffer.insert(" world");
        buffer.save_as(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "hello world");
        assert_eq!(buffer.path(), Some(path.as_path()));
        assert!(!buffer.is_dirty());
        fs::remove_file(path).unwrap();
    }
}
