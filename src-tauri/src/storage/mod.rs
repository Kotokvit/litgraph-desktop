//! Работа с файловой системой: загрузка/сохранение проектов.
//! См. docs/PROMPT_PLAN.md раздел 3.2.

use crate::models::{Project, ProjectMeta};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("Проект не найден: {0}")]
    NotFound(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Получить путь к директории проектов
fn projects_dir() -> Result<PathBuf, StorageError> {
    let home = dirs::home_dir().ok_or_else(|| StorageError::NotFound("home dir".into()))?;
    Ok(home.join(".local/share/litgraph/projects"))
}

pub fn list_projects() -> Result<Vec<ProjectMeta>, StorageError> {
    // TODO: реализовать — этап 5 в PROMPT_PLAN.md
    Ok(Vec::new())
}

pub fn load_project(id: &str) -> Result<Project, StorageError> {
    // TODO: реализовать — этап 5 в PROMPT_PLAN.md
    Err(StorageError::NotFound(id.to_string()))
}

pub fn save_project(id: &str, project: &Project) -> Result<(), StorageError> {
    // TODO: реализовать — этап 5 в PROMPT_PLAN.md
    Ok(())
}

pub fn delete_project(id: &str) -> Result<(), StorageError> {
    // TODO: реализовать — этап 5 в PROMPT_PLAN.md
    Ok(())
}
