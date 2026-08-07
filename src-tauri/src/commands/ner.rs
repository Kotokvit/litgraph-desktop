//! NER-извлечение сущностей через Python (spaCy + pymorphy3).
//!
//! V2: использует временный файл для передачи текста (вместо stdin) —
//! решает проблему "Канал оборвано (os error 32)" на больших текстах
//! (>100k символов). Pipe buffer переполняется и write_all блокируется
//! навсегда, если Python не успевает читать. Временный файл — надёжнее.
//!
//! Установка:
//!   pip install spacy pymorphy3 numpy scipy scikit-learn
//!   python -m spacy download ru_core_news_sm

use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::process::Command;
use std::process::Stdio;

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
    pub label: String,
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
    #[serde(rename = "chunksProcessed", default)]
    pub chunks_processed: Option<usize>,
}

/// Найти Python интерпретатор. Приоритет:
/// 1. ~/.litgraph-venv/bin/python (если пользователь создал venv)
/// 2. $LITGRAPH_PYTHON (ручное переопределение)
/// 3. Системный python3
fn find_python() -> String {
    let home = dirs::home_dir();
    let candidates: Vec<String> = [
        home.as_ref().map(|h| h.join(".litgraph-venv/bin/python").to_string_lossy().to_string()),
        std::env::var("LITGRAPH_PYTHON").ok(),
    ]
    .into_iter()
    .flatten()
    .collect();

    for candidate in &candidates {
        if std::path::Path::new(candidate).exists() {
            return candidate.clone();
        }
    }
    "python3".to_string()
}

/// Создать временный файл с текстом и вернуть его путь.
fn write_text_to_temp_file(text: &str) -> Result<std::path::PathBuf, String> {
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let path = temp_dir.join(format!("litgraph_text_{}_{}.txt", pid, timestamp));

    let mut file = fs::File::create(&path)
        .map_err(|e| format!("Не удалось создать временный файл {:?}: {}", path, e))?;
    file.write_all(text.as_bytes())
        .map_err(|e| format!("Не удалось записать текст во временный файл: {}", e))?;
    Ok(path)
}

/// Запустить Python скрипт, передав текст через временный файл.
///
/// V1 передавала через stdin, но на текстах >100k символов pipe buffer
/// переполнялся и write_all блокировался → "Канал оборвано (os error 32)".
///
/// V2: пишем текст в /tmp/litgraph_text_PID_TIMESTAMP.txt, передаём путь
/// через argv как единственный позиционный аргумент. Python читает файл.
fn run_python_with_text_file(script: &str, text: &str) -> Result<String, String> {
    let python_cmd = find_python();
    let text_file = write_text_to_temp_file(text)?;

    let result = (|| {
        // Запускаем python с скриптом и путём к файлу
        let output = Command::new(&python_cmd)
            .arg("-c")
            .arg(script)
            .arg(&text_file)
            .stdin(Stdio::null()) // не используем stdin
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| {
                format!(
                    "Не удалось запустить Python ({}): {}\n\n\
                     Для установки NER:\n\
                     1. Создайте venv: python -m venv ~/.litgraph-venv\n\
                     2. Активируйте: source ~/.litgraph-venv/bin/activate\n\
                     3. Установите: pip install spacy pymorphy3 numpy scipy scikit-learn\n\
                     4. Модель: python -m spacy download ru_core_news_sm",
                    python_cmd, e
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Если stderr пустой, может быть проблема с памятью (OOM killed)
            let msg = if stderr.is_empty() {
                format!(
                    "Python процесс завершился с кодом {:?} (возможно OOM killed).\n\
                     Попробуйте уменьшить размер текста или увеличить swap.",
                    output.status.code()
                )
            } else {
                format!("Python скрипт завершился с ошибкой: {}", stderr)
            };
            return Err(msg);
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(stdout)
    })();

    // Удаляем временный файл в любом случае
    let _ = fs::remove_file(&text_file);

    result
}

/// Tauri команда: извлечь сущности из текста.
#[tauri::command]
pub async fn extract_entities(text: String) -> Result<NerResult, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    let script = include_str!("../../python/ner_extract.py");
    let stdout = run_python_with_text_file(script, &text)?;

    let result: NerResult = serde_json::from_str(&stdout).map_err(|e| {
        format!(
            "Не удалось распарсить JSON от Python скрипта: {}.\n\
             Первые 500 символов вывода: {}",
            e,
            &stdout[..stdout.len().min(500)]
        )
    })?;

    Ok(result)
}

/// Tauri команда: анализ графа персонажей (NER + POLER).
///
/// Запускает poler_entities.py который:
/// 1. Извлекает персонажей через spaCy NER
/// 2. Строит граф co-occurrence
/// 3. Запускает POLER-динамику
/// 4. Возвращает кластеры персонажей
#[tauri::command]
pub async fn analyze_characters(text: String) -> Result<serde_json::Value, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    let script = include_str!("../../python/poler_entities.py");
    let stdout = run_python_with_text_file(script, &text)?;

    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Не удалось распарсить JSON: {}.", e))?;

    Ok(result)
}
