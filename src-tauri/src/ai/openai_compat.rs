//! OpenAI-совместимый API: работает с OpenAI, Groq, OpenRouter, Together AI,
//! LiteLLM, vLLM, Z.ai, и любым другим совместимым сервером.
//! См. docs/PROMPT_PLAN.md раздел 4.1 (вариант C).

use super::{AiError, ChatMessage};

pub async fn chat(
    endpoint: &str,
    api_key: &str,
    model: &str,
    messages: Vec<ChatMessage>,
) -> Result<String, AiError> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/chat/completions", endpoint))
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "model": model,
            "messages": messages.iter().map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content
            })).collect::<Vec<_>>(),
        }))
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    Ok(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}
