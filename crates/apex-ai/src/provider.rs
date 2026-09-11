use reqwest::blocking::Client;
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkPolicy {
    Allow,
    LocalOnly,
}

#[derive(Clone, Debug, Serialize)]
pub struct CompletionRequest {
    pub system: String,
    pub prompt: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("network AI is disabled by local-only mode")]
    NetworkDisabled,
    #[error("provider request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("provider returned an invalid response: {0}")]
    InvalidResponse(String),
    #[error("provider does not support embeddings")]
    EmbeddingsUnsupported,
}

pub trait ModelProvider: Send + Sync {
    fn name(&self) -> &str;
    fn is_networked(&self) -> bool;
    fn complete(
        &self,
        request: &CompletionRequest,
        policy: NetworkPolicy,
    ) -> Result<String, ProviderError>;
    fn embed(
        &self,
        _texts: &[String],
        _policy: NetworkPolicy,
    ) -> Result<Vec<Vec<f32>>, ProviderError> {
        Err(ProviderError::EmbeddingsUnsupported)
    }
}

pub struct AnthropicProvider {
    api_key: String,
    model: String,
    endpoint: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: model.into(),
            endpoint: "https://api.anthropic.com/v1/messages".into(),
            client: Client::new(),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }
}

impl ModelProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn is_networked(&self) -> bool {
        true
    }

    fn complete(
        &self,
        request: &CompletionRequest,
        policy: NetworkPolicy,
    ) -> Result<String, ProviderError> {
        if policy == NetworkPolicy::LocalOnly {
            return Err(ProviderError::NetworkDisabled);
        }
        let response: Value = self
            .client
            .post(&self.endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&json!({
                "model": self.model,
                "system": request.system,
                "messages": [{ "role": "user", "content": request.prompt }],
                "max_tokens": request.max_tokens,
                "temperature": request.temperature
            }))
            .send()?
            .error_for_status()?
            .json()?;
        response
            .get("content")
            .and_then(Value::as_array)
            .and_then(|blocks| {
                blocks
                    .iter()
                    .find(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            })
            .and_then(|block| block.get("text"))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| ProviderError::InvalidResponse(response.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn privacy_mode_blocks_before_any_request() {
        let provider =
            AnthropicProvider::new("never-sent", "model").with_endpoint("http://127.0.0.1:1");
        let error = provider
            .complete(
                &CompletionRequest {
                    system: String::new(),
                    prompt: "secret".into(),
                    max_tokens: 1,
                    temperature: 0.0,
                },
                NetworkPolicy::LocalOnly,
            )
            .unwrap_err();
        assert!(matches!(error, ProviderError::NetworkDisabled));
    }
}
