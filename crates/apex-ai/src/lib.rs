//! Provider-neutral AI, repository retrieval, and reviewed edit orchestration.

mod agent;
mod index;
mod provider;

pub use agent::{Agent, AgentError, EditDecision, FileEdit, ProposedChange};
pub use index::{CodeChunk, RepositoryIndex, SearchHit};
pub use provider::{
    AnthropicProvider, CompletionRequest, ModelProvider, NetworkPolicy, ProviderError,
};
