//! NER-извлечение сущностей через Python (Natasha + pymorphy3).
//!
//! v2.0 / Phase 1B: ВСЕ три команды (extract_entities, analyze_characters,
//! extract_svo) теперь используют v2 (Natasha Slovnet NER + pymorphy3).
//! v1 ner_extract.py полностью удалён — v2 экспортирует совместимые
//! символы (NLP, extract_entities, get_proper_lemma, FALSE_POSITIVE_NOUNS)
//! через spaCy-compat shim, что позволяет poler_entities.py и svo_extract.py
//! работать без изменений.
//!
//! ВАЖНО: svo_extract.py загружает spaCy `NLP` напрямую через `spacy.load()`
//! для dependency parsing (token.children, token.dep_, token.head) — это
//! осталось на spaCy, потому что Natasha не даёт эквивалентного API.
//! Из ner_extract.py SVO берёт только extract_entities/get_proper_lemma/
//! FALSE_POSITIVE_NOUNS — все три экспортируются из v2-shim.
//!
//! v2.1 / Phase 2 Step 1: extract_entities получает **Rust fast path**.
//! Сначала текст прогоняется через `crate::parser::characters::detect()`
//! (Rust-native, 3-signal detection). Если ВСЕ обнаруженные персонажи —
//! single-token с confidence >= 0.7 (см. матрицу Phase 2), Python не
//! запускается вообще: NerResult конструируется в Rust, model="rust-fast-path",
//! version="2.1". Это убирает 0.8–1.2s Python warm-up для простых корпусов.
//!
//! Fast path policy (детерминированная, см. `ParsedCharacter::confidence_from_signals`):
//!   - 3 сигнала (cap + speech + direct) → 1.0 → eligible
//!   - 2 сигнала single-token → 0.7 → eligible
//!   - 2 сигнала multi-token  → 0.5 → NOT eligible (Python для FIO)
//!   - 1 сигнал (только cap)  → 0.3 → NOT eligible (Python для validation)
//!
//! Все остальные команды (analyze_characters, extract_svo) оставлены на
//! Python v2 — Phase 2 surgical scope только для extract_entities.
//!
//! V1 (история): использовал временный файл для передачи текста (вместо
//! stdin) — решает проблему "Канал оборвано (os error 32)" на больших
//! текстах (>100k символов).
//!
//! Установка:
//!   pip install natasha pymorphy3 spacy
//!   python -m spacy download ru_core_news_sm    # только для extract_svo

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

