//! AI-функции: помощник, дописать главу, анализ сюжета.
//!
//! См. docs/PROMPT_PLAN.md раздел 4.
//! Промпты перенести из прототипа (src/app/api/ai/*/route.ts) — не менять.

use crate::ai::{AiProvider, ChatMessage};
use crate::models::Project;

#[tauri::command]
pub async fn ai_assistant(
    project: Project,
    message: String,
    history: Vec<ChatMessage>,
    selected_node_id: Option<String>,
    provider: AiProvider,
) -> Result<String, String> {
    // TODO: реализовать — этап 7 в PROMPT_PLAN.md
    // Промпт: src/app/api/ai/assistant/route.ts (216 строк TS)
    Err("Не реализовано".to_string())
}

#[tauri::command]
pub async fn ai_continue_chapter(
    project: Project,
    from_chapter_id: Option<String>,
    custom_prompt: Option<String>,
    provider: AiProvider,
) -> Result<String, String> {
    // TODO: реализовать — этап 7 в PROMPT_PLAN.md
    // Промпт: src/app/api/ai/continue-chapter/route.ts (177 строк TS)
    Err("Не реализовано".to_string())
}

#[tauri::command]
pub async fn ai_analyze_plot(
    project: Project,
    focus: String,
    provider: AiProvider,
) -> Result<String, String> {
    // TODO: реализовать — этап 7 в PROMPT_PLAN.md
    // Промпт: src/app/api/ai/analyze-plot/route.ts (210 строк TS)
    Err("Не реализовано".to_string())
}

#[tauri::command]
pub async fn ai_test_connection(provider: AiProvider) -> Result<bool, String> {
    // TODO: реализовать — этап 7 в PROMPT_PLAN.md
    crate::ai::test_connection(&provider).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_list_ollama_models(url: String) -> Result<Vec<String>, String> {
    // TODO: реализовать — этап 7 в PROMPT_PLAN.md
    crate::ai::list_ollama_models(&url).await.map_err(|e| e.to_string())
}
