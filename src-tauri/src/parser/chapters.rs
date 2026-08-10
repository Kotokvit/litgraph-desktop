//! Детекция глав по 9 паттернам.
//! Переписано с detectChapters() из parse-md/route.ts (TS).
//!
//! ## v0.4.0 — line-anchored + sub-chapter support
//!
//! ### Корень проблемы мега-глав (диагностика через HTML X-ray export)
//! v0.3.1 использовал `(?i)Глава\s+(\d+)` БЕЗ привязки к началу строки.
//! Это приводило к двум багам:
//!   1. На cover page (строка 2 исходника) все 36 заголовков склеены в
//!      одну строку «ГЛАВА 1 ОДЕССАГЛАВА 2 ДОГОВОР...» — regex матчит
//!      их всех, skip_table_of_contents ломается на сложных обложках.
//!   2. Главы 28-32 в романе «1-Сфера Предела» имеют суб-главы:
//!      «Глава 28», «Глава 28б», «Глава 28в», «Глава 28г». Старый regex
//!      захватывал только цифру → все 4 суб-главы сливались в одну
//!      мега-главу на 20k+ слов.
//!
//! ### Решение v0.4.0
//!   1. Line-anchored regex: `(?im)^\s*(?:#+\s*)?Глава\s+(\d+[а-я]?)`
//!      — требует чтобы «Глава» была в начале строки (с опциональным `#`)
//!   2. Захват цифры + опциональной буквы: `\d+[а-я]?` → «28», «28б",
//!      «28в» распознаются как РАЗНЫЕ главы
//!   3. skip_table_of_contents больше не нужен — line-anchoring сам
//!      отсеивает обложку (где все заголовки на одной строке)
//!   4. Дедупликация по полному num_str (включая букву) — «26» и «26б"
//!      считаются разными главами

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
    /// Суффикс суб-главы: «б» для «Глава 28б», «в» для «28в».
    /// `None` для обычных глав без буквенного суффикса.
    /// P2.1: добавлен для корректной сортировки TemporalAnchor.
    pub suffix: Option<String>,
    pub title: String,
    pub body: String,
    pub full_text: String,
    pub pos: usize,
    pub end: usize,
}

/// 9 паттернов для разных форматов заголовков глав.
///
/// v0.4.0: line-anchored + поддержка суб-глав с буквой (28б, 28в, 28г).
/// Все «текстовые» паттерны (Глава/Розділ/Chapter) теперь требуют `^`
/// (начало строки) и захватывают опциональную кириллическую/латинскую
/// букву после цифры. Это решает:
///   - Мега-главы из-за cover page (заголовки на одной строке не матчатся)
///   - Слияние суб-глав (28 + 28б + 28в + 28г = 4 отдельные главы)
///
/// v0.4.1: Упрощены regex-ы для производительности. fancy-regex с optional
/// group `(?:#+\s*)?` вызывал catastrophic backtracking на 2MB тексте
/// (>60s вместо <100ms). Убраны: `\s*` в начале (заголовки обычно в column 0),
/// `(?:#+\s*)?` (markdown hash детектится отдельным pattern-ом), `\b`
/// (заменён на явную проверку следующего символа через positive lookahead).
fn patterns() -> Vec<(&'static str, Regex)> {
    vec![
        // v0.4.1: Простые line-anchored patterns без lookahead/lookbehind.
        // fancy-regex имеет catastrophic backtracking на сложных patterns
        // с optional groups и lookahead. Решение: максимально простой regex.
        //   (?im) — case-insensitive + multiline
        //   ^Глава — в начале строки
        //   \s+ — пробел
        //   (\d+[а-я]?) — цифра + опциональная буква
        // Никаких \b, никаких (?:...)?, никаких lookahead.
        // Фильтрация «280» vs «28» делается пост-факт: если после числа идёт
        // цифра — это не глава (отбрасываем в коде ниже).
        ("ru-Глава",   Regex::new(r"(?im)^Глава\s+(\d+[а-я]?)").unwrap()),
        ("uk-Розділ",  Regex::new(r"(?im)^Розділ\s+(\d+[а-я]?)").unwrap()),
        ("uk-Частина", Regex::new(r"(?im)^Частина\s+(\d+[а-я]?)").unwrap()),
        ("en-Chapter", Regex::new(r"(?im)^Chapter\s+(\d+[a-z]?)").unwrap()),
        ("en-Part",    Regex::new(r"(?im)^Part\s+(\d+[a-z]?)").unwrap()),
        ("ru-Часть",   Regex::new(r"(?im)^Часть\s+(\d+[а-я]?)").unwrap()),
        // Markdown hash patterns
        ("md-hash-num",       Regex::new(r"(?m)^#\s+(\d+[а-я]?)[\s.]").unwrap()),
        ("md-hashhash-num",   Regex::new(r"(?m)^##\s+(\d+[а-я]?)[\s.]").unwrap()),
        ("md-hash-hash-num",  Regex::new(r"(?m)^###\s+(\d+[а-я]?)[\s.]").unwrap()),
    ]
}

