//! Автопарсер .md → граф.
//! Переписано с src/app/api/parse-md/route.ts (705 строк TS).

pub mod chapters;
pub mod characters;
pub mod locations;
// pub mod themes; // убран — не нужен
pub mod epsilon;

use crate::models::{LitEdge, LitNode, LitNodeData, ParseResult, ParseStats, Position};
use chrono::Utc;
use std::collections::HashMap;
use std::collections::HashSet;
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
    let characters_raw = characters::detect(markdown);
    // v0.4.0: применяем alias map (Веня→Вениамин, Аэлин→Аэлира, etc.)
    // ДО перекрёстной дедупликации с локациями.
    let characters = merge_aliases(characters_raw);
    let locations = locations::detect(markdown);

    // === v0.3.0: Перекрёстная дедупликация characters ↔ locations ===
    // Если слово обнаружено и как персонаж, и как локация — оставить только
    // персонажа. Имена персонажей в косвенных падежах часто матчатся
    // локациями через «к Алексея», «от Марты» и т.д.
    // Используем грубую лемматизацию по окончаниям (без pymorphy2).
    //
    // v0.4.2: Добавляем в char_lemmas ВСЕ алиасы (включая падежные формы
    // коротких имён: «рэя»→«рэй», «жору»→«жора», etc.). Раньше «Рэя»
    // оставалось в локациях, хотя это genitive от персонажа «Рэй».
    let alias_map: HashMap<String, String> = ALIASES
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let char_lemmas: std::collections::HashSet<String> = characters
        .iter()
        .flat_map(|c| {
            let mut l = vec![lemmatize_simple(&c.name)];
            l.extend(c.aliases.iter().map(|a| lemmatize_simple(a)));
            // v0.4.2: Добавляем все alias keys которые маппят к этому персонажу
            let canonical_lemma = lemmatize_simple(&c.name);
            for (alias_key, alias_target) in &alias_map {
                if lemmatize_simple(alias_target) == canonical_lemma {
                    l.push(alias_key.clone());
                }
            }
            l
        })
        .collect();
    let locations: Vec<locations::ParsedLocation> = locations
        .into_iter()
        .filter(|l| {
            let loc_lemma = lemmatize_simple(&l.name);
            let loc_lower = l.name.to_lowercase();
            // Проверяем и lemma, и оригинальную lowercase форму
            !char_lemmas.contains(&loc_lemma) && !char_lemmas.contains(&loc_lower)
        })
        .collect();

    let mut nodes: Vec<LitNode> = Vec::new();
    let mut edges: Vec<LitEdge> = Vec::new();

    // --- Epsilon: вычисляем энергию значимости ---
    let (global_counts, total_words_count) = epsilon::build_word_counts(markdown);
    let mut epsilon_results: Vec<epsilon::EpsilonResult> = chapters
        .iter()
        .map(|ch| {
            epsilon::compute_epsilon(&ch.full_text, &global_counts, total_words_count, None, 1.0)
        })
        .collect();
    epsilon::normalize_epsilons(&mut epsilon_results);
    let prologue_epsilon = if prologue_text.trim().len() > 100 {
        Some(epsilon::compute_epsilon(
            &prologue_text,
            &global_counts,
            total_words_count,
            None,
            1.0,
        ))
    } else {
        None
    };

    // --- Пролог (если есть) ---
    let mut prologue_id: Option<String> = None;
    if prologue_text.trim().len() > 100 {
        let id = uid("ch");
        let body_preview: String = prologue_text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
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
        meta.insert(
            "wordCount".to_string(),
            serde_json::Value::Number(word_count.into()),
        );
        let eps = &epsilon_results[idx];
        meta.insert(
            "epsilon".to_string(),
            serde_json::Value::Number((eps.normalized.round() as i64).into()),
        );
        meta.insert(
            "emotion".to_string(),
            serde_json::Value::Number((eps.emotion_count as i64).into()),
        );
        meta.insert(
            "uniqueWords".to_string(),
            serde_json::Value::Number((eps.unique_words as i64).into()),
        );
        if !ch_chars.is_empty() {
            meta.insert(
                "characters".to_string(),
                serde_json::Value::String(
                    ch_chars
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
        }
        if !ch_locs.is_empty() {
            meta.insert(
                "locations".to_string(),
                serde_json::Value::String(
                    ch_locs
                        .iter()
                        .take(3)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
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

    // --- Персонажи (включая концепты и организации) ---
    // v0.4.0: теперь ParsedCharacter имеет entity_type: Character | Organization | Concept
    // Для каждого типа создаём ноду соответствующего node_type
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

        // v0.4.0: node_type зависит от entity_type
        let (node_type, reason_label) = match c.entity_type {
            characters::EntityType::Character => ("character", "character"),
            characters::EntityType::Organization => ("organization", "organization"),
            characters::EntityType::Concept => ("concept", "concept"),
        };

        nodes.push(LitNode {
            id: id.clone(),
            node_type: node_type.to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: c.name.clone(),
                body: c.description.clone(),
                node_type: node_type.to_string(),
                tags: vec![],
                meta: Some(serde_json::json!({
                    "mentions": c.count,
                    "chapters": format!("{} глав", chapters_with.len()),
                    "firstChapter": first_chapter,
                    "speechCount": c.speech_count,
                    "directCount": c.direct_count,
                    "reason": c.reason,
                    "entityType": reason_label,
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
    for ch in chapters.iter() {
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
            data: Some(crate::models::EdgeData {
                kind: Some("flow".to_string()),
                note: None,
            }),
        });
    }

    // --- Связи: персонажи → главы ---
    for c in &characters {
        if let Some(cid) = char_ids.get(&c.name) {
            for ch in chapters.iter() {
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
                            data: Some(crate::models::EdgeData {
                                kind: Some("character".to_string()),
                                note: None,
                            }),
                        });
                    }
                }
            }
        }
    }

    // --- Связи: локации → главы ---
    for l in &locations {
        if let Some(lid) = loc_ids.get(&l.name) {
            for ch in chapters.iter() {
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
                            data: Some(crate::models::EdgeData {
                                kind: Some("location".to_string()),
                                note: None,
                            }),
                        });
                    }
                }
            }
        }
    }

    // --- Раскладка ---
    layout_nodes(
        &mut nodes,
        &chapters,
        &prologue_id,
        &characters,
        &locations,
        &chapter_ids,
        &char_ids,
        &loc_ids,
    );

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
    // v0.4.0: разделяем персонажей и концепты/организации по разным Y-координатам
    let mut char_row = 0usize;
    let mut concept_row = 0usize;
    let concept_y_start = 60.0 + 20.0 * char_y_step + 60.0; // ниже персонажей + отступ
    for (i, c) in characters.iter().enumerate() {
        if let Some(id) = char_ids.get(&c.name) {
            if let Some(n) = nodes.iter_mut().find(|n| n.id == *id) {
                let (x, y) = if c.entity_type == characters::EntityType::Character {
                    let y = char_y_start + (char_row as f64) * char_y_step;
                    char_row += 1;
                    (char_x, y)
                } else {
                    // concept/organization — отдельная колонка правее
                    let y = concept_y_start + (concept_row as f64) * char_y_step;
                    concept_row += 1;
                    (char_x + 250.0, y)
                };
                n.position = Position { x, y };
            }
        }
        // i нужно чтобы избежать unused warning
        let _ = i;
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
///   - Стем-чередования (Веня ↔ Вениамин) не обрабатываются лемматизатором
///     — для этого есть ALIASES в mod.rs (см. ниже).
///   - Возвращает как минимум 3 буквы корня (защита от over-cutting).
///
/// v0.4.0: Спецвипадок для женских имён на -ра/-ла/-на/-ма.
/// Проблема: «Аэлира» обрезалась до «аэлир» (отрезалась финальная «а»),
/// что НЕ матчится с «Аэлире» → тоже «аэлир» (совпадение случайно).
/// Но «Аэлин» (другое имя, уменьшительное) → «аэлин» (не обрезается,
/// 5 символов, окончание «ин» не в списке). В итоге Аэлира и Аэлин
/// НЕ сливались, хотя это один персонаж.
/// Фикс: для слов > 4 символов, заканчивающихся на согласную + «ра/ла/на/ма» + «а»,
/// сохраняем финальную «а». Это даёт: Аэлира → «аэлира», Иллира → «иллира»,
/// Марина → «марина», Алёна → «алёна». Падежи (Аэлире, Аэлиру) обрезаются
/// до «аэлир» — для их слияния с «аэлира» нужен alias map или pymorphy3.
pub fn lemmatize_simple(word: &str) -> String {
    let lower = word.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() <= 4 {
        return lower;
    }

    // v0.4.0: Женские имена на -ра/-ла/-на/-ма + «а».
    // Проверяем: предпоследняя буква ∈ {р, л, н, м}, последняя = «а»,
    // и перед ними стоит гласная (чтобы не слить «Марта» → «март»
    // с «Марту» → «март» — это рабочий кейс, оставляем).
    // Условие: word ends with [аеиоуяюэы] + [рлнм] + [а]
    // Примеры: Аэлира, Иллира, Марина, Алёна, Валерия, Виктория
    if chars.len() >= 5 {
        let last = chars[chars.len() - 1];
        let before_last = chars[chars.len() - 2];
        let before_before_last = chars[chars.len() - 3];
        let feminine_endings = ['р', 'л', 'н', 'м'];
        let vowels = ['а', 'е', 'и', 'о', 'у', 'я', 'ю', 'э', 'ы', 'ё'];
        if last == 'а'
            && feminine_endings.contains(&before_last)
            && vowels.contains(&before_before_last)
        {
            // Не обрезаем финальную «а» — возвращаем как есть.
            // Это сохраняет женские имена на -ра/-ла/-на/-ма в nominative.
            return lower;
        }
    }

    // v0.3.1: Спецвипадки для російських чоловічих імен на -ей / -ий.
    // Проблема: «Алексей» (nominative) закінчується на -й, який не в endings
    // (бо це частина основи). Тому lemma = «алексей». Але «Алексея» (genitive)
    // віддає «алексе» (обрізане -я). Вони НЕ зливаються — хоча це один персонаж.
    // Фікс: для слів > 4 символів на -ей/-ій відкидаємо останній -й → «алексе».
    let last_two: Vec<char> = chars[chars.len() - 2..].to_vec();
    if last_two == vec!['е', 'й'] || last_two == vec!['и', 'й'] {
        return chars[..chars.len() - 1].iter().collect();
    }
    // v0.3.1: Спецвипадок для -ею (instrumental of -ей names: «Алексею», «Андрею»).
    if chars.len() > 4 && last_two == vec!['е', 'ю'] {
        return chars[..chars.len() - 1].iter().collect();
    }
    // Типичные русские/украинские окончания (попытка отсечь самое длинное совпадение)
    let endings: &[&str] = &[
        "ами", "ями", "ого", "его", "ому", "ему", "ыми", "ими", "ах", "ях", "ой", "ая", "ее", "ие",
        "ые", "ою", "ею", "ом", "ем", "ам", "ям", "а", "я", "у", "ю", "ы", "и", "е", "о",
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

/// v0.4.0: Таблица алиасов для русских/украинских имён.
///
/// `lemmatize_simple` не может объединить разные основы (Веня ↔ Вениамин,
/// Вельямин ↔ Вениамин, Аэлин ↔ Аэлира) — для этого нужен словарь.
/// Эта таблица содержит пары (краткая форма → полная форма) для самых
/// частых русских имён. Используется в `merge_aliases()` после группировки.
///
/// В контексте анализируемого романа «1-Сфера Предела»:
///   - Веня (freq=25) + Вениамин (freq=47) + Вельямин (freq=43) = 115 упоминаний
///     одного персонажа (Вениамин Ард'Еш, он же «Вень», «Венька»)
///   - Аэлин (freq=23) + Аэлира (freq=47) = 70 упоминаний одного персонажа
///   - Иллира (freq=19) — отдельный персонаж (не алиас Аэлиры)
///
/// Принцип: алиасы применяются ТОЛЬКО когда lemma короткой формы
/// является prefix'ом lemma длинной формы (Веня → Вениамин: «веня» не prefix
/// «вениамин», но мы это знаем из словаря). Без словаря это не вычислить.
pub const ALIASES: &[(&str, &str)] = &[
    // Веня → Вениамин (уменьшительное от полного имени)
    ("веня", "вениамин"),
    ("венька", "вениамин"),
    ("венечка", "вениамин"),
    // Вельямин → Вениамин (старославянская форма того же имени)
    ("вельямин", "вениамин"),
    ("вельяминка", "вениамин"),
    // Аэлин → Аэлира (фэнтезийное имя, краткая форма)
    ("аэлин", "аэлира"),
    // Алексей → Алексей (каноническая форма, без алиасов)
    // Лёша → Алексей
    ("лёша", "алексей"),
    ("лёшка", "алексей"),
    ("лёшенька", "алексей"),
    // Саша → Александр (НЕ Алексей!)
    ("саша", "александр"),
    ("сашка", "александр"),
    ("сашенька", "александр"),
    // Марта — без алиасов (каноническая форма)
    // Маша → Мария
    ("маша", "мария"),
    ("машенька", "мария"),
    ("машка", "мария"),
    // Катя → Екатерина
    ("катя", "екатерина"),
    ("катюша", "екатерина"),
    ("катенька", "екатерина"),
    // Дима → Дмитрий
    ("дима", "дмитрий"),
    ("димка", "дмитрий"),
    ("димочка", "дмитрий"),
    // Петя → Пётр
    ("петя", "пётр"),
    ("петенька", "пётр"),
    ("петюня", "пётр"),
    // v0.4.2: Короткие имена с падежными формами (≤4 символов —
    // lemmatize_simple не справляется, нужны явные алиасы).
    // Рэя / Рэю / Рэем → Рэй (главный герой «1-Сфера Предела»)
    ("рэя", "рэй"),
    ("рэю", "рэй"),
    ("рэем", "рэй"),
    ("рэю", "рэй"),
    // Жора / Жору / Жорой → Жора (каноническая форма)
    ("жору", "жора"),
    ("жорой", "жора"),
    // Паша / Пашу / Пашей → Паша
    ("пашу", "паша"),
    ("пашей", "паша"),
    // Сёма / Сёму / Сёмой → Сёма
    ("сёму", "сёма"),
    ("сёмой", "сёма"),
];

/// v0.4.0: Применить таблицу алиасов к списку персонажей.
///
/// Если lemma персонажа найдена в ALIASES как короткая форма — сливаем
/// его с персонажем, чья lemma = полная форма. Суммируем freq, speech,
/// direct. Каноничным именем становится полная форма.
///
/// Возвращает список без дубликатов. Порядок сохраняется (полная форма
/// остаётся на своей позиции, короткая — удаляется, её данные добавляются
/// к полной).
pub fn merge_aliases(
    chars: Vec<crate::parser::characters::ParsedCharacter>,
) -> Vec<crate::parser::characters::ParsedCharacter> {
    use crate::parser::characters::ParsedCharacter;
    use std::collections::HashMap;

    // Строим map: lemma_короткая → lemma_полная
    let alias_map: HashMap<String, String> = ALIASES
        .iter()
        .map(|(short, full)| (short.to_string(), full.to_string()))
        .collect();

    // Строим map: lemma → индекс в chars (для быстрого поиска)
    let mut lemma_to_idx: HashMap<String, usize> = HashMap::new();
    for (i, c) in chars.iter().enumerate() {
        let lemma = lemmatize_simple(&c.name);
        // Сохраняем ПЕРВОЕ вхождение каждой lemma
        lemma_to_idx.entry(lemma).or_insert(i);
    }

    // Проходим по всем персонажам. Если lemma короткая и есть алиас на полную —
    // переносим данные в полную форму и помечаем короткую на удаление.
    let mut to_remove: HashSet<usize> = HashSet::new();
    let mut updates: HashMap<usize, (usize, usize, usize, Vec<String>)> = HashMap::new();
    // updates[target_idx] = (add_count, add_speech, add_direct, add_aliases)

    for (i, c) in chars.iter().enumerate() {
        let lemma = lemmatize_simple(&c.name);
        if let Some(full_lemma) = alias_map.get(&lemma) {
            if let Some(&target_idx) = lemma_to_idx.get(full_lemma) {
                if target_idx == i {
                    continue; // не сливаем сам с собой
                }
                // Переносим данные из короткой (i) в полную (target_idx)
                let entry = updates.entry(target_idx).or_insert((0, 0, 0, Vec::new()));
                entry.0 += c.count;
                entry.1 += c.speech_count;
                entry.2 += c.direct_count;
                entry.3.extend(c.aliases.iter().cloned());
                entry.3.push(c.name.clone());
                to_remove.insert(i);
            }
        }
    }

    // Применяем обновления
    let mut result: Vec<ParsedCharacter> = chars
        .into_iter()
        .enumerate()
        .filter_map(|(i, mut c)| {
            if to_remove.contains(&i) {
                None
            } else {
                if let Some((add_count, add_speech, add_direct, add_aliases)) = updates.remove(&i) {
                    c.count += add_count;
                    c.speech_count += add_speech;
                    c.direct_count += add_direct;
                    // Добавляем новые alias'ы (без дубликатов)
                    let existing: HashSet<String> = c.aliases.iter().cloned().collect();
                    for a in add_aliases {
                        if !existing.contains(&a) {
                            c.aliases.push(a);
                        }
                    }
                    // Обновляем reason
                    let lemma = lemmatize_simple(&c.name);
                    let forms_preview: Vec<String> =
                        c.aliases.iter().take(4).cloned().collect();
                    c.reason = format!(
                        "character:rule=linguistic_signal+alias_merge;freq={};speech_verb_hits={};direct_address_hits={};lemma={};ALIAS_MERGED;forms=[{}]",
                        c.count, c.speech_count, c.direct_count, lemma, forms_preview.join(",")
                    );
                }
                Some(c)
            }
        })
        .collect();

    // Пересортируем по убыванию частоты
    result.sort_by(|a, b| b.count.cmp(&a.count));
    result.truncate(25);
    result
}

// ════════════════════════════════════════════════════════════════
//  v0.5.0: LanguageTool-weights — украинская лемматизация,
//         расширенные ALIASES, падежные предикаты
//  Автоматически сгенерировано из:
//    /home/z/my-project/scripts/expand_linguistic_weights.py
// ════════════════════════════════════════════════════════════════

/// Расширенная таблица алиасов для русских/украинских/славянских имён.
///
/// Дополнение к [`ALIASES`]: содержит ~150 пар (краткая → полная форма)
/// для самых частых русских, украинских и общих славянских имён.
/// Включает уменьшительные, ласкательные и разговорные формы.
pub const EXTENDED_ALIASES: &[(&str, &str)] = &[
    ("веня", "вениамин"),
    ("венька", "вениамин"),
    ("венечка", "вениамин"),
    ("вельямин", "вениамин"),
    ("вельяминка", "вениамин"),
    ("лёша", "алексей"),
    ("лёшка", "алексей"),
    ("лёшенька", "алексей"),
    ("лёня", "леонид"),
    ("лёнька", "леонид"),
    ("саша", "александр"),
    ("сашка", "александр"),
    ("сашенька", "александр"),
    ("шура", "александр"),
    ("шурочка", "александр"),
    ("маша", "марія"),
    ("машенька", "мария"),
    ("машка", "мария"),
    ("маня", "мария"),
    ("манечка", "мария"),
    ("катя", "екатерина"),
    ("катюша", "екатерина"),
    ("катенька", "екатерина"),
    ("катюня", "екатерина"),
    ("дима", "дмитрий"),
    ("димка", "дмитрий"),
    ("димочка", "дмитрий"),
    ("митя", "дмитрий"),
    ("митенька", "дмитрий"),
    ("петя", "пётр"),
    ("петенька", "пётр"),
    ("петюня", "пётр"),
    ("петруха", "пётр"),
    ("ваня", "іван"),
    ("ванька", "иван"),
    ("ванюша", "иван"),
    ("ивашка", "иван"),
    ("ванечка", "иван"),
    ("боря", "борис"),
    ("борька", "борис"),
    ("боренька", "борис"),
    ("коля", "микола"),
    ("колька", "николай"),
    ("коленька", "николай"),
    ("николка", "николай"),
    ("юра", "юрій"),
    ("юрка", "юрий"),
    ("юрчик", "юрий"),
    ("юрочка", "юрий"),
    ("гена", "геннадий"),
    ("генка", "геннадий"),
    ("жора", "егор"),
    ("жорка", "егор"),
    ("жорочка", "егор"),
    ("егор", "егорий"),
    ("егорка", "егорий"),
    ("стёпа", "степан"),
    ("стёпка", "степан"),
    ("степочка", "степан"),
    ("костя", "константин"),
    ("костька", "константин"),
    ("костик", "константин"),
    ("миша", "михаил"),
    ("мишка", "михаил"),
    ("мишенька", "михаил"),
    ("мичка", "михаил"),
    ("андрюша", "андрей"),
    ("андрюшка", "андрей"),
    ("андрейка", "андрей"),
    ("серёжа", "сергей"),
    ("серёжка", "сергей"),
    ("серёженька", "сергей"),
    ("серый", "сергей"),
    ("витя", "виталий"),
    ("витёк", "виталий"),
    ("витенька", "виталий"),
    ("виталя", "виталий"),
    ("оля", "ольга"),
    ("олька", "ольга"),
    ("оленька", "ольга"),
    ("олюня", "ольга"),
    ("люся", "ольга"),
    ("лена", "елена"),
    ("ленка", "елена"),
    ("леночка", "елена"),
    ("алёна", "елена"),
    ("алёнушка", "елена"),
    ("таня", "тетяна"),
    ("танька", "татьяна"),
    ("танюша", "тетяна"),
    ("танечка", "татьяна"),
    ("нюра", "анна"),
    ("нюся", "анна"),
    ("анечка", "анна"),
    ("аннушка", "анна"),
    ("анка", "анна"),
    ("валя", "валентин"),
    ("валька", "валентин"),
    ("валенька", "валентин"),
    ("юля", "юлия"),
    ("юлька", "юлия"),
    ("юлечка", "юлия"),
    ("света", "светлана"),
    ("светка", "светлана"),
    ("светочка", "светлана"),
    ("наташа", "наталия"),
    ("ната", "наталия"),
    ("наташка", "наталия"),
    ("наточка", "наталия"),
    ("натуля", "наталия"),
    ("вера", "вера"),
    ("верочка", "вера"),
    ("надя", "надія"),
    ("наденька", "надежда"),
    ("надюша", "надежда"),
    ("люда", "людмила"),
    ("людочка", "людмила"),
    ("людка", "людмила"),
    ("мила", "людмила"),
    ("галя", "галина"),
    ("галька", "галина"),
    ("галочка", "галина"),
    ("галюня", "галина"),
    ("зина", "зинаида"),
    ("зинка", "зинаида"),
    ("зиночка", "зинаида"),
    ("соня", "софья"),
    ("сонька", "софья"),
    ("сонечка", "софья"),
    ("андрій", "андрій"),
    ("андрійко", "андрій"),
    ("богдан", "богдан"),
    ("богданко", "богдан"),
    ("бохдан", "богдан"),
    ("бодя", "богдан"),
    ("василь", "василь"),
    ("василько", "василь"),
    ("віталій", "віталій"),
    ("вітась", "віталій"),
    ("денис", "денис"),
    ("дениско", "денис"),
    ("дмитро", "дмитро"),
    ("дмитрик", "дмитро"),
    ("михайло", "михайло"),
    ("михайлик", "михайло"),
    ("микола", "микола"),
    ("миколка", "микола"),
    ("остап", "остап"),
    ("остапко", "остап"),
    ("петро", "петро"),
    ("петрик", "петро"),
    ("тарас", "тарас"),
    ("тарасик", "тарас"),
    ("юрій", "юрій"),
    ("юрко", "юрій"),
    ("ярослав", "ярослав"),
    ("ярик", "ярослав"),
    ("іван", "іван"),
    ("вано", "іван"),
    ("оксана", "оксана"),
    ("оксанка", "оксана"),
    ("оксаночка", "оксана"),
    ("соля", "оксана"),
    ("тетяна", "тетяна"),
    ("татьяна", "тетяна"),
    ("ольга", "ольга"),
    ("надія", "надія"),
    ("любов", "любов"),
    ("люба", "любов"),
    ("любочка", "любов"),
    ("марія", "марія"),
    ("марійка", "марія"),
    ("маруся", "марія"),
    ("галина", "галина"),
    ("дарина", "дарина"),
    ("даринка", "дарина"),
    ("дара", "дарина"),
    ("богдана", "богдана"),
    ("бодяна", "богдана"),
    ("устина", "устина"),
    ("аэлин", "аэлира"),
    ("рэя", "рэй"),
    ("рэю", "рэй"),
    ("рэем", "рэй"),
    ("жору", "жора"),
    ("жорой", "жора"),
    ("пашу", "паша"),
    ("пашей", "паша"),
    ("сёму", "сёма"),
    ("сёмой", "сёма"),
    ("ильич", "ильич"),
    ("ильинична", "ильинична"),
    ("петрович", "петрович"),
    ("петровна", "петровна"),
    ("сергеевич", "сергеевич"),
    ("сергеевна", "сергеевна"),
    ("николаевич", "николаевич"),
    ("николаевна", "николаевна"),
    ("алексеевич", "алексеевич"),
    ("алексеевна", "алексеевна"),
    ("иванович", "иванович"),
    ("ивановна", "ивановна"),
    ("владимирович", "владимирович"),
    ("владимировна", "владимировна"),
];

/// Объединённая таблица алиасов: [`ALIASES`] + [`EXTENDED_ALIASES`].
/// Используется в `merge_aliases_extended()`.
pub fn all_aliases() -> &'static [(&'static str, &'static str)] {
    // Возвращаем EXTENDED_ALIASES — он уже содержит все пары из ALIASES
    // плюс расширение. ALIASES оставлен для обратной совместимости.
    EXTENDED_ALIASES
}

/// Простая лемматизация украинского слова через отсечение типичных
/// окончаний. Параллель к [`lemmatize_simple`] для украинского языка.
///
/// # Поддерживаемые окончания
///
/// | Тип | Окончания |
/// |-----|----------|
/// | Род. мн. | -ів, -ів |
/// | Дав. мн. | -ам, -ям |
/// | Оруд. мн. | -ами, -ями |
/// | Місц. мн. | -ах, -ях |
/// | Род. од. | -и, -і, -а, -я |
/// | Дав. од. | -у, -ю, -і |
/// | Знах. од. | -у, -ю |
/// | Оруд. од. | -ою, -ею, -ом, -ем |
/// | Місц. од. | -і, -у |
pub fn lemmatize_ukrainian(word: &str) -> String {
    let lower = word.to_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    if chars.len() <= 4 {
        return lower;
    }
    // Украинские окончания (попытка отсечь самое длинное совпадение)
    let endings: &[&str] = &[
        "ами", "ями", "ів", "ів", "ові", "еві", "ом", "ем", "ах", "ях", "ою", "ею", "ми", "а", "я",
        "у", "ю", "и", "і", "е", "о",
    ];
    for ending in endings {
        let ending_chars: Vec<char> = ending.chars().collect();
        if chars.len() > ending_chars.len() + 2 {
            let tail_start = chars.len() - ending_chars.len();
            if chars[tail_start..] == ending_chars[..] {
                return chars[..tail_start].iter().collect();
            }
        }
    }
    lower
}

/// Генерирует падежные формы украинских имён собственных.
/// Используется в `EntityResolver`-эквиваленте для украинского текста.
pub fn generate_ukrainian_declensions(name: &str) -> Vec<String> {
    let name_trim = name.trim();
    if name_trim.is_empty() {
        return Vec::new();
    }
    let lc = name_trim.to_lowercase();
    let chars: Vec<char> = lc.chars().collect();
    let len = chars.len();
    if len < 2 {
        return vec![lc];
    }
    let mut forms = Vec::new();
    if lc.ends_with("ія") && len > 3 {
        let stem: String = chars[..len - 2].iter().collect();
        forms.push(format!("{}ії", stem));
        forms.push(format!("{}ію", stem));
        forms.push(format!("{}ією", stem));
    } else if lc.ends_with('а') {
        let stem: String = chars[..len - 1].iter().collect();
        forms.push(format!("{}и", stem));
        forms.push(format!("{}і", stem));
        forms.push(format!("{}у", stem));
        forms.push(format!("{}ою", stem));
    } else if lc.ends_with('я') {
        let stem: String = chars[..len - 1].iter().collect();
        forms.push(format!("{}і", stem));
        forms.push(format!("{}ю", stem));
        forms.push(format!("{}ею", stem));
    } else if lc.ends_with('й') {
        let stem: String = chars[..len - 1].iter().collect();
        forms.push(format!("{}я", stem));
        forms.push(format!("{}ю", stem));
        forms.push(format!("{}єм", stem));
        forms.push(format!("{}ї", stem));
    } else if lc.ends_with('ь') {
        let stem: String = chars[..len - 1].iter().collect();
        forms.push(format!("{}я", stem));
        forms.push(format!("{}ю", stem));
        forms.push(format!("{}ем", stem));
        forms.push(format!("{}і", stem));
    } else {
        let last = chars[len - 1];
        if "бвгджзклмнпрстфхцчшщ".contains(last) {
            forms.push(format!("{}а", lc));
            forms.push(format!("{}у", lc));
            forms.push(format!("{}ом", lc));
            forms.push(format!("{}і", lc));
            forms.push(format!("{}е", lc));
        }
    }
    forms.sort();
    forms.dedup();
    forms
}

/// Проверяет, может ли слово быть в русском родительном падеже.
/// Эвристика по окончанию: -а, -я, -ы, -и, -ов, -ев, -ам, -ям.
pub fn looks_like_russian_genitive(word: &str) -> bool {
    let lc = word.to_lowercase();
    let endings = [
        "ова", "ева", "ина", "яна", "овы", "евы", "ами", "ями", "ов", "ев", "ин", "ын", "а", "я",
        "ы", "и",
    ];
    endings
        .iter()
        .any(|e| lc.ends_with(e) && lc.len() > e.len() + 2)
}

/// Проверяет, может ли слово быть в русском дательном падеже.
/// Эвристика по окончанию: -у, -ю, -ам, -ям.
pub fn looks_like_russian_dative(word: &str) -> bool {
    let lc = word.to_lowercase();
    let endings = ["ам", "ям", "у", "ю"];
    endings
        .iter()
        .any(|e| lc.ends_with(e) && lc.len() > e.len() + 2)
}

/// Проверяет, может ли слово быть в русском творительном падеже.
/// Эвристика по окончанию: -ом, -ем, -ами, -ями, -ой, -ей, -ью.
pub fn looks_like_russian_instrumental(word: &str) -> bool {
    let lc = word.to_lowercase();
    let endings = ["ами", "ями", "ом", "ем", "ой", "ей", "ью"];
    endings
        .iter()
        .any(|e| lc.ends_with(e) && lc.len() > e.len() + 2)
}

/// Проверяет, может ли слово быть в русском предложном падеже.
/// Эвристика по окончанию: -е, -и, -у, -ю (после предлогов в/на/о/при).
pub fn looks_like_russian_prepositional(word: &str) -> bool {
    let lc = word.to_lowercase();
    // Предложный падеж труднее всего определить по окончанию —
    // он омонимичен с дательным (-у) и именительным (-е).
    // Используем только -и как надёжный маркер (после предлога).
    lc.ends_with("и") && lc.len() > 4
}

// ─── Wrappers для crate::languagetool_weights ──────────────────

/// Проверяет, содержит ли текст русскую тавтологию.
pub fn find_russian_tautology(text: &str) -> Option<&'static str> {
    crate::languagetool_weights::find_russian_tautology(text)
}

/// Проверяет, содержит ли текст украинский варваризм.
pub fn find_ukrainian_barbarism(text: &str) -> Option<&'static str> {
    crate::languagetool_weights::ukrainian_barbarism_fix(text)
}

/// Возвращает корректный русский вариант для паронима.
pub fn russian_paronym_correction(word: &str) -> Option<&'static str> {
    crate::languagetool_weights::russian_paronym_correction(word)
}

