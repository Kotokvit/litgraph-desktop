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
    #[error("Home directory not found")]
    NoHomeDir,
}

/// Получить путь к директории проектов
fn projects_dir() -> Result<PathBuf, StorageError> {
    let home = dirs::home_dir().ok_or(StorageError::NoHomeDir)?;
    Ok(home.join(".local/share/litgraph/projects"))
}

fn project_path(id: &str) -> Result<PathBuf, StorageError> {
    // Защита от path traversal
    let safe_id = id.chars().filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_').collect::<String>();
    if safe_id.is_empty() {
        return Err(StorageError::NotFound(id.to_string()));
    }
    Ok(projects_dir()?.join(format!("{}.litgraph", safe_id)))
}

pub fn list_projects() -> Result<Vec<ProjectMeta>, StorageError> {
    let dir = projects_dir()?;
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
        return Ok(Vec::new());
    }

    let mut metas = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("litgraph") {
            continue;
        }
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let metadata = std::fs::metadata(&path)?;
        let modified = metadata.modified()?;
        let updated_at = modified
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        // Читаем только нужные поля (без nodes/edges — для скорости)
        let content = std::fs::read_to_string(&path)?;
        let partial: serde_json::Value = serde_json::from_str(&content)?;
        let title = partial["title"].as_str().unwrap_or(&id).to_string();
        let node_count = partial["nodes"].as_array().map(|a| a.len()).unwrap_or(0);
        let edge_count = partial["edges"].as_array().map(|a| a.len()).unwrap_or(0);

        metas.push(ProjectMeta {
            id,
            title,
            updated_at,
            size_bytes: metadata.len(),
            node_count,
            edge_count,
        });
    }

    // Сортировка по дате изменения (новые сверху)
    metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(metas)
}

pub fn load_project(id: &str) -> Result<Project, StorageError> {
    let path = project_path(id)?;
    if !path.exists() {
        return Err(StorageError::NotFound(id.to_string()));
    }
    let content = std::fs::read_to_string(&path)?;
    let project: Project = serde_json::from_str(&content)?;
    Ok(project)
}

pub fn save_project(id: &str, project: &Project) -> Result<(), StorageError> {
    let dir = projects_dir()?;
    std::fs::create_dir_all(&dir)?;

    let path = project_path(id)?;
    let content = serde_json::to_string_pretty(project)?;
    std::fs::write(&path, content)?;
    Ok(())
}

pub fn delete_project(id: &str) -> Result<(), StorageError> {
    let path = project_path(id)?;
    if !path.exists() {
        return Err(StorageError::NotFound(id.to_string()));
    }
    std::fs::remove_file(&path)?;
    Ok(())
}
