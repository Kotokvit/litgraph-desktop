//! Управление проектами: список/загрузка/сохранение/удаление.
//!
//! Проекты хранятся в ~/.local/share/litgraph/projects/*.litgraph

use crate::models::{Project, ProjectMeta};
use crate::storage;

#[tauri::command]
pub async fn list_projects() -> Result<Vec<ProjectMeta>, String> {
    storage::list_projects().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn load_project(id: String) -> Result<Project, String> {
    storage::load_project(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_project(id: String, project: Project) -> Result<(), String> {
    storage::save_project(&id, &project).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_project(id: String) -> Result<(), String> {
    storage::delete_project(&id).map_err(|e| e.to_string())
}
