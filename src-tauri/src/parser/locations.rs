//! Детекция локаций: capitalized слова после предлогов места.
//! Переписано с detectLocations() из parse-md/route.ts.

use fancy_regex::Regex;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ParsedLocation {
    pub name: String,
    pub aliases: Vec<String>,
    pub count: usize,
    pub description: String,
}

/// Те же стоп-слово что и в characters.rs (упрощённо — импортируем из characters)
use super::characters::STOP_WORDS;

pub fn detect(text: &str) -> Vec<ParsedLocation> {
    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();

    // Регэксп: предлог места + Capitalized слово
    // v0.3.0: сужен список предлогов. Убраны: до, із, від, через, крізь,
    // from, through — они часто идут с одушевлёнными («к Алексея»,
    // «от Марты», «из дома») и создавали ложноположительные локации
    // из имён персонажей в косвенных падежах.
    // Оставлены только предлоги, требующие предложного/местного падежа
    // и действительно указывающие на lokацию.
    let re = Regex::new(
        r"(?<![a-zA-Z\x{0400}-\x{04FF}])(?:у|в|на|біля|під|над|за|около|под|возле|перед|in|at|on|near|under|over|behind)\s+([А-ЯЁA-Z][а-яёa-z\x{0400}-\x{04FF}]{2,})(?![a-zA-Z\x{0400}-\x{04FF}])",
    ).expect("invalid regex");

    let mut loc_counts: HashMap<String, usize> = HashMap::new();

    for caps_result in re.captures_iter(text) {
        let caps = match caps_result {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some(m) = caps.get(1) {
            let word = m.as_str();
            if stop.contains(word) {
                continue;
            }
            *loc_counts.entry(word.to_string()).or_insert(0) += 1;
        }
    }

    // v0.4.0: Группировка по лемме (вместо 4-символьного префикса).
    // Раньше «Яме» и «Яму» сливались по 4-char prefix «яме»/«яму» → НЕТ,
    // они не сливались (префиксы разные). Теперь «Яме» → lemmatize_simple →
    // «ям» (отрезали «е»), «Яму» → «ям» (отрезали «у») — СЛИВАЮТСЯ.
    // А «Земле» и «Империи» имеют разные леммы → НЕ сливаются.
    let mut groups: HashMap<String, (String, usize, HashSet<String>)> = HashMap::new();
    for (word, count) in &loc_counts {
        if *count < 3 {
            continue;
        }
        let key = super::lemmatize_simple(word);
        let entry = groups.entry(key).or_insert_with(|| (word.clone(), 0, HashSet::new()));
        entry.1 += count;
        entry.2.insert(word.clone());
        // v0.4.0: выбираем каноничное имя — nominative (обычно короче).
        // Раньше выбирали shortest, что работало, но теперь при lemmatize
        // лучше выбирать форму, которая ближе к lemma (т.е. nominative).
        // Эвристика: shortest form — обычно это nominative (Яма vs Яму vs Яме).
        if word.len() < entry.0.len() {
            entry.0 = word.clone();
        }
    }

    let mut sorted: Vec<_> = groups.into_values().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(15);

    sorted
        .into_iter()
        .map(|(rep, count, forms)| ParsedLocation {
            name: rep,
            aliases: forms.into_iter().collect(),
            count,
            description: format!(
                "Локация, упомянутая {} раз с предлогами места.",
                count
            ),
        })
        .collect()
}

/// Подсчёт упоминаний локации в тексте
/// БЕЗ regex — простые строковые поиски
pub fn count_in_text(aliases: &[String], text: &str) -> usize {
    let lower = text.to_lowercase();
    let mut total = 0;
    for alias in aliases {
        let alias_lower = alias.to_lowercase();
        let mut start = 0;
        while let Some(pos) = lower[start..].find(&alias_lower) {
            total += 1;
            start = start + pos + alias_lower.len();
        }
    }
    total
}