/// Возвращает корректный вариант русской коллокации.
pub fn russian_collocation_correction(text: &str) -> Option<&'static str> {
    crate::languagetool_weights::russian_collocation_fix(text)
}

/// Возвращает количество правил в LanguageTool-таблицах.
pub fn languagetool_rules_count() -> (usize, usize) {
    (
        crate::languagetool_weights::russian_rules_count(),
        crate::languagetool_weights::ukrainian_rules_count(),
    )
}

// =========================================================================
// Полные таблицы падежных окончаний русских существительных
// =========================================================================
//
// В русском языке 6 падежей × 3 рода × 2 числа = 36 ячеек парадигмы.
// Эта таблица содержит типовые окончания для каждого сочетания.
// Источник: грамматика русского языка (Зализняк-77).
//
// Используется функцией `detect_russian_case_by_ending()` для определения
// падежа словоформы по её окончанию. Это упрощённый эвристический подход
// (без учёта морфонологических чередований и исключений), но он даёт
// разумное первое приближение для парсера.

/// Падежная система русского языка.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RussianCase {
    /// Именительный (Nominative): кто? что? — Иван, дом, мама.
    Nominative,
    /// Родительный (Genitive): кого? чего? — Ивана, дома, мамы.
    Genitive,
    /// Дательный (Dative): кому? чему? — Ивану, дому, маме.
    Dative,
    /// Винительный (Accusative): кого? что? — Ивана, дом, маму.
    Accusative,
    /// Творительный (Instrumental): кем? чем? — Иваном, домом, мамой.
    Instrumental,
    /// Предложный (Prepositional): о ком? о чём? — Иване, доме, маме.
    Prepositional,
}

