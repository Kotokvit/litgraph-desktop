//! Автопарсер .md → граф.
//! Переписано с src/app/api/parse-md/route.ts (705 строк TS).

pub mod chapters;
pub mod characters;
pub mod locations;
// pub mod themes; // убран — не нужен
pub mod epsilon;

use crate::models::{LitEdge, LitNode, LitNodeData, ParseResult, ParseStats, Position};
use chrono::Utc;
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
