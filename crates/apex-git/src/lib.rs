//! Git operations for the built-in source-control panel.

use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileState {
    Untracked,
    Modified,
    Added,
    Deleted,
    Renamed,
    Conflicted,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusEntry {
    pub path: PathBuf,
    pub index: FileState,
    pub working_tree: FileState,
}

#[derive(Debug, Error)]
pub enum GitError {
    #[error("failed to invoke git: {0}")]
    Io(#[from] std::io::Error),
    #[error("git failed: {0}")]
    Command(String),
}

#[derive(Clone, Debug)]
pub struct GitRepository {
    root: PathBuf,
}

impl GitRepository {
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, GitError> {
        let output = Command::new("git")
            .args(["-C"])
            .arg(path.as_ref())
            .args(["rev-parse", "--show-toplevel"])
            .output()?;
        let root = command_text(output)?;
        Ok(Self {
            root: PathBuf::from(root.trim()),
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn status(&self) -> Result<Vec<StatusEntry>, GitError> {
        let output = self.git(&["status", "--porcelain=v1", "-z"])?;
        let text = command_text(output)?;
        Ok(parse_status(&text))
    }

    pub fn diff(&self, staged: bool) -> Result<String, GitError> {
        let args: &[&str] = if staged {
            &["diff", "--cached"]
        } else {
            &["diff"]
        };
        command_text(self.git(args)?)
    }

    pub fn stage(&self, paths: &[PathBuf]) -> Result<(), GitError> {
        let mut command = self.command();
        command.args(["add", "--"]);
        command.args(paths);
        command_success(command.output()?)
    }

    pub fn commit(&self, message: &str) -> Result<String, GitError> {
        command_text(self.git(&["commit", "-m", message])?)
    }

    pub fn push(&self) -> Result<String, GitError> {
        command_text(self.git(&["push"])?)
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command.args(["-C"]).arg(&self.root);
        command
    }

    fn git(&self, args: &[&str]) -> Result<Output, GitError> {
        Ok(self.command().args(args).output()?)
    }
}

fn command_text(output: Output) -> Result<String, GitError> {
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(GitError::Command(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

fn command_success(output: Output) -> Result<(), GitError> {
    command_text(output).map(|_| ())
}

fn parse_status(output: &str) -> Vec<StatusEntry> {
    output
        .split('\0')
        .filter(|record| record.len() >= 4)
        .map(|record| StatusEntry {
            index: parse_state(record.as_bytes()[0]),
            working_tree: parse_state(record.as_bytes()[1]),
            path: PathBuf::from(&record[3..]),
        })
        .collect()
}

fn parse_state(state: u8) -> FileState {
    match state {
        b'?' => FileState::Untracked,
        b'M' => FileState::Modified,
        b'A' => FileState::Added,
        b'D' => FileState::Deleted,
        b'R' => FileState::Renamed,
        b'U' => FileState::Conflicted,
        _ => FileState::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_porcelain_status_without_shell_interpolation() {
        let entries = parse_status(" M src/main.rs\0?? new file.txt\0");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].working_tree, FileState::Modified);
        assert_eq!(entries[1].index, FileState::Untracked);
        assert_eq!(entries[1].path, Path::new("new file.txt"));
    }

    #[test]
    fn discovers_the_apex_repository() {
        let repository = GitRepository::discover(env!("CARGO_MANIFEST_DIR")).unwrap();
        assert!(repository.root().join("Cargo.toml").is_file());
    }
}