/// Род существительного.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RussianGender {
    /// Мужской: стол, Иван, герой.
    Masculine,
    /// Женский: мама, сестра, ночь.
    Feminine,
    /// Средний: окно, поле, море.
    Neuter,
}

/// Число существительного.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RussianNumber {
    Singular,
    Plural,
}

/// Таблица типовых падежных окончаний для русских существительных.
/// Формат: (род, число, падеж, окончание, пример).
///
/// Пустая строка `""` означает нулевое окончание (стол→стол, окно→окн-о,
/// мать→мать).
pub const RUSSIAN_NOUN_CASE_ENDINGS: &[(RussianGender, RussianNumber, RussianCase, &str, &str)] = &[
    // ─── Единственное число ──────────────────────────────────────
    // Мужской род (твёрдая основа: стол, дом, Иван)
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "",
        "стол",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Genitive,
        "а",
        "стола",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Dative,
        "у",
        "столу",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Accusative,
        "",
        "стол",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Instrumental,
        "ом",
        "столом",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Prepositional,
        "е",
        "столе",
    ),
    // Мужской род (мягкая основа: герой, Иван-ий, календарь)
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "ь",
        "календарь",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "й",
        "герой",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Genitive,
        "я",
        "героя",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Dative,
        "ю",
        "герою",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Instrumental,
        "ем",
        "героем",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Singular,
        RussianCase::Prepositional,
        "е",
        "герое",
    ),
    // Женский род (твёрдая основа: мама, сестра, жена)
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "а",
        "мама",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Genitive,
        "ы",
        "мамы",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Dative,
        "е",
        "маме",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Accusative,
        "у",
        "маму",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Instrumental,
        "ой",
        "мамой",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Prepositional,
        "е",
        "маме",
    ),
    // Женский род (мягкая основа: ночь, мать, площадь)
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "ь",
        "ночь",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Genitive,
        "и",
        "ночи",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Dative,
        "и",
        "ночи",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Accusative,
        "ь",
        "ночь",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Instrumental,
        "ью",
        "ночью",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Prepositional,
        "и",
        "ночи",
    ),
    // Женский род (на -ия: Мария, линия, станция)
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "ия",
        "Мария",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Genitive,
        "ии",
        "Марии",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Dative,
        "ии",
        "Марии",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Accusative,
        "ию",
        "Марию",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Instrumental,
        "ией",
        "Марией",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Singular,
        RussianCase::Prepositional,
        "ии",
        "Марии",
    ),
    // Средний род (твёрдая основа: окно, дело, слово)
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "о",
        "окно",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Genitive,
        "а",
        "окна",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Dative,
        "у",
        "окну",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Accusative,
        "о",
        "окно",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Instrumental,
        "ом",
        "окном",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Prepositional,
        "е",
        "окне",
    ),
    // Средний род (мягкая основа: поле, море, счастье)
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "е",
        "море",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Genitive,
        "я",
        "моря",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Dative,
        "ю",
        "морю",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Accusative,
        "е",
        "море",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Instrumental,
        "ем",
        "морем",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Prepositional,
        "е",
        "море",
    ),
    // Средний род (на -ие: здание, желание, упоминание)
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Nominative,
        "ие",
        "здание",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Genitive,
        "ия",
        "здания",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Dative,
        "ию",
        "зданию",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Accusative,
        "ие",
        "здание",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Instrumental,
        "ием",
        "зданием",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Singular,
        RussianCase::Prepositional,
        "ии",
        "здании",
    ),
    // ─── Множественное число ─────────────────────────────────────
    // Мужской род (твёрдая основа)
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Nominative,
        "ы",
        "столы",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Genitive,
        "ов",
        "столов",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Dative,
        "ам",
        "столам",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Accusative,
        "ы",
        "столы",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Instrumental,
        "ами",
        "столами",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Prepositional,
        "ах",
        "столах",
    ),
    // Мужской род (мягкая основа)
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Nominative,
        "и",
        "герои",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Genitive,
        "ев",
        "героев",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Dative,
        "ям",
        "героям",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Accusative,
        "и",
        "герои",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Instrumental,
        "ями",
        "героями",
    ),
    (
        RussianGender::Masculine,
        RussianNumber::Plural,
        RussianCase::Prepositional,
        "ях",
        "героях",
    ),
    // Женский род (твёрдая основа)
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Nominative,
        "ы",
        "мамы",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Genitive,
        "",
        "мам",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Dative,
        "ам",
        "мамам",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Accusative,
        "",
        "мам",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Instrumental,
        "ами",
        "мамами",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Prepositional,
        "ах",
        "мамах",
    ),
    // Женский род (мягкая основа: -я → -и)
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Nominative,
        "и",
        "ночи",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Genitive,
        "ей",
        "ночей",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Dative,
        "ям",
        "ночам",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Instrumental,
        "ями",
        "ночами",
    ),
    (
        RussianGender::Feminine,
        RussianNumber::Plural,
        RussianCase::Prepositional,
        "ях",
        "ночах",
    ),
    // Средний род
    (
        RussianGender::Neuter,
        RussianNumber::Plural,
        RussianCase::Nominative,
        "а",
        "окна",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Plural,
        RussianCase::Genitive,
        "",
        "окн",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Plural,
        RussianCase::Dative,
        "ам",
        "окнам",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Plural,
        RussianCase::Accusative,
        "а",
        "окна",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Plural,
        RussianCase::Instrumental,
        "ами",
        "окнами",
    ),
    (
        RussianGender::Neuter,
        RussianNumber::Plural,
        RussianCase::Prepositional,
        "ах",
        "окнах",
    ),
];