/// v2.1 / Phase 2 Step 1: Rust fast path для extract_entities.
///
/// Прогоняет текст через `crate::parser::characters::detect()` (Rust-native,
/// 3-signal detection). Если ВСЕ обнаруженные characters — single-token с
/// confidence >= 0.7 (см. матрицу Phase 2), конструирует `NerResult` в Rust
/// и возвращает `Some(_)`. Иначе возвращает `None` — вызывающий код должен
/// запустить Python fallback.
///
/// **Provenance**: результат помечается `model = "rust-fast-path"`,
/// `version = "2.1"`, `chunks_processed = None`. Это позволяет UI/логам
/// различать ветки (см. Phase 2 merge policy matrix).
///
/// **Что НЕ делает fast path** (намеренно):
///   - Не возвращает `mentions` (Rust-парсер не трекает позиции) — пустой Vec
///   - Не возвращает `first_mention` — 0 (нет данных)
///   - Не возвращает locations/organizations — только PER
///   - Не разрешает multi-token ФИО — для этого есть Python
///
/// **Контракт стабильности**: возвращаемый `NerResult` проходит ту же
/// десериализацию, что и Python-результат. UI не должен различать ветки
/// по структуре JSON — только по полю `model`.
///
/// # Returns
///   - `Some(NerResult)` — fast path сработал, Python не нужен
///   - `None` — нужен Python fallback (multi-token, low-confidence, или
///     обнаружен хотя бы один concept/org, требующий Python validation)
fn rust_fast_path_entities(text: &str) -> Option<NerResult> {
    use crate::parser::characters::{detect, EntityType};

    let parsed = detect(text);

    // Если парсер вообще ничего не нашёл — отдаём empty NerResult из Rust
    // (это тоже валидный fast path: пустой текст не должен платить Python).
    if parsed.is_empty() {
        return Some(NerResult {
            entities: vec![],
            stats: NerStats {
                total: 0,
                persons: 0,
                locations: 0,
                organizations: 0,
            },
            model: "rust-fast-path".to_string(),
            version: "2.1".to_string(),
            truncated: false,
            text_length: text.len(),
            processed_length: text.len(),
            chunks_processed: None,
        });
    }

    // Eligibility check: ВСЕ Characters должны быть single-token И confidence >= 0.7.
    // Concepts/Organizations тоже форсируют fallback — Rust-парсер не умеет
    // их валидировать морфологически, Natasha точнее.
    let all_eligible = parsed.iter().all(|c| {
        if c.entity_type != EntityType::Character {
            return false; // Concept/Org → нужен Python для валидации
        }
        c.is_single_token() && c.confidence >= 0.7
    });

    if !all_eligible {
        return None;
    }

    // Строим NerResult из Rust-данных. Берём ТОЛЬКО Characters.
    let entities: Vec<Entity> = parsed
        .iter()
        .filter(|c| c.entity_type == EntityType::Character)
        .map(|c| Entity {
            lemma: c.name.clone(),
            label: "PER".to_string(),
            count: c.count,
            forms: c.aliases.clone(),
            first_mention: 0, // Rust-парсер не трекает позиции — Phase 3 добавит
            mentions: vec![], // Rust-парсер не трекает mentions — Phase 3 добавит
        })
        .collect();

    let persons = entities.len();
    let stats = NerStats {
        total: persons,
        persons,
        locations: 0,
        organizations: 0,
    };

    Some(NerResult {
        entities,
        stats,
        model: "rust-fast-path".to_string(),
        version: "2.1".to_string(),
        truncated: false,
        text_length: text.len(),
        processed_length: text.len(),
        chunks_processed: None,
    })
}

/// Tauri команда: извлечь сущности из текста.
///
/// v2.1 / Phase 2: двухуровневая диспетчеризация:
///   1. **Fast path (Rust)** — `rust_fast_path_entities()` для simple корпусов
///      (single-token characters с confidence >= 0.7). Latency: <1мс.
///   2. **Fallback (Python v2)** — Natasha Slovnet NER + pymorphy3 для
///      complex случаев (multi-token ФИО, ambiguous, concept validation).
///      Latency: 0.8–1.2s cold start, ~200мс warm.
///
/// JSON-контракт `NerResult` сохранён (structure не изменилась). UI различает
/// ветки только по полю `model`:
///   - `"rust-fast-path"` (version "2.1") — fast path сработал
///   - `"natasha-slovnet+pymorphy3"` (version "2.0") — Python fallback
///
/// Phase 1B: v1 ner_extract.py полностью удалён. v2 экспортирует совместимые
/// символы (NLP, extract_entities, get_proper_lemma, FALSE_POSITIVE_NOUNS)
/// через spaCy-compat shim, что позволяет poler_entities.py и svo_extract.py
/// работать без изменений.
#[tauri::command]
pub async fn extract_entities(text: String) -> Result<NerResult, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    // === Phase 2 Step 1: Rust fast path ===
    // Пробуем Rust-native detection. Если все characters — single-token с
    // confidence >= 0.7, возвращаем результат без Python spawn.
    if let Some(rust_result) = rust_fast_path_entities(&text) {
        return Ok(rust_result);
    }

    // === Python fallback ===
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
/// 1. Извлекает персонажей через v2 NER (Natasha)
/// 2. Строит граф co-occurrence
/// 3. Запускает POLER-динамику
/// 4. Возвращает кластеры персонажей
///
/// Phase 1B: вместо v1 ner_extract.py используем v2 (ner_extract_v2.py),
/// который копируется в temp dir под именем 'ner_extract.py' (контракт
/// имени модуля сохранён для poler_entities.py).
/// v2 экспортирует NLP, extract_entities через spaCy-compat shim.
#[tauri::command]
pub async fn analyze_characters(text: String) -> Result<serde_json::Value, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    let script = include_str!("../../python/poler_entities.py");
    // v2 вместо v1: копируем ner_extract_v2.py под именем ner_extract.py
    // (poler_entities.py делает `from ner_extract import extract_entities, NLP`)
    let ner_script = include_str!("../../python/ner_extract_v2.py");
    let svo_script = include_str!("../../python/svo_extract.py");
    let person_script = include_str!("../../../scripts/dev/grammar/person.py");
    let extra_files = vec![
        ("ner_extract.py", ner_script),  // v2 под именем v1
        ("svo_extract.py", svo_script),
        ("person.py", person_script),    // v2 зависит от person.py
    ];
    let stdout = run_python_with_text_file(script, &text, &extra_files)?;

    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Не удалось распарсить JSON: {}.", e))?;

    Ok(result)
}

