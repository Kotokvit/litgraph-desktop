//! AI-функции: помощник, дописать главу, анализ сюжета.
//! См. docs/PROMPT_PLAN.md раздел 4.

use crate::ai::{self, AiProvider, ChatMessage};
use crate::models::Project;

#[tauri::command]
pub async fn ai_assistant(
    project: Project,
    message: String,
    history: Vec<ChatMessage>,
    selected_node_id: Option<String>,
    provider: AiProvider,
) -> Result<String, String> {
    let (system, user) = crate::ai::prompts::build_assistant_prompt(&project, &message, &selected_node_id);
    let messages = crate::ai::prompts::build_messages(&system, &user, &history);
    ai::chat(&provider, messages).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_continue_chapter(
    project: Project,
    from_chapter_id: Option<String>,
    custom_prompt: Option<String>,
    provider: AiProvider,
) -> Result<String, String> {
    let (system, user) =
        crate::ai::prompts::build_continue_chapter_prompt(&project, &from_chapter_id, &custom_prompt);
    if system == "Нет глав" {
        return Err("В проекте нет глав".to_string());
    }
    let messages = vec![
        ChatMessage { role: "system".to_string(), content: system },
        ChatMessage { role: "user".to_string(), content: user },
    ];
    ai::chat(&provider, messages).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_analyze_plot(
    project: Project,
    focus: String,
    provider: AiProvider,
) -> Result<String, String> {
    let (system, user) = crate::ai::prompts::build_analyze_plot_prompt(&project, &focus);
    let messages = vec![
        ChatMessage { role: "system".to_string(), content: system },
        ChatMessage { role: "user".to_string(), content: user },
    ];
    ai::chat(&provider, messages).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_test_connection(provider: AiProvider) -> Result<bool, String> {
    ai::test_connection(&provider).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn ai_list_ollama_models(url: String) -> Result<Vec<String>, String> {
    ai::list_ollama_models(&url).await.map_err(|e| e.to_string())
}