/// Пытается определить падеж русского слова по его окончанию.
///
/// Это эвристический подход: возвращает первый найденный падеж для данного
/// окончания среди всех (род × число) комбинаций. Для точного определения
/// нужно знать род и число — в этом случае используйте
/// `detect_russian_case_with_gender_number()`.
///
/// # Примеры
///
/// ```rust,ignore
/// assert_eq!(detect_russian_case_by_ending("маму"), Some(RussianCase::Accusative));
/// assert_eq!(detect_russian_case_by_ending("Иване"), Some(RussianCase::Prepositional));
/// assert_eq!(detect_russian_case_by_ending("столами"), Some(RussianCase::Instrumental));
/// ```
pub fn detect_russian_case_by_ending(word: &str) -> Option<RussianCase> {
    let w = word.trim().to_lowercase();
    if w.is_empty() {
        return None;
    }
    // Идём от самых длинных окончаний к коротким — чтобы «-ия» совпало
    // раньше, чем «-я».
    let mut best: Option<(usize, RussianCase)> = None;
    for (_, _, case, ending, _) in RUSSIAN_NOUN_CASE_ENDINGS {
        if ending.is_empty() {
            continue;
        }
        if w.ends_with(ending) {
            let len = ending.chars().count();
            match best {
                None => best = Some((len, *case)),
                Some((cur, _)) if len > cur => best = Some((len, *case)),
                _ => {}
            }
        }
    }
    best.map(|(_, c)| c)
}

