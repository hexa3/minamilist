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
    cursor: Cursor,
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
            cursor: Cursor::default(),
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
            cursor: Cursor::default(),
            path: Some(path),
            dirty: false,
        })
    }

    pub fn insert(&mut self, text: &str) {
        self.text.insert(self.cursor.char_index, text);
        self.cursor.char_index += text.chars().count();
        self.dirty = true;
    }

    pub fn backspace(&mut self) {
        if self.cursor.char_index == 0 {
            return;
        }
        let start = self.cursor.char_index - 1;
        self.text.remove(start..self.cursor.char_index);
        self.cursor.char_index = start;
        self.dirty = true;
    }

    pub fn move_left(&mut self) {
        self.cursor.char_index = self.cursor.char_index.saturating_sub(1);
    }

    pub fn move_right(&mut self) {
        self.cursor.char_index = (self.cursor.char_index + 1).min(self.text.len_chars());
    }

    pub fn text(&self) -> String {
        self.text.to_string()
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
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
}
