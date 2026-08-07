//! Версионирование глав: сохранение/восстановление/удаление версий fullText.
//!
//! См. docs/PROMPT_PLAN.md раздел 3.3.

use crate::models::ChapterVersion;

#[tauri::command]
pub async fn save_version(
    project_id: String,
    node_id: String,
    label: Option<String>,
    source: Option<String>,
) -> Result<ChapterVersion, String> {
    // TODO: реализовать — этап 6 в PROMPT_PLAN.md
    Err("Не реализовано".to_string())
}

#[tauri::command]
pub async fn restore_version(
    project_id: String,
    node_id: String,
    version_id: String,
) -> Result<(), String> {
    // TODO: реализовать — этап 6 в PROMPT_PLAN.md
    Err("Не реализовано".to_string())
}

#[tauri::command]
pub async fn delete_version(
    project_id: String,
    node_id: String,
    version_id: String,
) -> Result<(), String> {
    // TODO: реализовать — этап 6 в PROMPT_PLAN.md
    Err("Не реализовано".to_string())
}

#[tauri::command]
pub async fn list_versions(project_id: String, node_id: String) -> Result<Vec<ChapterVersion>, String> {
    // TODO: реализовать — этап 6 в PROMPT_PLAN.md
    Ok(Vec::new())
}
