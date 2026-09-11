use std::{
    fs, io,
    path::{Path, PathBuf},
};

use crate::command::fuzzy_score;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectEntry {
    pub relative_path: PathBuf,
    pub is_directory: bool,
}

#[derive(Clone, Debug)]
pub struct ProjectIndex {
    root: PathBuf,
    entries: Vec<ProjectEntry>,
}

impl ProjectIndex {
    pub fn scan(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let mut entries = Vec::new();
        scan_directory(&root, &root, &mut entries)?;
        entries.sort_unstable_by(|a, b| a.relative_path.cmp(&b.relative_path));
        Ok(Self { root, entries })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn entries(&self) -> &[ProjectEntry] {
        &self.entries
    }

    pub fn files(&self) -> impl Iterator<Item = &ProjectEntry> {
        self.entries.iter().filter(|entry| !entry.is_directory)
    }

    pub fn find_files(&self, query: &str, limit: usize) -> Vec<&ProjectEntry> {
        let mut matches: Vec<_> = self
            .files()
            .filter_map(|entry| {
                fuzzy_score(query, &entry.relative_path.to_string_lossy())
                    .map(|score| (score, entry))
            })
            .collect();
        matches.sort_unstable_by_key(|(score, entry)| {
            (std::cmp::Reverse(*score), &entry.relative_path)
        });
        matches
            .into_iter()
            .take(limit)
            .map(|(_, entry)| entry)
            .collect()
    }
}

fn scan_directory(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<ProjectEntry>,
) -> io::Result<()> {
    for child in fs::read_dir(directory)? {
        let child = child?;
        let file_type = child.file_type()?;
        let name = child.file_name();
        if file_type.is_symlink()
            || matches!(
                name.to_str(),
                Some(".git" | "target" | "node_modules" | ".apex")
            )
        {
            continue;
        }
        let path = child.path();
        entries.push(ProjectEntry {
            relative_path: path.strip_prefix(root).unwrap_or(&path).to_path_buf(),
            is_directory: file_type.is_dir(),
        });
        if file_type.is_dir() {
            scan_directory(root, &path, entries)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fuzzy_find_locates_workspace_manifest() {
        let index = ProjectIndex::scan(env!("CARGO_MANIFEST_DIR")).unwrap();
        assert_eq!(
            index.find_files("cargo", 1)[0].relative_path,
            Path::new("Cargo.toml")
        );
    }
}
