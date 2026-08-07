//! NER-извлечение сущностей через Python (spaCy + pymorphy3).
//!
//! Вызывает python3 src-tauri/python/ner_extract.py, передавая текст через stdin,
//! и парсит JSON-ответ. Скрипт должен быть установлен вместе с моделями:
//!   pip install spacy pymorphy3
//!   python -m spacy download ru_core_news_sm

use serde::{Deserialize, Serialize};
use std::process::Command;
use std::process::Stdio;
use std::io::Write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityMention {
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub sentence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub lemma: String,
    pub label: String, // PER, LOC, GPE, ORG
    pub count: usize,
    pub forms: Vec<String>,
    #[serde(rename = "firstMention")]
    pub first_mention: usize,
    pub mentions: Vec<EntityMention>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NerStats {
    pub total: usize,
    pub persons: usize,
    pub locations: usize,
    pub organizations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NerResult {
    pub entities: Vec<Entity>,
    pub stats: NerStats,
    pub model: String,
    pub version: String,
    pub truncated: bool,
    #[serde(rename = "textLength")]
    pub text_length: usize,
    #[serde(rename = "processedLength")]
    pub processed_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NerError {
    pub error: String,
}

/// Tauri команда: извлечь сущности из текста.
///
/// В веб-превью (без Tauri) эта команда не вызывается — фронтенд должен
/// использовать /api/ner-extract endpoint. Но в Tauri desktop вызывается
/// именно она.
#[tauri::command]
pub async fn extract_entities(text: String) -> Result<NerResult, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    // Путь к Python скрипту (встроен в бинарник через include_str!)
    let script = include_str!("../python/ner_extract.py");

    // Запускаем python3 с скриптом
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Не удалось запустить python3: {}. Установите Python 3 и spaCy: pip install spacy pymorphy3 && python -m spacy download ru_core_news_sm", e))?;

    // Передаём текст через stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| format!("Ошибка записи в stdin: {}", e))?;
        // stdin автоматически закрывается при drop
    }

    // Ждём завершения и читаем stdout/stderr
    let output = child
        .wait_with_output()
        .map_err(|e| format!("Ошибка ожидания python3: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Python скрипт завершился с ошибкой: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Парсим JSON
    let result: NerResult = serde_json::from_str(&stdout)
        .map_err(|e| {
            format!(
                "Не удалось распарсить JSON от Python скрипта: {}. Первые 500 символов вывода: {}",
                e,
                &stdout[..stdout.len().min(500)]
            )
        })?;

    Ok(result)
}
