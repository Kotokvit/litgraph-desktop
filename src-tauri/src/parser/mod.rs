//! Автопарсер .md → граф.
//! Переписано с src/app/api/parse-md/route.ts (705 строк TS).

pub mod chapters;
pub mod characters;
pub mod locations;
// pub mod themes; // убран — не нужен
pub mod epsilon;

use crate::models::{LitEdge, LitNode, LitNodeData, ParseResult, ParseStats, Position};
use chrono::Utc;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Пустой текст")]
    Empty,
    #[error("Regex error: {0}")]
    Regex(#[from] fancy_regex::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

fn uid(prefix: &str) -> String {
    format!(
        "{}_{}_{}",
        prefix,
        Utc::now().timestamp_millis(),
        &Uuid::new_v4().to_string()[..8]
    )
}

pub fn build_graph(
    markdown: &str,
    project_title: &str,
    author: &str,
) -> Result<ParseResult, ParseError> {
    if markdown.trim().is_empty() {
        return Err(ParseError::Empty);
    }

    let (chapters, prologue_text) = chapters::detect(markdown);
    let characters = characters::detect(markdown);
    let locations = locations::detect(markdown);

    // === v0.3.0: Перекрёстная дедупликация characters ↔ locations ===
    // Если слово обнаружено и как персонаж, и как локация — оставить только
    // персонажа. Имена персонажей в косвенных падежах часто матчатся
    // локациями через «к Алексея», «от Марты» и т.д.
    // Используем грубую лемматизацию по окончаниям (без pymorphy2).
    let char_lemmas: std::collections::HashSet<String> = characters
        .iter()
        .flat_map(|c| {
            let mut l = vec![lemmatize_simple(&c.name)];
            l.extend(c.aliases.iter().map(|a| lemmatize_simple(a)));
            l
        })
        .collect();
    let locations: Vec<locations::ParsedLocation> = locations
        .into_iter()
        .filter(|l| !char_lemmas.contains(&lemmatize_simple(&l.name)))
        .collect();

    let mut nodes: Vec<LitNode> = Vec::new();
    let mut edges: Vec<LitEdge> = Vec::new();

    // --- Epsilon: вычисляем энергию значимости ---
    let (global_counts, total_words_count) = epsilon::build_word_counts(markdown);
    let mut epsilon_results: Vec<epsilon::EpsilonResult> = chapters
        .iter()
        .map(|ch| epsilon::compute_epsilon(&ch.full_text, &global_counts, total_words_count, None, 1.0))
        .collect();
    epsilon::normalize_epsilons(&mut epsilon_results);
    let prologue_epsilon = if prologue_text.trim().len() > 100 {
        Some(epsilon::compute_epsilon(&prologue_text, &global_counts, total_words_count, None, 1.0))
    } else { None };

    // --- Пролог (если есть) ---
    let mut prologue_id: Option<String> = None;
    if prologue_text.trim().len() > 100 {
        let id = uid("ch");
        let body_preview: String = prologue_text.split_whitespace().collect::<Vec<_>>().join(" ");
        let body = if body_preview.len() > 400 {
            // Безопасный срез по char boundary (UTF-8)
            let mut end = 400;
            while end > 0 && !body_preview.is_char_boundary(end) {
                end -= 1;
            }
            format!("{}…", &body_preview[..end])
        } else {
            body_preview
        };
        let word_count = prologue_text.split_whitespace().count();
        nodes.push(LitNode {
            id: id.clone(),
            node_type: "chapter".to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: "Пролог".to_string(),
                body,
                node_type: "chapter".to_string(),
                tags: vec!["пролог".to_string()],
                meta: Some(if let Some(ref pe) = prologue_epsilon {
                    serde_json::json!({ "wordCount": word_count, "epsilon": (pe.normalized.round() as i64), "emotion": pe.emotion_count })
                } else {
                    serde_json::json!({ "wordCount": word_count })
                }),
                full_text: Some(prologue_text.clone()),
                versions: None,
            },
        });
        prologue_id = Some(id);
    }

    // --- Главы ---
    let mut chapter_ids: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
    for (idx, ch) in chapters.iter().enumerate() {
        let id = uid("ch");
        chapter_ids.insert(ch.num, id.clone());

        // Найдём персонажей и локации в этой главе
        let ch_chars: Vec<String> = characters
            .iter()
            .filter(|c| characters::count_in_text(&c.aliases, &ch.full_text) > 0)
            .map(|c| c.name.clone())
            .collect();
        let ch_locs: Vec<String> = locations
            .iter()
            .filter(|l| locations::count_in_text(&l.aliases, &ch.full_text) > 0)
            .map(|l| l.name.clone())
            .collect();

        let word_count = ch.full_text.split_whitespace().count();
        let mut meta = serde_json::Map::new();
        meta.insert("wordCount".to_string(), serde_json::Value::Number(word_count.into()));
        let eps = &epsilon_results[idx];
        meta.insert("epsilon".to_string(), serde_json::Value::Number((eps.normalized.round() as i64).into()));
        meta.insert("emotion".to_string(), serde_json::Value::Number((eps.emotion_count as i64).into()));
        meta.insert("uniqueWords".to_string(), serde_json::Value::Number((eps.unique_words as i64).into()));
        if !ch_chars.is_empty() {
            meta.insert(
                "characters".to_string(),
                serde_json::Value::String(ch_chars.iter().take(5).cloned().collect::<Vec<_>>().join(", ")),
            );
        }
        if !ch_locs.is_empty() {
            meta.insert(
                "locations".to_string(),
                serde_json::Value::String(ch_locs.iter().take(3).cloned().collect::<Vec<_>>().join(", ")),
            );
        }

        nodes.push(LitNode {
            id: id.clone(),
            node_type: "chapter".to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: format!("Глава {}: {}", ch.num, ch.title),
                body: ch.body.clone(),
                node_type: "chapter".to_string(),
                tags: vec![],
                meta: Some(serde_json::Value::Object(meta)),
                full_text: Some(ch.full_text.clone()),
                versions: None,
            },
        });
    }

    // --- Персонажи ---
    let mut char_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for c in &characters {
        let id = uid("chr");
        char_ids.insert(c.name.clone(), id.clone());

        let chapters_with: Vec<&chapters::ParsedChapter> = chapters
            .iter()
            .filter(|ch| characters::count_in_text(&c.aliases, &ch.full_text) > 0)
            .collect();
        let first_chapter = chapters_with
            .first()
            .map(|ch| format!("Глава {}", ch.num))
            .unwrap_or_else(|| "—".to_string());

        nodes.push(LitNode {
            id: id.clone(),
            node_type: "character".to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: c.name.clone(),
                body: c.description.clone(),
                node_type: "character".to_string(),
                tags: vec![],
                meta: Some(serde_json::json!({
                    "mentions": c.count,
                    "chapters": format!("{} глав", chapters_with.len()),
                    "firstChapter": first_chapter,
                    // v0.3.0: X-ray поля — показывают ПОЧЕМУ парсер решил
                    // что это персонаж. Видны в HTML X-ray export sidebar.
                    "speechCount": c.speech_count,
                    "directCount": c.direct_count,
                    "reason": c.reason,
                })),
                full_text: None,
                versions: None,
            },
        });
    }

    // --- Локации ---
    let mut loc_ids: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for l in &locations {
        let id = uid("loc");
        loc_ids.insert(l.name.clone(), id.clone());

        let chapters_with: Vec<&chapters::ParsedChapter> = chapters
            .iter()
            .filter(|ch| locations::count_in_text(&l.aliases, &ch.full_text) > 0)
            .collect();
        let first_chapter = chapters_with
            .first()
            .map(|ch| format!("Глава {}", ch.num))
            .unwrap_or_else(|| "—".to_string());

        nodes.push(LitNode {
            id: id.clone(),
            node_type: "location".to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: l.name.clone(),
                body: l.description.clone(),
                node_type: "location".to_string(),
                tags: vec![],
                meta: Some(serde_json::json!({
                    "mentions": l.count,
                    "chapters": format!("{} глав", chapters_with.len()),
                    "firstChapter": first_chapter,
                })),
                full_text: None,
                versions: None,
            },
        });
    }


    // --- Связи: поток глав ---
    let mut ordered: Vec<String> = Vec::new();
    if let Some(pid) = &prologue_id {
        ordered.push(pid.clone());
    }
    for (idx, ch) in chapters.iter().enumerate() {
        if let Some(id) = chapter_ids.get(&ch.num) {
            ordered.push(id.clone());
        }
    }
    for i in 0..ordered.len().saturating_sub(1) {
        edges.push(LitEdge {
            id: uid("e"),
            source: ordered[i].clone(),
            target: ordered[i + 1].clone(),
            source_handle: None,
            target_handle: None,
            edge_type: Some("smoothstep".to_string()),
            animated: Some(true),
            data: Some(crate::models::EdgeData { kind: Some("flow".to_string()), note: None }),
        });
    }

    // --- Связи: персонажи → главы ---
    for c in &characters {
        if let Some(cid) = char_ids.get(&c.name) {
            for (idx, ch) in chapters.iter().enumerate() {
                let count = characters::count_in_text(&c.aliases, &ch.full_text);
                if count >= 3 {
                    if let Some(ch_id) = chapter_ids.get(&ch.num) {
                        edges.push(LitEdge {
                            id: uid("e"),
                            source: cid.clone(),
                            target: ch_id.clone(),
                            source_handle: None,
                            target_handle: None,
                            edge_type: Some("smoothstep".to_string()),
                            animated: Some(false),
                            data: Some(crate::models::EdgeData { kind: Some("character".to_string()), note: None }),
                        });
                    }
                }
            }
        }
    }

    // --- Связи: локации → главы ---
    for l in &locations {
        if let Some(lid) = loc_ids.get(&l.name) {
            for (idx, ch) in chapters.iter().enumerate() {
                let count = locations::count_in_text(&l.aliases, &ch.full_text);
                if count >= 2 {
                    if let Some(ch_id) = chapter_ids.get(&ch.num) {
                        edges.push(LitEdge {
                            id: uid("e"),
                            source: lid.clone(),
                            target: ch_id.clone(),
                            source_handle: None,
                            target_handle: None,
                            edge_type: Some("smoothstep".to_string()),
                            animated: Some(false),
                            data: Some(crate::models::EdgeData { kind: Some("location".to_string()), note: None }),
                        });
                    }
                }
            }
        }
    }


    // --- Раскладка ---
    layout_nodes(&mut nodes, &chapters, &prologue_id, &characters, &locations, &chapter_ids, &char_ids, &loc_ids);

    let word_count = markdown.split_whitespace().count();
    let now = Utc::now().timestamp_millis() as u64;
    let edges_count = edges.len();
    let chapters_count = chapters.len();
    let characters_count = characters.len();
    let locations_count = locations.len();
    

    Ok(ParseResult {
        title: project_title.to_string(),
        author: author.to_string(),
        description: format!(
            "Автоматически разобранный текст: {} глав, {} персонажей, {} локаций, {} связей. Всего {} слов.",
            chapters_count,
            characters_count,
            locations_count,
            
            edges_count,
            word_count
        ),
        nodes,
        edges,
        created_at: now,
        updated_at: now,
        stats: ParseStats {
            chapters: chapters_count,
            characters: characters_count,
            locations: locations_count,
            edges: edges_count,
            words: word_count,
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn layout_nodes(
    nodes: &mut Vec<LitNode>,
    chapters: &[chapters::ParsedChapter],
    prologue_id: &Option<String>,
    characters: &[characters::ParsedCharacter],
    locations: &[locations::ParsedLocation],
    chapter_ids: &std::collections::HashMap<u32, String>,
    char_ids: &std::collections::HashMap<String, String>,
    loc_ids: &std::collections::HashMap<String, String>,
    
) {
    // Главы — центральная колонка
    let chapter_x = 600.0;
    let chapter_y_start = 60.0;
    let chapter_y_step = 130.0;
    for (i, ch) in chapters.iter().enumerate() {
        if let Some(id) = chapter_ids.get(&ch.num) {
            if let Some(n) = nodes.iter_mut().find(|n| n.id == *id) {
                n.position = Position {
                    x: chapter_x,
                    y: chapter_y_start + (i as f64) * chapter_y_step,
                };
            }
        }
    }
    if let Some(pid) = prologue_id {
        if let Some(n) = nodes.iter_mut().find(|n| n.id == *pid) {
            n.position = Position {
                x: chapter_x,
                y: chapter_y_start - chapter_y_step,
            };
        }
    }
    let char_x = 1100.0;
    let char_y_start = 60.0;
    let char_y_step = 110.0;
    for (i, c) in characters.iter().enumerate() {
        if let Some(id) = char_ids.get(&c.name) {
            if let Some(n) = nodes.iter_mut().find(|n| n.id == *id) {
                n.position = Position {
                    x: char_x,
                    y: char_y_start + (i as f64) * char_y_step,
                };
            }
        }
    }

    // Локации — ещё правее
    let loc_x = 1500.0;
    let loc_y_start = 60.0;
    let loc_y_step = 110.0;
    for (i, l) in locations.iter().enumerate() {
        if let Some(id) = loc_ids.get(&l.name) {
            if let Some(n) = nodes.iter_mut().find(|n| n.id == *id) {
                n.position = Position {
                    x: loc_x,
                    y: loc_y_start + (i as f64) * loc_y_step,
                };
            }
        }
    }
}

// Утилита для генерации ID (используется в storage тоже)
pub fn new_uid(prefix: &str) -> String {
    uid(prefix)
}

/// Простая лемматизация русского/украинского слова через отсечение
/// типичных окончаний. Это грубая аппроксимация — для настоящей точности
/// нужен pymorphy3. Используется для:
///   1. Перекрёстной дедупликации characters ↔ locations (чтобы
///      «Алексея»-локация не дублировала «Алексей»-персонажа).
///   2. Группировки падежей внутри characters::detect (v0.3.1):
///      «Алексей» + «Алексея» + «Алексею» → одна группа с lemma «алексе».
///
/// Ограничения (без словаря нельзя исправить):
///   - Короткие имена (≤4 символов) возвращаются as-is: «Рэй» и «Рэя»
///     НЕ сольются (нужен pymorphy3 — Варіант C).
///   - Стем-чередования (Веня ↔ Вениамин) не обрабатываются.
///   - Возвращает как минимум 3 буквы корня (защита от over-cutting).
pub fn lemmatize_simple(word: &str) -> String {
    let lower = word.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() <= 4 {
        return lower;
    }
    // v0.3.1: Спецвипадки для російських чоловічих імен на -ей / -ий.
    // Проблема: «Алексей» ( nominative) закінчується на -й, який не в endings
    // (бо це частина основи). Тому lemma = «алексей». Але «Алексея» (genitive)
    // віддає «алексе» (обрізане -я). Вони НЕ зливаються — хоча це один персонаж.
    // Фікс: для слів > 4 символів на -ей/-ій відкидаємо останній -й → «алексе».
    // Це правильно зливає Алексей+Алексея+Алексею, Андрей+Андрея, Сергей+Сергея,
    // Геннадий+Геннадия, Виталий+Виталия — без впливу на інші слова.
    let last_two: Vec<char> = chars[chars.len() - 2..].to_vec();
    if last_two == vec!['е', 'й'] || last_two == vec!['и', 'й'] {
        return chars[..chars.len() - 1].iter().collect();
    }
    // v0.3.1: Спецвипадок для -ею (instrumental of -ей names: «Алексею», «Андрею»).
    // Без цього «Алексею» матчитися з 2-char ending "ею" → lemma "алекс"
    // (відрізає і "ю", і "е"), що НЕ зливається з «алексе» від «Алексей».
    // Фікс: для слів > 4 символів на -ею відкидаємо тільки "ю" → «алексе».
    if chars.len() > 4 && last_two == vec!['е', 'ю'] {
        return chars[..chars.len() - 1].iter().collect();
    }
    // Типичные русские/украинские окончания (попытка отсечь самое длинное совпадение)
    let endings: &[&str] = &[
        "ами", "ями", "ого", "его", "ому", "ему", "ыми", "ими",
        "ах", "ях", "ой", "ая", "ее", "ие", "ые", "ою", "ею",
        "ом", "ем", "ам", "ям",
        "а", "я", "у", "ю", "ы", "и", "е", "о",
    ];
    for ending in endings {
        let ending_chars: Vec<char> = ending.chars().collect();
        if chars.len() > ending_chars.len() + 2 {
            // оставить минимум 3 буквы корня
            let tail_start = chars.len() - ending_chars.len();
            if chars[tail_start..] == ending_chars[..] {
                return chars[..tail_start].iter().collect();
            }
        }
    }
    lower
}