pub fn detect(text: &str) -> (Vec<ParsedChapter>, String) {
    // v0.4.0: skip_table_of_contents убран — line-anchored regex сам
    // отсеивает cover page (где все заголовки склеены в одну строку).
    // Если «Глава 1» не в начале строки — она не матчится.
    let text = text;

    let mut best_matches: Vec<(usize, String)> = Vec::new();
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

    if best_matches.is_empty() {
        // Fallback: если line-anchored regex ничего не нашёл, попробуем
        // старый mobile regex (без ^) — вдруг файл использует необычный
        // формат. Это сохраняет обратную совместимость.
        let text_for_fallback = text;
        let fallback_re = Regex::new(r"(?i)Глава\s+(\d+)\b").unwrap();
        let mut fallback_matches: Vec<(usize, String)> = Vec::new();
        for caps_result in fallback_re.captures_iter(text_for_fallback) {
            if let Ok(caps) = caps_result {
                if let Some(m) = caps.get(0) {
                    if let Some(num) = caps.get(1) {
                        fallback_matches.push((m.start(), num.as_str().to_string()));
                    }
                }
            }
        }
        if !fallback_matches.is_empty() {
            best_matches = fallback_matches;
        }
    }

    if best_matches.is_empty() {
        let body_clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let body_preview = if body_clean.len() > 400 {
            format!("{}\u{2026}", safe_slice(&body_clean, 400))
        } else {
            body_clean.clone()
        };
        return (
            vec![ParsedChapter {
                num: 1,
                suffix: None,
                title: "Текст целиком".to_string(),
                body: body_preview,
                full_text: text.to_string(),
                pos: 0,
                end: text.len(),
            }],
            String::new(),
        );
    }

    // Уникальные по полному num_str (включая букву суффикса: «28» ≠ «28б»)
    // Берём первое вхождение каждой главы — повторяющиеся заголовки
    // (опечатки автора, дубли в исходнике) игнорируем.
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
        // v0.4.0: num_str может содержать букву ("28б"). Извлекаем числовую
        // часть для поля `num`, а букву сохраняем в title через num_str.
        let num: u32 = num_str
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse()
            .unwrap_or(1);
        let next_pos = if i + 1 < sorted.len() {
            sorted[i + 1].0
        } else {
            text.len()
        };

        let match_end = find_match_end(text, *pos, num_str);
        let after_max = match_end + 500;
        let after_end = if after_max > text.len() {
            text.len()
        } else {
            let mut e = after_max;
            while e > match_end && !text.is_char_boundary(e) {
                e -= 1;
            }
            e
        };
        let after = &text[match_end..after_end];
        let title = extract_title_from_after(after);

        let body_text = &text[match_end..next_pos];
        let body_clean: String = body_text.split_whitespace().collect::<Vec<_>>().join(" ");
        let body_preview = if body_clean.len() > 400 {
            format!("{}\u{2026}", safe_slice(&body_clean, 400))
        } else {
            body_clean.clone()
        };

        // v0.4.0: если num_str содержит букву (напр. «28б»), добавляем
        // её в заголовок, чтобы пользователь видел «Глава 28б» а не «Глава 28».
        // P2.1: та же буква-суффикс сохраняется в поле `suffix` для
        // корректной сортировки TemporalAnchor.
        let (display_title, suffix_field): (String, Option<String>) = if num_str.chars().any(|c| c.is_alphabetic()) {
            // Извлекаем букву-суффикс
            let suffix: String = num_str.chars().filter(|c| c.is_alphabetic()).collect();
            // Если title уже начинается с цифры+буквы — не дублируем
            let dt = if title.starts_with(&format!("{}{}", num, suffix)) {
                title.clone()
            } else {
                format!("{}{} {}", num, suffix, title)
            };
            (dt, Some(suffix))
        } else {
            (title.clone(), None)
        };

        chapters.push(ParsedChapter {
            num,
            suffix: suffix_field,
            title: display_title,
            body: body_preview,
            full_text: body_text.trim().to_string(),
            pos: *pos,
            end: next_pos,
        });
    }

    (chapters, prologue_text)
}

// v0.4.0: skip_table_of_contents удалён — line-anchored regex сам отсеивает
// cover page. Если «Глава 1» не в начале строки (как на обложке, где все
// заголовки склеены в одну строку), она не матчится. Fallback на старый
// regex без ^ сохраняет обратную совместимость для файлов с необычным
// форматом (где заголовки реально идут в середине строки).
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