/// Пытается определить падеж русского слова с учётом рода и числа.
///
/// Более точная версия `detect_russian_case_by_ending()`: фильтрует
/// таблицу окончаний по указанным роду и числу, затем ищет совпадение.
pub fn detect_russian_case_with_gender_number(
    word: &str,
    gender: RussianGender,
    number: RussianNumber,
) -> Option<RussianCase> {
    let w = word.trim().to_lowercase();
    if w.is_empty() {
        return None;
    }
    let mut best: Option<(usize, RussianCase)> = None;
    for (g, n, case, ending, _) in RUSSIAN_NOUN_CASE_ENDINGS {
        if *g != gender || *n != number {
            continue;
        }
        if ending.is_empty() {
            continue;
        }
        if w.ends_with(ending) {
            let len = ending.chars().count();
            match best {
                None => best = Some((len, *case)),
                Some((cur, _)) if len > cur => best = Some((len, *case)),
                _ => {}
            }
        }
    }
    best.map(|(_, c)| c)
}

// =========================================================================
// Таблица русских↔украинских когнатных пар
// =========================================================================
//
// Когнаты — слова в родственных языках, имеющие общее происхождение.
// Для русского и украинского (оба — восточнославянские) многие
// литературные имена и топонимы имеют когнатные пары. Эта таблица
// используется функцией `find_cognate_pair()` для перекрёстного
// разрешения сущностей: если в тексте упоминается «Іван», а в графе
// уже есть «Иван», их следует связать как одну сущность.
//
// Таблица пополняема — это «веса» программы, а не исчерпывающий словарь.

