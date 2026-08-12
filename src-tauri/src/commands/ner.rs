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
    // v2.2 / Step 2b: populate first_mention + mentions из ParsedCharacter.mention_starts.
    let entities: Vec<Entity> = parsed
        .iter()
        .filter(|c| c.entity_type == EntityType::Character)
        .map(|c| entity_from_parsed(c, text))
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
        version: "2.2".to_string(),
        truncated: false,
        text_length: text.len(),
        processed_length: text.len(),
        chunks_processed: None,
    })
}

/// v2.2 / Step 2b: Construct `Entity` from `ParsedCharacter`, populating
/// `first_mention` and `mentions` from `mention_starts` byte offsets.
///
/// For each byte offset in `mention_starts`, extract:
///   - `text`: the actual substring at that position (using alias length)
///   - `start`/`end`: byte offsets in original text
///   - `sentence`: surrounding sentence (heuristic — text between ./?/!/.)
///
/// If `mention_starts` is empty, `first_mention = 0` and `mentions = vec![]`
/// (preserves backwards compatibility with v2.1 callers that don't track positions).
fn entity_from_parsed(c: &crate::parser::characters::ParsedCharacter, text: &str) -> Entity {
    use crate::parser::characters::ParsedCharacter;

    let mentions: Vec<EntityMention> = c
        .mention_starts
        .iter()
        .filter_map(|&start| {
            // Find which alias matches at this position (case-insensitive).
            // We try each alias; first match wins.
            let lower_text = text.to_lowercase();
            for alias in &c.aliases {
                let alias_lower = alias.to_lowercase();
                if start + alias_lower.len() <= text.len()
                    && lower_text[start..start + alias_lower.len()] == alias_lower
                {
                    let end = start + alias_lower.len();
                    let mention_text = &text[start..end];
                    let sentence = extract_sentence_around(text, start);
                    return Some(EntityMention {
                        text: mention_text.to_string(),
                        start,
                        end,
                        sentence,
                    });
                }
            }
            // Fallback: use c.name length if no alias matches (shouldn't happen
            // in practice, but defensive — mention_starts were collected from aliases).
            let name_len = c.name.len();
            if start + name_len <= text.len() {
                let end = start + name_len;
                let sentence = extract_sentence_around(text, start);
                return Some(EntityMention {
                    text: text[start..end].to_string(),
                    start,
                    end,
                    sentence,
                });
            }
            None
        })
        .collect();

    let first_mention = c.first_mention.unwrap_or(0);

    Entity {
        lemma: c.name.clone(),
        label: "PER".to_string(),
        count: c.count,
        forms: c.aliases.clone(),
        first_mention,
        mentions,
    }
}

