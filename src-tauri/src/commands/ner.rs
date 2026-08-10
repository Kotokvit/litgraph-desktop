//! NER-извлечение сущностей через Python (Natasha + pymorphy3).
//!
//! v2.0 (commit после 6b78790): extract_entities переключён с v1 (spaCy +
//! ~100-словный blacklist) на v2 (Natasha Slovnet NER + pymorphy3
//! морфология). Точность детекции ФИО +60-133%. JSON-контракт сохранён.
//!
//! v1 (ner_extract.py) остаётся как библиотека для SVO и POLER pipelines
//! (analyze_characters, extract_svo) — они импортируют из v1 `NLP`,
//! `get_proper_lemma`, `FALSE_POSITIVE_NOUNS`. Миграция в v2 — Phase 1B.
//!
//! V1 (сохранено для истории): использовал временный файл для передачи
//! текста (вместо stdin) — решает проблему "Канал оборвано (os error 32)"
//! на больших текстах (>100k символов). Pipe buffer переполняется и
//! write_all блокируется навсегда, если Python не успевает читать.
//!
//! Установка:
//!   pip install natasha pymorphy3

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
pub(crate) fn find_python() -> String {
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
pub(crate) fn run_python_with_text_file(
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
                     Для установки NER v2:\n\
                     1. Создайте venv: python -m venv ~/.litgraph-venv\n\
                     2. Активируйте: source ~/.litgraph-venv/bin/activate\n\
                     3. Установите: pip install natasha pymorphy3",
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
///
/// v2.0: использует Natasha (Slovnet NER) + pymorphy3 вместо spaCy + blacklists.
/// Точность детекции ФИО +60-133% по сравнению с v1 (см. scripts/dev/grammar/person.py).
/// JSON-контракт сохранён (NerResult struct не изменился).
///
/// v1 (ner_extract.py) остаётся как библиотека для SVO и POLER pipelines,
/// потому что svo_extract.py и poler_entities.py импортируют из него
/// `NLP`, `get_proper_lemma`, `FALSE_POSITIVE_NOUNS` — их миграция в v2
/// это Phase 1B.
#[tauri::command]
pub async fn extract_entities(text: String) -> Result<NerResult, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    let script = include_str!("../../python/ner_extract_v2.py");
    // v2 импортирует person.py — кладём его рядом с main_script.py в temp dir.
    // person.py зависит только от natasha (внешний пакет), без внутренних импортов.
    let person_script = include_str!("../../../scripts/dev/grammar/person.py");
    let extra_files = vec![("person.py", person_script)];
    let stdout = run_python_with_text_file(script, &text, &extra_files)?;

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
    // poler_entities.py импортирует ner_extract И svo_extract — кладём все 3 файла
    let ner_script = include_str!("../../python/ner_extract.py");
    let svo_script = include_str!("../../python/svo_extract.py");
    let extra_files = vec![
        ("ner_extract.py", ner_script),
        ("svo_extract.py", svo_script),
    ];
    let stdout = run_python_with_text_file(script, &text, &extra_files)?;

    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Не удалось распарсить JSON: {}.", e))?;

    Ok(result)
}

/// Tauri команда: извлечь SVO (Subject-Verb-Object) из текста.
/// Запускает svo_extract.py который через spaCy dependency parsing
/// находит триплеты: кто -> что сделал -> с кем/чем.
#[tauri::command]
pub async fn extract_svo(text: String) -> Result<serde_json::Value, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    let script = include_str!("../../python/svo_extract.py");
    // svo_extract.py импортирует ner_extract — кладём оба файла
    let ner_script = include_str!("../../python/ner_extract.py");
    let extra_files = vec![("ner_extract.py", ner_script)];
    let stdout = run_python_with_text_file(script, &text, &extra_files)?;

    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Не удалось распарсить JSON: {}.", e))?;

    Ok(result)
}
