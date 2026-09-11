//! Incremental Tree-sitter parsing and editor-oriented token classification.

use std::{ops::Range, path::Path};

use thiserror::Error;
use tree_sitter::{InputEdit, Parser, Point, Tree};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    C,
    Cpp,
}

impl Language {
    pub fn from_path(path: impl AsRef<Path>) -> Option<Self> {
        match path
            .as_ref()
            .extension()?
            .to_str()?
            .to_ascii_lowercase()
            .as_str()
        {
            "rs" => Some(Self::Rust),
            "py" | "pyi" => Some(Self::Python),
            "js" | "jsx" | "mjs" | "cjs" => Some(Self::JavaScript),
            "ts" | "tsx" | "mts" | "cts" => Some(Self::TypeScript),
            "go" => Some(Self::Go),
            "c" | "h" => Some(Self::C),
            "cc" | "cpp" | "cxx" | "hpp" | "hxx" => Some(Self::Cpp),
            _ => None,
        }
    }

    fn grammar(self) -> tree_sitter::Language {
        match self {
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Self::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Self::Go => tree_sitter_go::LANGUAGE.into(),
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::Cpp => tree_sitter_cpp::LANGUAGE.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HighlightKind {
    Comment,
    String,
    Number,
    Keyword,
    Function,
    Type,
    Constant,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighlightSpan {
    pub bytes: Range<usize>,
    pub kind: HighlightKind,
}

pub struct SyntaxDocument {
    language: Language,
    parser: Parser,
    tree: Tree,
    source: String,
}

#[derive(Debug, Error)]
pub enum SyntaxError {
    #[error("Tree-sitter rejected the {0:?} grammar")]
    Language(Language),
    #[error("Tree-sitter did not return a parse tree")]
    Parse,
    #[error("edit range {start}..{end} is not on UTF-8 boundaries")]
    InvalidEdit { start: usize, end: usize },
}

impl SyntaxDocument {
    pub fn new(language: Language, source: impl Into<String>) -> Result<Self, SyntaxError> {
        let source = source.into();
        let mut parser = Parser::new();
        parser
            .set_language(&language.grammar())
            .map_err(|_| SyntaxError::Language(language))?;
        let tree = parser.parse(&source, None).ok_or(SyntaxError::Parse)?;
        Ok(Self {
            language,
            parser,
            tree,
            source,
        })
    }

    pub fn language(&self) -> Language {
        self.language
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// Apply one UTF-8 byte-range replacement and reuse the prior parse tree.
    pub fn apply_edit(
        &mut self,
        old_bytes: Range<usize>,
        replacement: &str,
    ) -> Result<(), SyntaxError> {
        if old_bytes.start > old_bytes.end
            || !self.source.is_char_boundary(old_bytes.start)
            || !self.source.is_char_boundary(old_bytes.end)
        {
            return Err(SyntaxError::InvalidEdit {
                start: old_bytes.start,
                end: old_bytes.end,
            });
        }
        let start_position = point_at(&self.source, old_bytes.start);
        let old_end_position = point_at(&self.source, old_bytes.end);
        self.source.replace_range(old_bytes.clone(), replacement);
        let new_end_byte = old_bytes.start + replacement.len();
        let new_end_position = point_at(&self.source, new_end_byte);
        self.tree.edit(&InputEdit {
            start_byte: old_bytes.start,
            old_end_byte: old_bytes.end,
            new_end_byte,
            start_position,
            old_end_position,
            new_end_position,
        });
        self.tree = self
            .parser
            .parse(&self.source, Some(&self.tree))
            .ok_or(SyntaxError::Parse)?;
        Ok(())
    }

    /// Synchronize to a new source snapshot using the smallest single replacement.
    pub fn update_source(&mut self, new_source: &str) -> Result<(), SyntaxError> {
        if self.source == new_source {
            return Ok(());
        }
        let prefix = self
            .source
            .chars()
            .zip(new_source.chars())
            .take_while(|(left, right)| left == right)
            .map(|(character, _)| character.len_utf8())
            .sum::<usize>();
        let old_tail = &self.source[prefix..];
        let new_tail = &new_source[prefix..];
        let suffix = old_tail
            .chars()
            .rev()
            .zip(new_tail.chars().rev())
            .take_while(|(left, right)| left == right)
            .map(|(character, _)| character.len_utf8())
            .sum::<usize>();
        let old_end = self.source.len() - suffix;
        let new_end = new_source.len() - suffix;
        self.apply_edit(prefix..old_end, &new_source[prefix..new_end])
    }

    pub fn highlights(&self) -> Vec<HighlightSpan> {
        let mut spans = Vec::new();
        let mut cursor = self.tree.walk();
        collect_highlights(&mut cursor, &mut spans);
        spans
    }
}

fn point_at(source: &str, byte: usize) -> Point {
    let prefix = &source[..byte];
    let row = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len(), |(_, tail)| tail.len());
    Point::new(row, column)
}

fn collect_highlights(cursor: &mut tree_sitter::TreeCursor<'_>, spans: &mut Vec<HighlightSpan>) {
    let node = cursor.node();
    if let Some(kind) = classify(node.kind()) {
        spans.push(HighlightSpan {
            bytes: node.byte_range(),
            kind,
        });
        return;
    }
    if cursor.goto_first_child() {
        loop {
            collect_highlights(cursor, spans);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn classify(kind: &str) -> Option<HighlightKind> {
    if kind.contains("comment") {
        return Some(HighlightKind::Comment);
    }
    if kind.contains("string") || matches!(kind, "char_literal" | "template_string") {
        return Some(HighlightKind::String);
    }
    if kind.contains("integer") || kind.contains("float") || kind == "number" {
        return Some(HighlightKind::Number);
    }
    if matches!(kind, "type_identifier" | "primitive_type") {
        return Some(HighlightKind::Type);
    }
    if matches!(kind, "true" | "false" | "null" | "None" | "nil") {
        return Some(HighlightKind::Constant);
    }
    if matches!(
        kind,
        "fn" | "let"
            | "mut"
            | "pub"
            | "impl"
            | "trait"
            | "use"
            | "mod"
            | "struct"
            | "enum"
            | "match"
            | "if"
            | "else"
            | "for"
            | "while"
            | "loop"
            | "return"
            | "async"
            | "await"
            | "def"
            | "class"
            | "import"
            | "from"
            | "as"
            | "const"
            | "var"
            | "function"
            | "interface"
            | "package"
            | "func"
            | "defer"
            | "go"
            | "switch"
            | "case"
            | "break"
            | "continue"
            | "typedef"
            | "namespace"
            | "template"
    ) {
        return Some(HighlightKind::Keyword);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_all_day_one_languages() {
        for (path, language) in [
            ("main.rs", Language::Rust),
            ("main.py", Language::Python),
            ("main.js", Language::JavaScript),
            ("main.ts", Language::TypeScript),
            ("main.go", Language::Go),
            ("main.c", Language::C),
            ("main.cpp", Language::Cpp),
        ] {
            assert_eq!(Language::from_path(path), Some(language));
        }
    }

    #[test]
    fn incrementally_reparses_and_classifies_rust() {
        let mut document = SyntaxDocument::new(Language::Rust, "fn main() { let n = 1; }").unwrap();
        let one = document.source().find('1').unwrap();
        document.apply_edit(one..one + 1, "42").unwrap();
        assert!(document.highlights().iter().any(|span| {
            span.kind == HighlightKind::Number && &document.source()[span.bytes.clone()] == "42"
        }));
    }

    #[test]
    fn snapshot_sync_preserves_unicode_boundaries() {
        let mut document = SyntaxDocument::new(Language::Python, "print('🦀')").unwrap();
        document.update_source("print('fast 🦀')").unwrap();
        assert_eq!(document.source(), "print('fast 🦀')");
    }
}