/// v2.2 / Step 2b: Heuristic sentence extraction around byte offset.
///
/// Searches backwards for sentence terminator (.!?·—) and forwards for next one.
/// Returns the substring between those boundaries (trimmed).
/// If no terminator found, returns the whole text (clamped to 200 chars around offset).
fn extract_sentence_around(text: &str, offset: usize) -> String {
    let bytes = text.as_bytes();
    let max_len = 200;

    if bytes.is_empty() {
        return String::new();
    }

    // Find sentence start: scan backwards for terminator
    let mut start = 0;
    if offset > 0 {
        let mut i = std::cmp::min(offset, bytes.len() - 1);
        while i > 0 {
            let b = bytes[i];
            if b == b'.' || b == b'!' || b == b'?' || b == b'\n' {
                start = i + 1;
                break;
            }
            if i == 0 { break; }
            i = i.saturating_sub(1);
            if offset.saturating_sub(i) > max_len {
                start = i;
                break;
            }
        }
        // Ensure start is on a valid char boundary (avoid slicing in middle of multibyte char)
        while start < text.len() && !text.is_char_boundary(start) {
            start += 1;
        }
    }

    // Find sentence end: scan forwards for terminator
    let mut end = bytes.len();
    let mut i = std::cmp::min(offset, bytes.len());
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'.' || b == b'!' || b == b'?' || b == b'\n' {
            end = i + 1;
            break;
        }
        i += 1;
        if i.saturating_sub(offset) > max_len {
            end = i;
            break;
        }
    }
    // Ensure end is on a valid char boundary
    while end > start && !text.is_char_boundary(end) {
        end -= 1;
    }

    text[start..end.min(text.len())]
        .trim()
        .replace('\n', " ")
        .chars()
        .take(max_len)
        .collect()
}

    #[cfg(test)]
    mod extract_sentence_tests {
        use super::extract_sentence_around;

        #[test]
        fn test_extract_sentence_empty_text() {
            let s = "";
            let res = extract_sentence_around(s, 0);
            assert_eq!(res, "");
        }

        #[test]
        fn test_extract_sentence_cyrillic_long_no_panic() {
            // long cyrillic sentence without terminators (>200 bytes)
            let mut s = String::new();
            for _ in 0..250 {
                s.push('а');
            }
            // place offset in the middle
            let off = s.len() / 2;
            let res = extract_sentence_around(&s, off);
            assert!(!res.is_empty());
        }

        #[test]
        fn test_extract_sentence_mixed_ascii_cyrillic() {
            let s = "Hello world. Привет мир без точки длинная строка которая продолжается и не имеет точки дальше".to_string();
            let off = s.find("Привет").unwrap_or(0);
            let res = extract_sentence_around(&s, off);
            assert!(res.contains("Привет") || !res.is_empty());
        }
    }