/// Пара когнатов: (русская форма, украинская форма).
pub const RU_UK_COGNATE_PAIRS: &[(&str, &str)] = &[
    // Имена
    ("Иван", "Іван"),
    ("Пётр", "Петро"),
    ("Павел", "Павло"),
    ("Илья", "Ілля"),
    ("Александр", "Олександр"),
    ("Алексей", "Олексій"),
    ("Николай", "Микола"),
    ("Дмитрий", "Дмитро"),
    ("Сергей", "Сергій"),
    ("Андрей", "Андрій"),
    ("Михаил", "Михайло"),
    ("Владимир", "Володимир"),
    ("Виктор", "Віктор"),
    ("Юрий", "Юрій"),
    ("Анатолий", "Анатолій"),
    ("Константин", "Костянтин"),
    ("Григорий", "Григорій"),
    ("Степан", "Степан"),
    ("Богдан", "Богдан"),
    ("Анна", "Ганна"),
    ("Мария", "Марія"),
    ("Елена", "Олена"),
    ("Ольга", "Ольга"),
    ("Татьяна", "Тетяна"),
    ("Наталья", "Наталія"),
    ("Светлана", "Світлана"),
    ("Ирина", "Ірина"),
    ("Катерина", "Катерина"),
    ("Юлия", "Юлія"),
    ("Любовь", "Любов"),
    ("Вера", "Віра"),
    ("Надежда", "Надія"),
    ("Софья", "Софія"),
    ("Анастасия", "Анастасія"),
    ("Дарья", "Дарина"),
    ("Полина", "Павліна"),
    ("Валентина", "Валентина"),
    // Топонимы
    ("Москва", "Москва"),
    ("Киев", "Київ"),
    ("Петербург", "Петербург"),
    ("Минск", "Мінськ"),
    ("Одесса", "Одеса"),
    ("Харьков", "Харків"),
    ("Львов", "Львів"),
    ("Днепр", "Дніпро"),
    ("Донецк", "Донецьк"),
    ("Луганск", "Луганськ"),
    ("Запорожье", "Запоріжжя"),
    ("Крым", "Крим"),
    ("Полтава", "Полтава"),
    ("Чернигов", "Чернігів"),
    ("Винница", "Вінниця"),
    ("Житомир", "Житомир"),
    ("Ровно", "Рівне"),
    ("Тернополь", "Тернопіль"),
    ("Хмельницкий", "Хмельницький"),
    ("Черкассы", "Черкаси"),
    ("Черновцы", "Чернівці"),
    ("Ужгород", "Ужгород"),
    ("Сумы", "Суми"),
    ("Николаев", "Миколаїв"),
    ("Херсон", "Херсон"),
    ("Кировоград", "Кіровоград"),
    // Реки
    ("Волга", "Волга"),
    ("Днепр", "Дніпро"),
    ("Дон", "Дон"),
    ("Десна", "Десна"),
    ("Висла", "Вісла"),
    ("Дунай", "Дунай"),
    // Общие существительные (для литературных текстов)
    ("город", "місто"),
    ("деревня", "село"),
    ("дом", "дім"),
    ("улица", "вулиця"),
    ("площадь", "площа"),
    ("мост", "міст"),
    ("церковь", "церква"),
    ("школа", "школа"),
    ("гимназия", "гімназія"),
    ("институт", "інститут"),
    ("университет", "університет"),
    ("театр", "театр"),
    ("музей", "музей"),
    ("библиотека", "бібліотека"),
    ("больница", "лікарня"),
    ("аптека", "аптека"),
    ("почта", "пошта"),
    ("банк", "банк"),
    ("рынок", "ринок"),
    ("магазин", "магазин"),
    ("кафе", "кафе"),
    ("ресторан", "ресторан"),
    ("вокзал", "вокзал"),
    ("станция", "станція"),
    ("поезд", "потяг"),
    ("автомобиль", "автомобіль"),
    ("машина", "машина"),
    ("трамвай", "трамвай"),
    ("троллейбус", "тролейбус"),
    ("метро", "метро"),
    ("самолёт", "літак"),
    ("корабль", "корабель"),
    ("лодка", "човен"),
    ("велосипед", "велосипед"),
    // Природные объекты
    ("лес", "ліс"),
    ("поле", "поле"),
    ("река", "ріка"),
    ("озеро", "озеро"),
    ("море", "море"),
    ("гора", "гора"),
    ("долина", "долина"),
    ("холм", "пагорб"),
    ("пустыня", "пустеля"),
    ("остров", "острів"),
    ("полуостров", "півострів"),
    ("берег", "берег"),
    ("пляж", "пляж"),
    ("скала", "скеля"),
    ("пещера", "печера"),
    ("водопад", "водоспад"),
    // Временные понятия
    ("утро", "ранок"),
    ("день", "день"),
    ("вечер", "вечір"),
    ("ночь", "ніч"),
    ("неделя", "тиждень"),
    ("месяц", "місяць"),
    ("год", "рік"),
    ("век", "вік"),
    ("зима", "зима"),
    ("весна", "весна"),
    ("лето", "літо"),
    ("осень", "осінь"),
];

