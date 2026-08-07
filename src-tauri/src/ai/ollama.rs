//! Ollama — локальные модели, бесплатно, офлайн.
//! См. docs/PROMPT_PLAN.md раздел 4.1 (вариант A).

use super::{AiError, ChatMessage};

pub async fn chat(url: &str, model: &str, messages: Vec<ChatMessage>) -> Result<String, AiError> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{}/api/chat", url))
        .json(&serde_json::json!({
            "model": model,
            "messages": messages.iter().map(|m| serde_json::json!({
                "role": m.role,
                "content": m.content
            })).collect::<Vec<_>>(),
            "stream": false
        }))
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    Ok(body["message"]["content"]
        .as_str()
        .unwrap_or("")
        .to_string())
}

pub async fn list_models(url: &str) -> Result<Vec<String>, AiError> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/api/tags", url))
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    let models = body["models"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    Ok(models)
}
