//! Экспорт проекта в JSON / текст / Markdown.

use crate::models::Project;

#[tauri::command]
pub async fn export_project(
    project: Project,
    format: String,
    path: String,
) -> Result<(), String> {
    // TODO: реализовать — этап 9 в PROMPT_PLAN.md
    // Использовать tauri_plugin_dialog для выбора файла
    Err("Не реализовано".to_string())
}