/// Ищет украинский когнат для русского слова (или наоборот).
///
/// Возвращает `Some(форму_на_другом_языке)` если слово найдено в
/// таблице когнатов. Поиск — case-insensitive (нормализует к lowercase).
///
/// # Примеры
///
/// ```rust,ignore
/// assert_eq!(find_cognate_pair("Иван"), Some("Іван"));
/// assert_eq!(find_cognate_pair("іван"), Some("иван"));  // обратный поиск
/// assert_eq!(find_cognate_pair("xyz"), None);
/// ```
pub fn find_cognate_pair(word: &str) -> Option<&'static str> {
    if let Some((target, _, _)) = crate::dict::cognate::normalize_token(word) {
        return Some(target);
    }
    let w = word.trim().to_lowercase();
    if w.is_empty() {
        return None;
    }
    for (ru, uk) in RU_UK_COGNATE_PAIRS {
        if ru.to_lowercase() == w {
            return Some(uk);
        }
        if uk.to_lowercase() == w {
            return Some(ru);
        }
    }
    None
}

/// Проверяет, является ли слово русским именем (по таблице когнатов).
pub fn is_ru_known_name(word: &str) -> bool {
    let w = word.trim().to_lowercase();
    if w.is_empty() {
        return false;
    }
    if crate::dict::cognate::normalize_token(&w).is_some() {
        return true;
    }
    RU_UK_COGNATE_PAIRS
        .iter()
        .any(|(ru, _)| ru.to_lowercase() == w)
}

