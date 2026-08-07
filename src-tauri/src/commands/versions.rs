//! Версионирование глав: сохранение/восстановление/удаление версий fullText.
//! См. docs/PROMPT_PLAN.md раздел 3.3.
//!
//! Версии хранятся прямо в data.versions ноды (как в прототипе).
//! Эти команды работают с проектом через storage.

use crate::models::ChapterVersion;
use crate::storage;
use chrono::Utc;
use uuid::Uuid;

fn new_version_id() -> String {
    format!("v_{}_{}", Utc::now().timestamp_millis(), &Uuid::new_v4().to_string()[..8])
}

#[tauri::command]
pub async fn save_version(
    project_id: String,
    node_id: String,
    label: Option<String>,
    source: Option<String>,
) -> Result<ChapterVersion, String> {
    let mut project = storage::load_project(&project_id).map_err(|e| e.to_string())?;

    let node = project
        .nodes
        .iter_mut()
        .find(|n| n.id == node_id)
        .ok_or_else(|| format!("Нода не найдена: {}", node_id))?;

    let full_text = node.data.full_text.clone().unwrap_or_default();
    if full_text.trim().is_empty() {
        return Err("Нельзя сохранить пустую версию".to_string());
    }

    let word_count = full_text.split_whitespace().count();
    let version = ChapterVersion {
        id: new_version_id(),
        timestamp: Utc::now().timestamp_millis() as u64,
        full_text: full_text.clone(),
        word_count,
        label: Some(label.unwrap_or_else(|| {
            format!("Версия от {}", Utc::now().format("%Y-%m-%d %H:%M:%S"))
        })),
        source: Some(source.unwrap_or_else(|| "manual".to_string())),
    };

    // Добавляем версию в начало списка, максимум 50
    let versions = node.data.versions.get_or_insert_with(Vec::new);
    versions.insert(0, version.clone());
    if versions.len() > 50 {
        versions.truncate(50);
    }

    storage::save_project(&project_id, &project).map_err(|e| e.to_string())?;
    Ok(version)
}

#[tauri::command]
pub async fn restore_version(
    project_id: String,
    node_id: String,
    version_id: String,
) -> Result<(), String> {
    let mut project = storage::load_project(&project_id).map_err(|e| e.to_string())?;

    let node = project
        .nodes
        .iter_mut()
        .find(|n| n.id == node_id)
        .ok_or_else(|| format!("Нода не найдена: {}", node_id))?;

    let versions = node.data.versions.get_or_insert_with(Vec::new);
    let version = versions
        .iter()
        .find(|v| v.id == version_id)
        .cloned()
        .ok_or_else(|| format!("Версия не найдена: {}", version_id))?;

    // Сначала сохраним текущее состояние как версию (перед откатом)
    let current_text = node.data.full_text.clone().unwrap_or_default();
    if !current_text.trim().is_empty() {
        let backup = ChapterVersion {
            id: new_version_id(),
            timestamp: Utc::now().timestamp_millis() as u64,
            full_text: current_text,
            word_count: 0, // посчитаем ниже
            label: Some(format!(
                "Перед откатом к версии от {}",
                chrono::DateTime::from_timestamp(version.timestamp as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "unknown".to_string())
            )),
            source: Some("restore".to_string()),
        };
        let mut backup = backup;
        backup.word_count = backup.full_text.split_whitespace().count();
        versions.insert(0, backup);
        if versions.len() > 50 {
            versions.truncate(50);
        }
    }

    // Применяем выбранную версию
    let body_preview: String = version.full_text.split_whitespace().collect::<Vec<_>>().join(" ");
    let body = if body_preview.len() > 400 {
        format!("{}…", &body_preview[..400])
    } else {
        body_preview
    };
    node.data.full_text = Some(version.full_text.clone());
    node.data.body = body;

    // Обновим wordCount в meta
    if let Some(meta) = node.data.meta.as_mut() {
        if let Some(obj) = meta.as_object_mut() {
            obj.insert(
                "wordCount".to_string(),
                serde_json::Value::Number(version.word_count.into()),
            );
        }
    }

    storage::save_project(&project_id, &project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn delete_version(
    project_id: String,
    node_id: String,
    version_id: String,
) -> Result<(), String> {
    let mut project = storage::load_project(&project_id).map_err(|e| e.to_string())?;

    let node = project
        .nodes
        .iter_mut()
        .find(|n| n.id == node_id)
        .ok_or_else(|| format!("Нода не найдена: {}", node_id))?;

    if let Some(versions) = node.data.versions.as_mut() {
        versions.retain(|v| v.id != version_id);
    }

    storage::save_project(&project_id, &project).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn list_versions(
    project_id: String,
    node_id: String,
) -> Result<Vec<ChapterVersion>, String> {
    let project = storage::load_project(&project_id).map_err(|e| e.to_string())?;

    let node = project
        .nodes
        .iter()
        .find(|n| n.id == node_id)
        .ok_or_else(|| format!("Нода не найдена: {}", node_id))?;

    Ok(node.data.versions.clone().unwrap_or_default())
}
