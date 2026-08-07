//! Детекция глав по 9 паттернам.
//! Переписано с detectChapters() из parse-md/route.ts (TS).

use fancy_regex::Regex;
use std::collections::HashMap;

/// Безопасный срез строки по байтам (подгоняет под char boundary UTF-8)
fn safe_slice(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[derive(Debug, Clone)]
pub struct ParsedChapter {
    pub num: u32,
    pub title: String,
    pub body: String,
    pub full_text: String,
    pub pos: usize,
    pub end: usize,
}

/// 9 паттернов для разных форматов заголовков глав
fn patterns() -> Vec<(&'static str, Regex)> {
    vec![
        ("uk-Глава", Regex::new(r"Глава\s+(\d+)").unwrap()),
        ("uk-Розділ", Regex::new(r"Розділ\s+(\d+)").unwrap()),
        ("uk-Частина", Regex::new(r"Частина\s+(\d+)").unwrap()),
        ("en-Chapter", Regex::new(r"Chapter\s+(\d+)").unwrap()),
        ("en-Part", Regex::new(r"Part\s+(\d+)").unwrap()),
        ("ru-Часть", Regex::new(r"Часть\s+(\d+)").unwrap()),
        ("md-hash-num", Regex::new(r"(?m)^#\s+(\d+)[\s.]").unwrap()),
        ("md-hashhash-num", Regex::new(r"(?m)^##\s+(\d+)[\s.]").unwrap()),
        ("md-hash-hash-num", Regex::new(r"(?m)^###\s+(\d+)[\s.]").unwrap()),
    ]
}

pub fn detect(text: &str) -> (Vec<ParsedChapter>, String) {
    let mut best_matches: Vec<(usize, String)> = Vec::new(); // (pos, captured_num)
    let mut best_count = 0;

    for (_name, re) in patterns() {
        let mut matches: Vec<(usize, String)> = Vec::new();
        for caps_result in re.captures_iter(text) {
            let caps = match caps_result {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some(m) = caps.get(0) {
                if let Some(num) = caps.get(1) {
                    matches.push((m.start(), num.as_str().to_string()));
                }
            }
        }
        if matches.len() > best_count {
            best_count = matches.len();
            best_matches = matches;
        }
    }

    // Если ничего не нашли — возвращаем одну "главу" со всем текстом
    if best_matches.is_empty() {
        let body_clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let body_preview = if body_clean.len() > 400 {
            format!("{}…", safe_slice(&body_clean, 400))
        } else {
            body_clean.clone()
        };
        return (
            vec![ParsedChapter {
                num: 1,
                title: "Текст целиком".to_string(),
                body: body_preview,
                full_text: text.to_string(),
                pos: 0,
                end: text.len(),
            }],
            String::new(),
        );
    }

    // Уникальные по номеру (берём первое вхождение)
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut sorted: Vec<(usize, String)> = Vec::new();
    for (pos, num) in &best_matches {
        if !seen.contains_key(num) {
            seen.insert(num.clone(), *pos);
            sorted.push((*pos, num.clone()));
        }
    }
    sorted.sort_by_key(|(pos, _)| *pos);

    let prologue_text = if sorted.is_empty() {
        String::new()
    } else {
        text[..sorted[0].0].to_string()
    };

    let mut chapters: Vec<ParsedChapter> = Vec::new();
    for i in 0..sorted.len() {
        let (pos, num_str) = &sorted[i];
        let num: u32 = num_str.parse().unwrap_or(1);
        let next_pos = if i + 1 < sorted.len() {
            sorted[i + 1].0
        } else {
            text.len()
        };
        let _full_text = &text[*pos..next_pos];

        // Заголовок: текст после "Глава N" (500 символов)
        // Найдём конец match'а "Глава N"
        let match_end = find_match_end(text, *pos, num_str);
        let after_max = match_end + 500;
        let after_end = if after_max > text.len() {
            text.len()
        } else {
            // Подгоним под char boundary
            let mut e = after_max;
            while e > match_end && !text.is_char_boundary(e) {
                e -= 1;
            }
            e
        };
        let after = &text[match_end..after_end];
        let title = extract_title_from_after(after);

        // Тело — без заголовка
        let body_text = &text[match_end..next_pos];
        let body_clean: String = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
        let body_preview = if body_clean.len() > 400 {
            format!("{}…", safe_slice(&body_clean, 400))
        } else {
            body_clean.clone()
        };

        chapters.push(ParsedChapter {
            num,
            title,
            body: body_preview,
            full_text: body_text.trim().to_string(),
            pos: *pos,
            end: next_pos,
        });
    }

    (chapters, prologue_text)
}

/// Найти конец match'а "Глава N" в тексте начиная с pos
fn find_match_end(text: &str, pos: usize, num: &str) -> usize {
    // Ищем num начиная с pos
    let search_text = &text[pos..];
    if let Some(rel_pos) = search_text.find(num) {
        pos + rel_pos + num.len()
    } else {
        pos
    }
}

fn extract_title_from_after(after: &str) -> String {
    // Уберём начальные : - — и пробелы
    let mut cleaned = after.trim_start_matches(|c: char| c.is_whitespace() || c == ':' || c == '-' || c == '—');

    // Уберём начальную скобку "(...)" если есть
    if cleaned.starts_with('(') {
        if let Some(end) = cleaned.find(')') {
            cleaned = &cleaned[end + 1..];
            cleaned = cleaned.trim_start_matches(|c: char| c.is_whitespace() || c == ':');
        }
    }

    // Заголовок — до переноса строки или до границы предложения
    let candidate = if let Some(np) = cleaned.find('\n') {
        if np > 0 && np < 200 {
            &cleaned[..np]
        } else {
            // Поищем границу предложения
            find_sentence_boundary(cleaned, 150)
        }
    } else {
        find_sentence_boundary(cleaned, 150)
    };

    // Очистка от меток
    let mut result = candidate.to_string();
    result = regex_replace(&result, r"\(Робоча назва\)\s*:?\s*", "");
    result = regex_replace(&result, r"\(Виправлена версія\)\s*", "");
    result = regex_replace(&result, r"\(Фінальна версія\)\s*", "");
    result = regex_replace(&result, r"\(ФІНАЛЬНИЙ ТЕКСТ\)\s*", "");
    result = regex_replace(&result, r"\(Відредагована версія\)\s*", "");
    result = regex_replace(&result, r"Глава\s*\d+\s*:?\s*", "");
    result = regex_replace(&result, r"\(Частина\s+[IVX]+\s*[—-]\s*[^)]+\)", "");
    result = regex_replace(&result, r#"\(Арка\s+\d+\s*:\s*"[^"]+"\)"#, "");
    result = regex_replace(&result, r#"\(Арка\s+"[^"]+"\)"#, "");
    result = regex_replace(&result, r"\(Континент\s+[^)]+\)", "");
    result = regex_replace(&result, r"\(Локація:\s*[^)]+\)", "");
    result = regex_replace(&result, r"Місце дії:\s*[^.]+\.?\s*", "");
    result = regex_replace(&result, r"\(Початок\)", "");
    // Сжать пробелы
    result = regex_replace(&result, r"\s+", " ");
    result = result.trim_matches(|c: char| c.is_whitespace() || c == '.' || c == ':' || c == '—' || c == '-').to_string();

    // Ограничить длину
    if result.len() > 70 {
        let cut = safe_slice(&result, 70);
        let last_sep = cut.rfind(',').map(|i| i)
            .max(cut.rfind(" — ").map(|i| i))
            .max(cut.rfind(" - ").map(|i| i));
        if let Some(sep) = last_sep {
            if sep > 30 {
                result = safe_slice(cut, sep).to_string();
            } else {
                result = cut.to_string();
            }
        } else {
            result = cut.to_string();
        }
    }

    if result.is_empty() {
        "Глава".to_string()
    } else {
        result
    }
}

fn find_sentence_boundary(s: &str, max_len: usize) -> &str {
    // Поиск [.!?…] + пробел + заглавная буква
    let re = Regex::new(r"[.!?…]\s+[А-ЯЮЯЩЬЦФВІЇҐЄA-Z]").unwrap();
    if let Ok(Some(m)) = re.find(s) {
        if m.start() < max_len {
            return safe_slice(s, m.start());
        }
    }
    if s.len() > 80 {
        safe_slice(s, 80)
    } else {
        s
    }
}

fn regex_replace(text: &str, pattern: &str, replacement: &str) -> String {
    let re = match Regex::new(pattern) {
        Ok(r) => r,
        Err(_) => return text.to_string(),
    };
    re.replace_all(text, replacement).to_string()
}