/// Проверяет, является ли слово украинским именем (по таблице когнатов).
pub fn is_uk_known_name(word: &str) -> bool {
    let w = word.trim().to_lowercase();
    if w.is_empty() {
        return false;
    }
    if crate::dict::cognate::normalize_token(&w).is_some() {
        return true;
    }
    RU_UK_COGNATE_PAIRS
        .iter()
        .any(|(_, uk)| uk.to_lowercase() == w)
}

// =========================================================================
// Wrappers for crate::linguistic_entities (flat replacement tables)
// =========================================================================

/// Ищет русскую замену для слова в плоской таблице LanguageTool.
pub fn find_ru_replacement(word: &str) -> Option<&'static [&'static str]> {
    crate::linguistic_entities::find_ru_replacement(word)
}

/// Ищет украинскую замену для слова в плоской таблице LanguageTool.
pub fn find_uk_replacement(word: &str) -> Option<&'static [&'static str]> {
    crate::linguistic_entities::find_uk_replacement(word)
}

/// Ищет тавтологию вида «корень родственное_слово» в тексте.
pub fn find_ru_word_root_tautology(text_lower: &str) -> Option<&'static str> {
    crate::linguistic_entities::find_ru_word_root_tautology(text_lower)
}

/// Проверяет, является ли слово днём недели (RU).
pub fn is_ru_weekday(word: &str) -> bool {
    crate::linguistic_entities::is_ru_weekday(word)
}

/// Проверяет, является ли слово месяцем (RU).
pub fn is_ru_month(word: &str) -> bool {
    crate::linguistic_entities::is_ru_month(word)
}

/// Проверяет, является ли слово профессией (RU).
pub fn is_ru_profession(word: &str) -> bool {
    crate::linguistic_entities::is_ru_profession(word)
}

/// Проверяет, является ли слово цветом (RU).
pub fn is_ru_color(word: &str) -> bool {
    crate::linguistic_entities::is_ru_color(word)
}

/// Проверяет, является ли слово национальностью (RU).
pub fn is_ru_nation(word: &str) -> bool {
    crate::linguistic_entities::is_ru_nation(word)
}

/// Проверяет, является ли слово качеством человека (RU).
pub fn is_ru_human_quality(word: &str) -> bool {
    crate::linguistic_entities::is_ru_human_quality(word)
}

/// Проверяет, является ли слово вводным (RU).
pub fn is_ru_vvodnoe(word: &str) -> bool {
    crate::linguistic_entities::is_ru_vvodnoe(word)
}

/// Проверяет, является ли слово днём недели (UK).
pub fn is_uk_weekday(word: &str) -> bool {
    crate::ukrainian_semantic_categories::is_uk_weekday(word)
}

/// Проверяет, является ли слово месяцем (UK).
pub fn is_uk_month(word: &str) -> bool {
    crate::ukrainian_semantic_categories::is_uk_month(word)
}

/// Проверяет, является ли слово профессией (UK).
pub fn is_uk_profession(word: &str) -> bool {
    crate::ukrainian_semantic_categories::is_uk_profession(word)
}

/// Проверяет, является ли слово цветом (UK).
pub fn is_uk_color(word: &str) -> bool {
    crate::ukrainian_semantic_categories::is_uk_color(word)
}

/// Проверяет, является ли слово национальностью (UK).
pub fn is_uk_nation(word: &str) -> bool {
    crate::ukrainian_semantic_categories::is_uk_nation(word)
}

/// Проверяет, является ли слово качеством человека (UK).
pub fn is_uk_human_quality(word: &str) -> bool {
    crate::ukrainian_semantic_categories::is_uk_human_quality(word)
}

/// Проверяет, является ли слово вводным (UK).
pub fn is_uk_vvodnoe(word: &str) -> bool {
    crate::ukrainian_semantic_categories::is_uk_vvodnoe(word)
}

/// Ищет украинский пароним и возвращает нормализованную форму.
pub fn ukrainian_paronym_correction(word: &str) -> Option<&'static str> {
    crate::ukrainian_semantic_categories::ukrainian_paronym_correct(word)
}

/// Возвращает суммарное количество записей в таблицах замен.
pub fn total_replacement_entries() -> usize {
    crate::linguistic_entities::total_replacement_entries()
}

/// Возвращает количество корневых пар в таблице `RU_WORD_ROOTS`.
pub fn total_word_root_entries() -> usize {
    crate::linguistic_entities::total_word_root_entries()
}
