//! AI-провайдеры: Ollama / OpenAI-compat / Z.ai.
//! См. docs/PROMPT_PLAN.md раздел 4.

pub mod ollama;
pub mod openai_compat;
pub mod prompts;
pub mod types;

use serde::{Deserialize, Serialize};
use thiserror::Error;
pub use types::ChatMessage;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AiProvider {
    Ollama {
        url: String,         // http://localhost:11434
        model: String,       // llama3.1, qwen2.5, ...
    },
    OpenAiCompat {
        endpoint: String,    // https://api.openai.com/v1
        api_key: String,
        model: String,       // gpt-4o-mini, ...
    },
    Zai {
        api_key: String,
        model: String,
    },
}

#[derive(Debug, Error)]
pub enum AiError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
}

pub async fn chat(provider: &AiProvider, messages: Vec<ChatMessage>) -> Result<String, AiError> {
    match provider {
        AiProvider::Ollama { url, model } => ollama::chat(url, model, messages).await,
        AiProvider::OpenAiCompat { endpoint, api_key, model } => {
            openai_compat::chat(endpoint, api_key, model, messages).await
        }
        AiProvider::Zai { api_key, model } => {
            // Z.ai через OpenAI-compat endpoint
            openai_compat::chat(
                "https://api.z.ai/v1",
                api_key,
                model,
                messages,
            )
            .await
        }
    }
}

pub async fn test_connection(provider: &AiProvider) -> Result<bool, AiError> {
    let test_msg = vec![ChatMessage {
        role: "user".to_string(),
        content: "ping".to_string(),
    }];
    match chat(provider, test_msg).await {
        Ok(_) => Ok(true),
        Err(e) => Err(AiError::ConnectionFailed(e.to_string())),
    }
}

pub async fn list_ollama_models(url: &str) -> Result<Vec<String>, AiError> {
    ollama::list_models(url).await
}