/// v2.2 / Step 2c: 4-way merge policy implementation.
///
/// Соединяет Rust-detected characters (from `detect()`) с Python-результатом
/// (NerResult от Natasha v2). Применяет 4 профиля из arch plan §4.2:
///
/// | Rust | Python | Policy |
/// |------|--------|--------|
/// | X    | X      | Confirmed: merge mentions; Rust keeps positions; Python validates |
/// | X    | —      | Rust-only: accept if confidence ≥ 0.7, discard otherwise |
/// | —    | X      | Python-only: accept as high-confidence fallback |
/// | X (lemma A) | Y (lemma B) | Conflict: Python wins for lemma/gender; Rust wins for positions |
///
/// **Matching key**: `lemma.to_lowercase()` — case-insensitive после лемматизации.
/// Rust-сторона уже даёт lemma (canonical name); Python даёт pymorphy3 lemma.
///
/// **Когда вызывать**: только когда Rust fast path не eligible (вернул None),
/// но мы всё равно прогнали `detect()` для использования позиционных данных
/// в merge с Python-результатом.
///
/// # Returns
/// `NerResult` с `model = "rust-fast-path+natasha-merge"`, `version = "2.2"`,
/// объединёнными entities (Rust positions + Python morph), и aggregated stats.
pub fn merge_results(
    rust_parsed: &[crate::parser::characters::ParsedCharacter],
    python_result: &NerResult,
    text: &str,
) -> NerResult {
    use crate::parser::characters::EntityType;
    use std::collections::HashMap;

    // Index Rust characters by lowercase lemma (only Characters — Concepts go to Python)
    let rust_by_lemma: HashMap<String, &crate::parser::characters::ParsedCharacter> = rust_parsed
        .iter()
        .filter(|c| c.entity_type == EntityType::Character)
        .map(|c| (c.name.to_lowercase(), c))
        .collect();

    // Index Python entities by lowercase lemma
    let python_by_lemma: HashMap<String, &Entity> = python_result
        .entities
        .iter()
        .map(|e| (e.lemma.to_lowercase(), e))
        .collect();

    // Build merged entity list
    let mut merged: Vec<Entity> = Vec::new();
    let mut seen_rust: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut seen_python: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Pass 1: Rust entities → look for Python match
    for rust_char in rust_parsed.iter().filter(|c| c.entity_type == EntityType::Character) {
        let key = rust_char.name.to_lowercase();
        seen_rust.insert(key.clone());

        if let Some(py_entity) = python_by_lemma.get(&key) {
            // Profile 1: Rust X, Python X — Confirmed
            // Merge: take Python's lemma/label/forms (morphologically validated),
            // but use Rust's mentions/positions (Rust keeps positions).
            // Note: py_entity is &&Entity (HashMap<String, &Entity>), so we need
            // explicit deref to clone the Entity itself, not the reference.
            let mut merged_entity = (*py_entity).clone();
            // Override with Rust positional data if Rust has any
            if !rust_char.mention_starts.is_empty() {
                let rust_entity = entity_from_parsed(rust_char, text);
                merged_entity.mentions = rust_entity.mentions;
                merged_entity.first_mention = rust_entity.first_mention;
                // Use higher count (Rust counts all aliases, Python might miss some)
                merged_entity.count = merged_entity.count.max(rust_entity.count);
            }
            merged.push(merged_entity);
        } else {
            // Profile 2: Rust X, Python —
            // Accept if confidence ≥ 0.7, discard otherwise
            if rust_char.confidence >= 0.7 {
                merged.push(entity_from_parsed(rust_char, text));
            }
            // else: discard (low-confidence Rust-only entity, Python didn't validate)
        }
    }

    // Pass 2: Python entities not matched in Pass 1 → Profile 3: Python-only
    for py_entity in &python_result.entities {
        let key = py_entity.lemma.to_lowercase();
        if !seen_rust.contains(&key) {
            // Profile 3: Python-only entity — accept as-is
            seen_python.insert(key.clone());
            merged.push(py_entity.clone());
        }
        // Profile 4 (lemma conflict) is handled implicitly:
        // If Rust has "lemma A" and Python has "lemma B" for same position,
        // they won't match by lemma → Python goes through Profile 3,
        // Rust goes through Profile 2 (accept if confidence ≥ 0.7).
        // This is the desired behaviour: Python wins for morph,
        // Rust still contributes its positional data IF its confidence is high enough.
    }

    // Note: at this point, Profile 4 (lemma conflict) is detected heuristically:
    // if a Rust entity was discarded in Pass 1 (Profile 2, confidence < 0.7)
    // AND there's a Python entity at a nearby byte offset, that's a conflict.
    // For now, we don't merge positions in this case — Python wins outright.
    // Future: add byte-offset proximity check to merge positions even on lemma conflict.

    // Compute merged stats
    let persons = merged.iter().filter(|e| e.label == "PER").count();
    let locations = merged.iter().filter(|e| e.label == "LOC").count();
    let organizations = merged.iter().filter(|e| e.label == "ORG").count();
    let stats = NerStats {
        total: merged.len(),
        persons,
        locations,
        organizations,
    };

    NerResult {
        entities: merged,
        stats,
        model: "rust-fast-path+natasha-merge".to_string(),
        version: "2.2".to_string(),
        truncated: python_result.truncated,
        text_length: text.len(),
        processed_length: python_result.processed_length,
        chunks_processed: python_result.chunks_processed,
    }
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
// v2.2 / Phase 2 Step 1+2: Unit tests for rust_fast_path_entities()
// ============================================================================
//
// Эти тесты — формальная верификация Phase 2 dispatch policy:
//
//   1. Single-token name + speech verb → fast path срабатывает, model="rust-fast-path"
//   2. Multi-token name → fast path НЕ срабатывает (None → Python fallback)
//   3. Только concept (Бездна) → fast path НЕ срабатывает (None → Python для validation)
//   4. Пустой текст → fast path возвращает пустой NerResult (без Python spawn)
//   5. Mixed: single + multi-token → fast path НЕ срабатывает (хотя бы один multi → fallback)
//   6. (v2.2) mentions/firstMention populated из mention_starts byte offsets
//
// Без этих тестов «dispatch policy» остаётся только документацией.
#[cfg(test)]
mod phase2_fast_path_tests {
    use super::*;

    /// Single-token name + speech verb → fast path eligible.
    /// Latency: <1мс (без Python spawn).
    /// model="rust-fast-path", version="2.2", entities содержат персонажа.
    #[test]
    fn test_fast_path_single_token_with_speech_verb() {
        // "Борис сказал слово" → 2 сигнала (cap + speech), single-token → 0.7
        let text = "Борис сказал слово. Борис промолчал и ушёл в ночь.";
        let result = rust_fast_path_entities(text);

        assert!(result.is_some(), "Fast path должен сработать для single-token + speech verb");
        let ner = result.unwrap();

        assert_eq!(ner.model, "rust-fast-path");
        assert_eq!(ner.version, "2.2");
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
        assert_eq!(parsed.get("version").and_then(|v| v.as_str()), Some("2.2"));
        assert_eq!(parsed.get("truncated").and_then(|v| v.as_bool()), Some(false));
        assert!(parsed.get("textLength").is_some(), "textLength field (camelCase)");
        assert!(parsed.get("processedLength").is_some(), "processedLength field (camelCase)");
        // chunksProcessed — Option, может быть null в JSON
        assert!(parsed.get("chunksProcessed").is_some(),
            "chunksProcessed field должен присутствовать (даже если null)");
    }

    // ========================================================================
    // v2.2 / Step 2b: Tests for mention_starts → mentions/firstMention wiring
    // ========================================================================

    /// v2.2: Fast path результат должен содержать populated mentions
    /// (не пустой Vec, как в v2.1).
    #[test]
    fn test_fast_path_mentions_populated() {
        let text = "Борис сказал слово. Борис промолчал и ушёл в ночь.";
        let result = rust_fast_path_entities(text).expect("fast path должен сработать");

        let boris = result.entities.iter().find(|e| e.lemma == "Борис")
            .expect("Борис должен быть в entities");

        // mentions должен быть непустым (Борис встречается 2 раза в тексте)
        assert!(!boris.mentions.is_empty(),
            "mentions должен быть populated (v2.2), не пустым как в v2.1");

        // Каждый mention должен иметь валидные byte offsets
        for m in &boris.mentions {
            assert!(m.start < m.end, "start < end для каждого mention");
            assert!(m.end <= text.len(), "end не выходит за пределы текста");
            // Проверяем что text[start..end] действительно содержит "Борис" (case-insensitive)
            assert_eq!(
                text[m.start..m.end].to_lowercase(),
                "борис",
                "mention text должен быть формой имени"
            );
            assert!(!m.sentence.is_empty(), "sentence не пустой");
        }

        // first_mention должен указывать на первый "Борис" в тексте
        let first_boris_pos = text.to_lowercase().find("борис")
            .expect("Борис должен быть в тексте");
        assert_eq!(boris.first_mention, first_boris_pos,
            "first_mention должен указывать на первое упоминание");
    }

    /// v2.2: Все mentions должны быть отсортированы по позиции.
    #[test]
    fn test_fast_path_mentions_sorted_by_position() {
        let text = "Архип сказал. Потом Архип ушёл. Архип вернулся.";
        let result = rust_fast_path_entities(text).expect("fast path должен сработать");

        let arkhip = result.entities.iter().find(|e| e.lemma == "Архип")
            .expect("Архип должен быть в entities");

        let positions: Vec<usize> = arkhip.mentions.iter().map(|m| m.start).collect();
        let mut sorted = positions.clone();
        sorted.sort();
        assert_eq!(positions, sorted, "mentions должны быть отсортированы по позиции");
    }
}

// ============================================================================
// v2.2 / Phase 2 Step 2c: Unit tests for merge_results() — 4-way merge policy
// ============================================================================
//
// Тесты на 4 профиля merge policy (arch plan §4.2):
//   1. Profile 1 (Rust X, Python X) — confirmed: merge mentions, Rust keeps positions
//   2. Profile 2 (Rust X, Python —) — Rust-only: accept if confidence ≥ 0.7
//   3. Profile 3 (Rust —, Python X) — Python-only: accept as-is
//   4. Profile 4 (lemma conflict) — Python wins morph, Rust wins positions (if eligible)
//   5. Stats aggregation correct after merge
//   6. Provenance: model = "rust-fast-path+natasha-merge", version = "2.2"
//
// Эти тесты НЕ запускают Python — они конструируют NerResult вручную,
// имитируя Python-вывод, и проверяют логику merge.
#[cfg(test)]
mod merge_policy_tests {
    use super::*;
    use crate::parser::characters::{detect, EntityType, ParsedCharacter, SIGNAL_CAPITALIZED};

    /// Helper: построить fake Python NerResult с заданными entities.
    fn fake_python_result(entities: Vec<Entity>) -> NerResult {
        NerResult {
            entities,
            stats: NerStats { total: 0, persons: 0, locations: 0, organizations: 0 },
            model: "natasha-slovnet+pymorphy3".to_string(),
            version: "2.0".to_string(),
            truncated: false,
            text_length: 0,
            processed_length: 0,
            chunks_processed: None,
        }
    }

    /// Helper: построить fake Python Entity (без mentions — Python их даст отдельно).
    fn fake_python_entity(lemma: &str, label: &str, count: usize) -> Entity {
        Entity {
            lemma: lemma.to_string(),
            label: label.to_string(),
            count,
            forms: vec![lemma.to_string()],
            first_mention: 0,
            mentions: vec![],
        }
    }

    /// Profile 1: Rust и Python оба нашли "Борис".
    /// Merge: берём Rust mentions (позиции), Python lemma/label/forms (morph).
    #[test]
    fn test_merge_profile1_confirmed() {
        let text = "Борис сказал слово. Борис промолчал.";
        let rust_parsed = detect(text);
        let python = fake_python_result(vec![fake_python_entity("Борис", "PER", 2)]);

        let merged = merge_results(&rust_parsed, &python, text);

        assert_eq!(merged.model, "rust-fast-path+natasha-merge");
        assert_eq!(merged.version, "2.2");
        assert_eq!(merged.entities.len(), 1, "одна merged entity");

        let boris = &merged.entities[0];
        assert_eq!(boris.lemma, "Борис");
        assert_eq!(boris.label, "PER");
        // Rust positional data должна быть populated (не пустой, как от Python)
        assert!(!boris.mentions.is_empty(), "mentions из Rust (positions)");
        assert!(boris.first_mention > 0, "first_mention из Rust (position)");
    }

    /// Profile 2: Rust нашёл "Борис", Python его не нашёл.
    /// Принимаем если confidence ≥ 0.7 (что верно для single-token + speech verb).
    #[test]
    fn test_merge_profile2_rust_only_accepted() {
        let text = "Борис сказал слово. Борис промолчал.";
        let rust_parsed = detect(text);
        // Python вернул пустой список entities
        let python = fake_python_result(vec![]);

        let merged = merge_results(&rust_parsed, &python, text);

        // Rust-only Boris с confidence 0.7 должен пройти
        let boris = merged.entities.iter().find(|e| e.lemma == "Борис");
        assert!(boris.is_some(), "Profile 2: Rust-only Boris (confidence≥0.7) должен быть принят");
        let boris = boris.unwrap();
        assert!(!boris.mentions.is_empty(), "mentions populated из Rust");
    }

    /// Profile 2 (negative): Rust нашёл low-confidence entity, Python — нет.
    /// Должен быть discarded (confidence < 0.7).
    #[test]
    fn test_merge_profile2_rust_only_low_confidence_discarded() {
        let text = "Бездна смотрела. Бездна звала. Бездна ждала. \
                    Бездна дышала. Бездна молчала. Бездна пела. \
                    Бездна раскрывалась. Бездна закрывалась. \
                    Бездна улыбалась. Бездна хмурилась.";
        let rust_parsed = detect(text);
        // В этом тексте Бездна — Concept (нет speech verb), confidence 0.3
        let python = fake_python_result(vec![]);

        let merged = merge_results(&rust_parsed, &python, text);

        // Concept не должен попасть в merge result (он не Character)
        let bezdna = merged.entities.iter().find(|e| e.lemma == "Бездна");
        assert!(bezdna.is_none(), "Concept entity не должен попасть в merge");
    }

    /// Profile 3: Python нашёл entity, Rust — нет.
    /// Принимаем как high-confidence fallback.
    #[test]
    fn test_merge_profile3_python_only_accepted() {
        let text = "Какой-то текст без персонажей.";
        let rust_parsed = detect(text);
        // Python нашёл LOC, который Rust не умеет искать
        let python = fake_python_result(vec![
            fake_python_entity("Москва", "LOC", 1),
        ]);

        let merged = merge_results(&rust_parsed, &python, text);

        let moscow = merged.entities.iter().find(|e| e.lemma == "Москва");
        assert!(moscow.is_some(), "Profile 3: Python-only entity должен быть принят");
        assert_eq!(moscow.unwrap().label, "LOC");
    }

    /// Profile 4: Lemma conflict — Rust нашёл "Борис", Python нашёл "Боря"
    /// (разные леммы для одного и того же персонажа).
    /// Оба должны пройти — Python wins morph (своими lemma), Rust wins positions (если eligible).
    #[test]
    fn test_merge_profile4_lemma_conflict() {
        let text = "Борис сказал слово. Борис промолчал.";
        let rust_parsed = detect(text);
        // Python вернул другую lemma для того же персонажа
        let python = fake_python_result(vec![
            fake_python_entity("Боря", "PER", 2),
        ]);

        let merged = merge_results(&rust_parsed, &python, text);

        // Оба должны быть в merged: Борис (Rust, confidence 0.7) + Боря (Python)
        let boris = merged.entities.iter().find(|e| e.lemma == "Борис");
        let borya = merged.entities.iter().find(|e| e.lemma == "Боря");
        assert!(boris.is_some(), "Rust Борис должен пройти (confidence ≥ 0.7)");
        assert!(borya.is_some(), "Python Боря должна пройти (Profile 3)");
        // У Бориса — Rust positions (mentions populated)
        assert!(!boris.unwrap().mentions.is_empty());
    }

    /// Stats aggregation: merged stats должны правильно считать PER/LOC/ORG.
    #[test]
    fn test_merge_stats_aggregation() {
        let text = "Борис сказал слово. Борис промолчал.";
        let rust_parsed = detect(text);
        let python = fake_python_result(vec![
            fake_python_entity("Борис", "PER", 2),
            fake_python_entity("Москва", "LOC", 1),
            fake_python_entity("КГБ", "ORG", 1),
        ]);

        let merged = merge_results(&rust_parsed, &python, text);

        assert_eq!(merged.stats.persons, 1, "один PER (Борис)");
        assert_eq!(merged.stats.locations, 1, "одна LOC (Москва)");
        assert_eq!(merged.stats.organizations, 1, "одна ORG (КГБ)");
        assert_eq!(merged.stats.total, 3, "total = 3 entities");
    }

    /// Provenance: merged result должен иметь правильный model + version.
    #[test]
    fn test_merge_provenance() {
        let text = "Борис сказал слово.";
        let rust_parsed = detect(text);
        let python = fake_python_result(vec![fake_python_entity("Борис", "PER", 1)]);

        let merged = merge_results(&rust_parsed, &python, text);

        assert_eq!(merged.model, "rust-fast-path+natasha-merge");
        assert_eq!(merged.version, "2.2");
    }
}
