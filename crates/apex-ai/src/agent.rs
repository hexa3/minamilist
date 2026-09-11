use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{CompletionRequest, ModelProvider, NetworkPolicy, ProviderError, RepositoryIndex};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileEdit {
    pub path: PathBuf,
    pub original: String,
    pub replacement: String,
    pub rationale: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProposedChange {
    pub summary: String,
    pub plan: Vec<String>,
    pub edits: Vec<FileEdit>,
    #[serde(default)]
    pub verification: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EditDecision {
    Accept,
    Reject,
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("model output was not a valid change proposal: {0}")]
    InvalidProposal(String),
    #[error("unsafe edit path: {0}")]
    UnsafePath(PathBuf),
    #[error("file changed since proposal: {0}")]
    StaleFile(PathBuf),
    #[error("could not apply edit: {0}")]
    Io(#[from] io::Error),
}

pub struct Agent<'a> {
    provider: &'a dyn ModelProvider,
    policy: NetworkPolicy,
}

impl<'a> Agent<'a> {
    pub fn new(provider: &'a dyn ModelProvider, policy: NetworkPolicy) -> Self {
        Self { provider, policy }
    }

    pub fn propose(
        &self,
        task: &str,
        index: &RepositoryIndex,
    ) -> Result<ProposedChange, AgentError> {
        let context = index
            .search(task, 8)
            .into_iter()
            .map(|hit| {
                format!(
                    "FILE {}:{}-{}\n{}",
                    hit.chunk.path.display(),
                    hit.chunk.start_line,
                    hit.chunk.end_line,
                    hit.chunk.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        let response = self.provider.complete(
            &CompletionRequest {
                system: "You are Apex's editing agent. Return only JSON with summary, plan, edits (path, original, replacement, rationale), and verification. Use exact original text. Never use absolute paths.".into(),
                prompt: format!("TASK\n{task}\n\nREPOSITORY CONTEXT\n{context}"),
                max_tokens: 8192,
                temperature: 0.1,
            },
            self.policy,
        )?;
        let json = response
            .strip_prefix("```json")
            .and_then(|text| text.strip_suffix("```"))
            .unwrap_or(&response)
            .trim();
        let proposal: ProposedChange = serde_json::from_str(json)
            .map_err(|error| AgentError::InvalidProposal(error.to_string()))?;
        for edit in &proposal.edits {
            validate_relative_path(&edit.path)?;
        }
        Ok(proposal)
    }

    /// Apply only explicitly accepted files after checking originals are current.
    pub fn apply(
        &self,
        root: impl AsRef<Path>,
        proposal: &ProposedChange,
        decisions: &[EditDecision],
    ) -> Result<Vec<PathBuf>, AgentError> {
        let root = root.as_ref();
        let accepted: Vec<_> = proposal
            .edits
            .iter()
            .zip(decisions)
            .filter(|(_, decision)| **decision == EditDecision::Accept)
            .map(|(edit, _)| edit)
            .collect();
        for edit in &accepted {
            validate_relative_path(&edit.path)?;
            let current = fs::read_to_string(root.join(&edit.path))?;
            if current != edit.original {
                return Err(AgentError::StaleFile(edit.path.clone()));
            }
        }
        for edit in &accepted {
            fs::write(root.join(&edit.path), &edit.replacement)?;
        }
        Ok(accepted.into_iter().map(|edit| edit.path.clone()).collect())
    }
}

fn validate_relative_path(path: &Path) -> Result<(), AgentError> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AgentError::UnsafePath(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal_before_apply() {
        assert!(matches!(
            validate_relative_path(Path::new("../secret")),
            Err(AgentError::UnsafePath(_))
        ));
    }
}
