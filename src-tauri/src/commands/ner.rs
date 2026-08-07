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

/// Запустить Python скрипт, передав текст через временный файл.
///
/// V3: скрипт и его зависимости (ner_extract.py) записываются во временную
/// директорию /tmp/litgraph_scripts_PID/, чтобы import ner_extract работал.
/// Раньше было python -c "script" — но тогда __file__ недоступен и import
/// ner_extract падал с ModuleNotFoundError.
fn run_python_with_text_file(
    script: &str,
    text: &str,
    extra_files: &[(&str, &str)], // (filename, content) — доп. файлы рядом
) -> Result<String, String> {
    let python_cmd = find_python();

    // Создаём временную директорию для скриптов
    let temp_dir = std::env::temp_dir();
    let pid = std::process::id();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let script_dir = temp_dir.join(format!("litgraph_scripts_{}_{}", pid, timestamp));
    fs::create_dir_all(&script_dir)
        .map_err(|e| format!("Не удалось создать temp директорию: {}", e))?;

    // Записываем главный скрипт
    let main_script_path = script_dir.join("main_script.py");
    fs::write(&main_script_path, script)
        .map_err(|e| format!("Не удалось записать скрипт: {}", e))?;

    // Записываем дополнительные файлы (ner_extract.py и т.д.)
    for (filename, content) in extra_files {
        let path = script_dir.join(filename);
        fs::write(&path, content)
            .map_err(|e| format!("Не удалось записать {}: {}", filename, e))?;
    }

    // Записываем текст в файл
    let text_file = script_dir.join("input_text.txt");
    fs::write(&text_file, text)
        .map_err(|e| format!("Не удалось записать текст: {}", e))?;

    let result = (|| {
        let output = Command::new(&python_cmd)
            .arg(&main_script_path)
            .arg(&text_file)
            .stdin(Stdio::null())
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

    // Удаляем временную директорию со всеми файлами
    let _ = fs::remove_dir_all(&script_dir);

    result
}

/// Tauri команда: извлечь сущности из текста.
#[tauri::command]
pub async fn extract_entities(text: String) -> Result<NerResult, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    let script = include_str!("../../python/ner_extract.py");
    // ner_extract.py не имеет зависимостей от других наших скриптов
    let stdout = run_python_with_text_file(script, &text, &[])?;

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
    // poler_entities.py импортирует ner_extract, поэтому кладём оба файла рядом
    let ner_script = include_str!("../../python/ner_extract.py");
    let extra_files = vec![("ner_extract.py", ner_script)];
    let stdout = run_python_with_text_file(script, &text, &extra_files)?;

    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Не удалось распарсить JSON: {}.", e))?;

    Ok(result)
}
