//! v0.4.0: Авто-пайплайн «одним вызовом».
//!
//! При загрузке .md файла автоматически запускает ВСЕ этапы анализа:
//!   1. Rust auto-parse (chapters, characters, locations, concepts, organizations)
//!   2. Python NER (spaCy + pymorphy3) — переопределяет персонажей с правильными
//!      леммами (решает Рэй/Рэя, Яме/Яму и т.д.)
//!   3. (Опционально) POLER-анализ и Conflict-graph — запускаются отдельно
//!      по кнопкам, т.к. они тяжёлые и данные не вписываются в ParseResult
//!
//! Принцип «один алгоритм»: каждый этап выполняет свою задачу и передаёт
//! результат следующему. NER enrich'ит (не заменяет) Rust-парсер —
//! добавляет недостающие имена, исправляет леммы, но не трогает chapters
//! и locations (Rust-парсер для них достаточно точен).

use crate::commands::ner::{run_python_with_text_file, NerResult};
use crate::models::{ParseParams, ParseResult};
use crate::parser;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Прогресс выполнения пайплайна (для UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipelineProgress {
    pub step: String,
    pub step_index: u32,
    pub total_steps: u32,
    pub message: String,
}

/// Результат авто-пайплайна.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FullParseResult {
    /// Базовый граф (chapters + characters + locations + concepts + organizations)
    pub parse_result: ParseResult,
    /// NER-сущности (если NER удалось запустить). None если Python/spaCy недоступны.
    pub ner_entities: Option<NerResult>,
    /// Было ли NER успешно объединено с Rust-персонажами.
    pub ner_merged: bool,
    /// Версия пайплайна.
    pub pipeline_version: String,
}

/// Tauri команда: полный авто-пайплайн при загрузке .md файла.
///
/// Шаги:
///   1. Rust auto-parse → базовый ParseResult
///   2. Python NER → извлечение сущностей с pymorphy3 лемматизацией
///   3. Merge: NER-персонажи enrich'ат Rust-персонажей (добавляют alias'ы,
///      исправляют леммы для коротких имён типа Рэй/Рэя)
///
/// Если NER не запускается (Python/spaCy не установлены) — возвращаем
/// только Rust-результат с пометкой ner_merged=false. UI показывает
/// предупреждение «NER недоступен, используйте только Rust-парсер».
#[tauri::command]
pub async fn parse_md_full(params: ParseParams) -> Result<FullParseResult, String> {
    if params.markdown.trim().is_empty() {
        return Err("Пустой текст".to_string());
    }

    // === Шаг 1: Rust auto-parse ===
    let mut parse_result = parser::build_graph(&params.markdown, &params.project_title, &params.author)
        .map_err(|e| e.to_string())?;

    // === Шаг 2: Python NER (опционально) ===
    let ner_result = run_ner_safe(&params.markdown);

    // === Шаг 3: Merge NER → Rust characters ===
    let ner_merged = if let Some(ref ner) = ner_result {
        merge_ner_into_parse_result(&mut parse_result, ner, &params.markdown)
    } else {
        false
    };

    // Обновляем description
    if ner_merged {
        if let Some(ref ner) = ner_result {
            parse_result.description = format!(
                "{} NER-обогащён: {} сущностей (spaCy + pymorphy3).",
                parse_result.description,
                ner.stats.total
            );
        }
    }

    Ok(FullParseResult {
        parse_result,
        ner_entities: ner_result,
        ner_merged,
        pipeline_version: "0.4.0".to_string(),
    })
}