/// Tauri команда: извлечь SVO (Subject-Verb-Object) из текста.
/// Запускает svo_extract.py который через spaCy dependency parsing
/// находит триплеты: кто -> что сделал -> с кем/чем.
///
/// Phase 1B: NLP (spaCy) остаётся для dependency parsing — Natasha не даёт
/// эквивалентного API (token.children, token.dep_, token.head).
/// Но extract_entities, get_proper_lemma, FALSE_POSITIVE_NOUNS теперь
/// берутся из v2-shim (копируется под именем ner_extract.py).
#[tauri::command]
pub async fn extract_svo(text: String) -> Result<serde_json::Value, String> {
    if text.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    let script = include_str!("../../python/svo_extract.py");
    // v2 вместо v1: копируем ner_extract_v2.py под именем ner_extract.py
    let ner_script = include_str!("../../python/ner_extract_v2.py");
    let person_script = include_str!("../../../scripts/dev/grammar/person.py");
    let extra_files = vec![
        ("ner_extract.py", ner_script),  // v2 под именем v1
        ("person.py", person_script),    // v2 зависит от person.py
    ];
    let stdout = run_python_with_text_file(script, &text, &extra_files)?;

    let result: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("Не удалось распарсить JSON: {}.", e))?;

    Ok(result)
}

// ============================================================================
// v2.1 / Phase 2 Step 1: Unit tests for rust_fast_path_entities()
// ============================================================================
//
// Эти тесты — формальная верификация Phase 2 dispatch policy:
//
//   1. Single-token name + speech verb → fast path срабатывает, model="rust-fast-path"
//   2. Multi-token name → fast path НЕ срабатывает (None → Python fallback)
//   3. Только concept (Бездна) → fast path НЕ срабатывает (None → Python для validation)
//   4. Пустой текст → fast path возвращает пустой NerResult (без Python spawn)
//   5. Mixed: single + multi-token → fast path НЕ срабатывает (хотя бы один multi → fallback)
//
// Без этих тестов «dispatch policy» остаётся только документацией.
#[cfg(test)]
mod phase2_fast_path_tests {
    use super::*;

    /// Single-token name + speech verb → fast path eligible.
    /// Latency: <1мс (без Python spawn).
    /// model="rust-fast-path", version="2.1", entities содержат персонажа.
    #[test]
    fn test_fast_path_single_token_with_speech_verb() {
        // "Борис сказал слово" → 2 сигнала (cap + speech), single-token → 0.7
        let text = "Борис сказал слово. Борис промолчал и ушёл в ночь.";
        let result = rust_fast_path_entities(text);

        assert!(result.is_some(), "Fast path должен сработать для single-token + speech verb");
        let ner = result.unwrap();

        assert_eq!(ner.model, "rust-fast-path");
        assert_eq!(ner.version, "2.1");
        assert_eq!(ner.chunks_processed, None);
        assert!(!ner.truncated);
        assert_eq!(ner.text_length, text.len());
        assert_eq!(ner.processed_length, text.len());

        // Stats: только PER (locations/orgs = 0)
        assert!(ner.stats.persons >= 1, "Должен найти хотя бы одного персонажа");
        assert_eq!(ner.stats.locations, 0);
        assert_eq!(ner.stats.organizations, 0);

        // Entity проверка: Борис с label=PER
        let boris = ner.entities.iter().find(|e| e.lemma == "Борис");
        assert!(boris.is_some(), "Борис должен быть в entities");
        let boris = boris.unwrap();
        assert_eq!(boris.label, "PER");
        assert!(boris.count >= 1);
    }