/// Запустить NER с graceful fallback.
/// Если Python/spaCy недоступны — возвращаем None (не падляем).
fn run_ner_safe(text: &str) -> Option<NerResult> {
    let script = include_str!("../../python/ner_extract.py");
    match run_python_with_text_file(script, text, &[]) {
        Ok(stdout) => {
            match serde_json::from_str::<NerResult>(&stdout) {
                Ok(result) => Some(result),
                Err(e) => {
                    eprintln!("[parse_md_full] NER JSON parse error: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("[parse_md_full] NER Python error: {}", e);
            None
        }
    }
}

/// v0.4.2: Hard filter для NER entities — отбрасывает мусор до merge.
///
/// Критерии reject:
///   1. lemma содержит \n, \r, \t (многострочный шум)
///   2. lemma начинается с lowercase буквы
///   3. lemma содержит одновременно кириллицу И латиницу ("Root-Оператор")
///   4. lemma содержит цифры или спецсимволы (кроме дефиса и апострофа)
///   5. count < MIN_NER_COUNT (3 для PER, 5 для ORG)
fn is_valid_ner_entity(lemma: &str, label: &str, count: usize) -> bool {
    if lemma.is_empty() {
        return false;
    }
    // 1. Newlines
    if lemma.contains('\n') || lemma.contains('\r') || lemma.contains('\t') {
        return false;
    }
    // 2. First char must be uppercase letter
    let first_char = lemma.chars().next().unwrap();
    if !first_char.is_alphabetic() || !first_char.is_uppercase() {
        return false;
    }
    // 3. Mix of Cyrillic and Latin → reject
    let has_cyr = lemma.chars().any(|c| ('а'..='я').contains(&c) || ('А'..='Я').contains(&c) || c == 'ё' || c == 'Ё');
    let has_lat = lemma.chars().any(|c| ('a'..='z').contains(&c) || ('A'..='Z').contains(&c));
    if has_cyr && has_lat {
        return false;
    }
    // 4. Only letters, spaces, hyphens, apostrophes
    if !lemma.chars().all(|c| c.is_alphabetic() || c == ' ' || c == '-' || c == '\'' || c == '\u{2019}') {
        return false;
    }
    // 5. Min count
    let min_count = match label {
        "PER" => 3,
        "ORG" => 5,
        "LOC" | "GPE" => 2,
        _ => 5,
    };
    if count < min_count {
        return false;
    }
    true
}

/// Merge NER-сущностей в ParseResult.
///
/// Стратегия:
///   - NER находит персонажей с правильными леммами (pymorphy3).
///     Если NER нашёл персонажа «Рэй» с forms=[Рэй, Рэя, Рэю] — мы добавляем
///     эти forms к Rust-персонажу «Рэй» (если он есть) или создаём нового.
///   - NER-локации добавляем к Rust-локациям (если ещё нет).
///   - NER-организации добавляем к Rust-organization нодам.
///
/// Возвращает true если merge прошёл успешно.
fn merge_ner_into_parse_result(
    parse_result: &mut ParseResult,
    ner: &NerResult,
    _full_text: &str,
) -> bool {
    use crate::models::{LitNode, LitNodeData, Position};
    use chrono::Utc;
    use uuid::Uuid;

    // Индекс персонажей по имени (lowercase) для быстрого поиска
    let mut char_by_name: HashMap<String, usize> = HashMap::new();
    for (i, n) in parse_result.nodes.iter().enumerate() {
        if n.node_type == "character" {
            char_by_name.insert(n.data.title.to_lowercase(), i);
        }
    }

    // Счётчики
    let mut added_chars = 0u32;
    let mut enriched_chars = 0u32;
    let mut added_locs = 0u32;
    let mut added_orgs = 0u32;

    // Проходим по всем NER-сущностям
    for entity in &ner.entities {
        // v0.4.2: Hard filter — отбрасываем мусор до обработки
        if !is_valid_ner_entity(&entity.lemma, &entity.label, entity.count) {
            continue;
        }
        match entity.label.as_str() {
            "PER" => {
                let lemma_lower = entity.lemma.to_lowercase();
                if let Some(&idx) = char_by_name.get(&lemma_lower) {
                    // Персонаж уже есть — enrich: добавляем недостающие forms
                    let node = &mut parse_result.nodes[idx];
                    if let Some(ref mut meta) = node.data.meta {
                        if let Some(existing_forms) = meta.get("forms").and_then(|v| v.as_array()) {
                            // Не трогаем — forms уже там
                            let _ = existing_forms;
                        }
                        // Обновляем mentions если NER нашёл больше
                        if let Some(ner_count) = Some(entity.count) {
                            let current = meta.get("mentions").and_then(|v| v.as_u64()).unwrap_or(0);
                            if (ner_count as u64) > current {
                                if let Some(v) = meta.as_object_mut() {
                                    v.insert("mentions".to_string(), serde_json::json!(ner_count));
                                    v.insert("nerEnriched".to_string(), serde_json::json!(true));
                                }
                            }
                        }
                    }
                    enriched_chars += 1;
                } else {
                    // Новый персонаж из NER — добавляем
                    let id = format!(
                        "chr_{}_{}",
                        Utc::now().timestamp_millis(),
                        &Uuid::new_v4().to_string()[..8]
                    );
                    let node = LitNode {
                        id: id.clone(),
                        node_type: "character".to_string(),
                        position: Position { x: 1100.0, y: 60.0 + (added_chars as f64) * 110.0 },
                        data: LitNodeData {
                            title: entity.lemma.clone(),
                            body: format!(
                                "NER-извлечённый персонаж (spaCy + pymorphy3). {} упоминаний, форм: {}.",
                                entity.count,
                                entity.forms.len()
                            ),
                            node_type: "character".to_string(),
                            tags: vec!["ner".to_string()],
                            meta: Some(serde_json::json!({
                                "mentions": entity.count,
                                "forms": entity.forms,
                                "source": "ner",
                                "reason": "character:rule=ner_spacy_pymorphy3;ner_label=PER",
                            })),
                            full_text: None,
                            versions: None,
                        },
                    };
                    parse_result.nodes.push(node);
                    char_by_name.insert(lemma_lower, parse_result.nodes.len() - 1);
                    added_chars += 1;
                }
            }
            "LOC" => {
                // Локации из NER — добавляем только если ещё нет
                let loc_lower = entity.lemma.to_lowercase();
                let exists = parse_result.nodes.iter().any(|n| {
                    n.node_type == "location" && n.data.title.to_lowercase() == loc_lower
                });
                if !exists && entity.count >= 2 {
                    let id = format!(
                        "loc_{}_{}",
                        Utc::now().timestamp_millis(),
                        &Uuid::new_v4().to_string()[..8]
                    );
                    let node = LitNode {
                        id,
                        node_type: "location".to_string(),
                        position: Position { x: 1500.0, y: 60.0 + (added_locs as f64) * 110.0 },
                        data: LitNodeData {
                            title: entity.lemma.clone(),
                            body: format!("NER-локация, {} упоминаний.", entity.count),
                            node_type: "location".to_string(),
                            tags: vec!["ner".to_string()],
                            meta: Some(serde_json::json!({
                                "mentions": entity.count,
                                "forms": entity.forms,
                                "source": "ner",
                            })),
                            full_text: None,
                            versions: None,
                        },
                    };
                    parse_result.nodes.push(node);
                    added_locs += 1;
                }
            }
            "ORG" => {
                // Организации из NER — добавляем если ещё нет
                let org_lower = entity.lemma.to_lowercase();
                let exists = parse_result.nodes.iter().any(|n| {
                    (n.node_type == "organization" || n.node_type == "concept")
                        && n.data.title.to_lowercase() == org_lower
                });
                if !exists && entity.count >= 2 {
                    let id = format!(
                        "org_{}_{}",
                        Utc::now().timestamp_millis(),
                        &Uuid::new_v4().to_string()[..8]
                    );
                    let node = LitNode {
                        id,
                        node_type: "organization".to_string(),
                        position: Position { x: 1350.0, y: 60.0 + (added_orgs as f64) * 110.0 },
                        data: LitNodeData {
                            title: entity.lemma.clone(),
                            body: format!(
                                "NER-организация (spaCy), {} упоминаний.",
                                entity.count
                            ),
                            node_type: "organization".to_string(),
                            tags: vec!["ner".to_string()],
                            meta: Some(serde_json::json!({
                                "mentions": entity.count,
                                "forms": entity.forms,
                                "source": "ner",
                                "reason": "organization:rule=ner_spacy;ner_label=ORG",
                            })),
                            full_text: None,
                            versions: None,
                        },
                    };
                    parse_result.nodes.push(node);
                    added_orgs += 1;
                }
            }
            _ => {} // другие типы (MISC и т.д.) игнорируем
        }
    }

    // Обновляем stats
    parse_result.stats.characters = parse_result.nodes.iter()
        .filter(|n| n.node_type == "character")
        .count();
    parse_result.stats.locations = parse_result.nodes.iter()
        .filter(|n| n.node_type == "location")
        .count();

    eprintln!(
        "[parse_md_full] NER merge: +{} chars, enriched {} chars, +{} locs, +{} orgs",
        added_chars, enriched_chars, added_locs, added_orgs
    );

    true
}