    /// Single-token name + speech verb + direct address → fast path eligible.
    /// Это уже 3 сигнала → confidence 1.0.
    #[test]
    fn test_fast_path_three_signals_confidence_1() {
        let text = "Архип сказал привет. — Архип, иди сюда!";
        let result = rust_fast_path_entities(text);

        assert!(result.is_some(), "3-signal single-token → fast path eligible");
        let ner = result.unwrap();
        assert_eq!(ner.model, "rust-fast-path");
        assert!(ner.stats.persons >= 1);
    }

    /// Multi-token name (Иван Петров) → fast path НЕ срабатывает,
    /// потому что нужен Python для FIO resolution.
    #[test]
    fn test_fast_path_multi_token_returns_none() {
        // Multi-token: "Иван Петров" → is_single_token() = false → confidence 0.5
        // 0.5 < 0.7 → not eligible → None
        let text = "Иван Петров сказал слово. Иван Петров промолчал.";
        let result = rust_fast_path_entities(text);

        assert!(result.is_none(),
            "Multi-token name → fast path НЕ должен срабатывать (нужен Python)");
    }

    /// Concept (Бездна) в тексте → fast path НЕ срабатывает,
    /// потому что Rust не умеет валидировать concept vs character морфологически.
    #[test]
    fn test_fast_path_concept_returns_none() {
        // «Бездна» многократно, но без speech/direct → Concept после reclassify
        let text = "Бездна смотрела. Бездна звала. Бездна ждала. \
                    Бездна дышала. Бездна молчала. Бездна пела. \
                    Бездна раскрывалась. Бездна закрывалась. \
                    Бездна улыбалась. Бездна хмурилась.";
        let result = rust_fast_path_entities(text);

        assert!(result.is_none(),
            "Concept entity → fast path НЕ должен срабатывать (нужен Python для validation)");
    }

    /// Пустой текст → fast path возвращает empty NerResult (без Python spawn).
    /// Это валидный кейс: пустой текст не должен платить 0.8s Python warm-up.
    #[test]
    fn test_fast_path_empty_text_returns_empty_result() {
        let text = "   \n\t  "; // только whitespace
        let result = rust_fast_path_entities(text);

        // Note: detect() может вернуть пустой vec для whitespace-only text
        // → fast path возвращает Some(empty NerResult)
        assert!(result.is_some(), "Empty/whitespace text → fast path возвращает empty result");
        let ner = result.unwrap();
        assert_eq!(ner.entities.len(), 0);
        assert_eq!(ner.stats.total, 0);
        assert_eq!(ner.model, "rust-fast-path");
    }

    /// Mixed: single-token + multi-token в одном тексте → fast path НЕ срабатывает.
    /// Достаточно ОДНОГО multi-token, чтобы форсировать Python fallback для всего текста.
    #[test]
    fn test_fast_path_mixed_single_and_multi_returns_none() {
        let text = "Анна сказала. Борис промолчал. Иван Петров кивнул.";
        let result = rust_fast_path_entities(text);

        assert!(result.is_none(),
            "Хотя бы один multi-token → fast path НЕ должен срабатывать");
    }

    /// JSON serialization contract: fast path результат должен сериализоваться
    /// в тот же JSON shape, что и Python-результат.
    #[test]
    fn test_fast_path_json_contract() {
        let text = "Борис сказал слово. Борис промолчал.";
        let result = rust_fast_path_entities(text).expect("fast path должен сработать");

        let json = serde_json::to_string(&result).expect("сериализация должна работать");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("десериализация должна работать");

        // Проверяем обязательные поля NerResult
        assert!(parsed.get("entities").is_some(), "entities field");
        assert!(parsed.get("stats").is_some(), "stats field");
        assert_eq!(parsed.get("model").and_then(|v| v.as_str()), Some("rust-fast-path"));
        assert_eq!(parsed.get("version").and_then(|v| v.as_str()), Some("2.1"));
        assert_eq!(parsed.get("truncated").and_then(|v| v.as_bool()), Some(false));
        assert!(parsed.get("textLength").is_some(), "textLength field (camelCase)");
        assert!(parsed.get("processedLength").is_some(), "processedLength field (camelCase)");
        // chunksProcessed — Option, может быть null в JSON
        assert!(parsed.get("chunksProcessed").is_some(),
            "chunksProcessed field должен присутствовать (даже если null)");
    }
}
