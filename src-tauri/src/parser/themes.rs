//! Детекция тем/мотивов: ~80 тематических существительных на 3 языках.
//! Переписано с detectThemes() из parse-md/route.ts.

use fancy_regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ParsedTheme {
    pub name: String,
    pub count: usize,
    pub description: String,
}

/// Словарь тематических существительных (keyword → theme_name)
pub const THEME_KEYWORDS: &[(&str, &str)] = &[
    // УКР
    ("тиша", "Тишина"),
    ("мовчання", "Молчание"),
    ("пам'ять", "Память"),
    ("память", "Память"),
    ("світло", "Свет"),
    ("світ", "Мир"),
    ("темрява", "Тьма"),
    ("тінь", "Тень"),
    ("тіні", "Тень"),
    ("страх", "Страх"),
    ("надія", "Надежда"),
    ("любов", "Любовь"),
    ("зрада", "Предательство"),
    ("прощення", "Прощение"),
    ("самотність", "Одиночество"),
    ("доля", "Судьба"),
    ("свобода", "Свобода"),
    ("вибір", "Выбор"),
    ("правда", "Правда"),
    ("брехня", "Ложь"),
    ("брехні", "Ложь"),
    ("війна", "Война"),
    ("мир", "Мир"),
    ("смерть", "Смерть"),
    ("життя", "Жизнь"),
    ("народження", "Рождение"),
    ("кров", "Кровь"),
    ("вогонь", "Огонь"),
    ("вода", "Вода"),
    ("повітря", "Воздух"),
    ("земля", "Земля"),
    ("небо", "Небо"),
    ("провина", "Вина"),
    ("кара", "Кара"),
    ("покута", "Покаяние"),
    ("спокута", "Искупление"),
    ("голод", "Голод"),
    ("біль", "Боль"),
    ("журба", "Печаль"),
    ("радість", "Радость"),
    ("злість", "Злость"),
    ("гнів", "Гнев"),
    ("ніжність", "Нежность"),
    ("дитинство", "Детство"),
    ("дорослість", "Взросление"),
    ("час", "Время"),
    ("мить", "Мгновение"),
    ("вічність", "Вечность"),
    ("пітьма", "Мрак"),
    ("провалля", "Бездна"),
    ("безодня", "Бездна"),
    ("голос", "Голос"),
    ("шепіт", "Шёпот"),
    ("слово", "Слово"),
    ("мова", "Язык/речь"),
    ("молчание", "Молчание"),
    // РУС
    ("тишина", "Тишина"),
    ("молчание", "Молчание"),
    ("свет", "Свет"),
    ("тьма", "Тьма"),
    ("тень", "Тень"),
    ("страх", "Страх"),
    ("надежда", "Надежда"),
    ("любовь", "Любовь"),
    ("предательство", "Предательство"),
    ("прощение", "Прощение"),
    ("одиночество", "Одиночество"),
    ("судьба", "Судьба"),
    ("свобода", "Свобода"),
    ("выбор", "Выбор"),
    ("правда", "Правда"),
    ("ложь", "Ложь"),
    ("война", "Война"),
    ("смерть", "Смерть"),
    ("жизнь", "Жизнь"),
    ("рождение", "Рождение"),
    ("кровь", "Кровь"),
    ("огонь", "Огонь"),
    ("воздух", "Воздух"),
    ("вечность", "Вечность"),
    ("мгновение", "Мгновение"),
    ("время", "Время"),
    ("детство", "Детство"),
    ("взросление", "Взросление"),
    ("боль", "Боль"),
    ("печаль", "Печаль"),
    ("радость", "Радость"),
    ("гнев", "Гнев"),
    ("нежность", "Нежность"),
    ("вина", "Вина"),
    ("искупление", "Искупление"),
    ("покаяние", "Покаяние"),
    ("шёпот", "Шёпот"),
    ("шепот", "Шёпот"),
    ("голос", "Голос"),
    ("слово", "Слово"),
    ("бездна", "Бездна"),
    ("мрак", "Мрак"),
    ("язык", "Язык/речь"),
    // EN
    ("silence", "Silence"),
    ("memory", "Memory"),
    ("light", "Light"),
    ("darkness", "Darkness"),
    ("shadow", "Shadow"),
    ("fear", "Fear"),
    ("hope", "Hope"),
    ("love", "Love"),
    ("betrayal", "Betrayal"),
    ("forgiveness", "Forgiveness"),
    ("loneliness", "Loneliness"),
    ("fate", "Fate"),
    ("freedom", "Freedom"),
    ("choice", "Choice"),
    ("truth", "Truth"),
    ("lie", "Lie"),
    ("war", "War"),
    ("death", "Death"),
    ("life", "Life"),
    ("birth", "Birth"),
    ("blood", "Blood"),
    ("fire", "Fire"),
    ("water", "Water"),
    ("air", "Air"),
    ("eternity", "Eternity"),
    ("time", "Time"),
    ("childhood", "Childhood"),
    ("pain", "Pain"),
    ("sorrow", "Sorrow"),
    ("joy", "Joy"),
    ("anger", "Anger"),
    ("voice", "Voice"),
    ("whisper", "Whisper"),
    ("word", "Word"),
    ("abyss", "Abyss"),
];

pub fn detect(text: &str) -> Vec<ParsedTheme> {
    let lower_text = text.to_lowercase();
    let mut counts: HashMap<String, usize> = HashMap::new();

    for (keyword, theme_name) in THEME_KEYWORDS {
        let pattern = format!(
            r"(?<![a-zа-яё\x{{0400}}-\x{{04FF}}]){}(?![a-zа-яё\x{{0400}}-\x{{04FF}}])",
            fancy_regex::escape(keyword)
        );
        if let Ok(re) = Regex::new(&pattern) {
            let count = re.find_iter(&lower_text).count();
            if count >= 5 {
                *counts.entry(theme_name.to_string()).or_insert(0) += count;
            }
        }
    }

    let mut sorted: Vec<_> = counts.into_iter().filter(|(_, c)| *c >= 5).collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(10);

    sorted
        .into_iter()
        .map(|(name, count)| {
            let lower_name = name.to_lowercase();
            ParsedTheme {
                name: name.clone(),
                count,
                description: format!(
                    "Сквозной мотив «{}» — встречается {} раз в тексте. Автор может связать его с конкретными главами через связи типа «Тема».",
                    lower_name, count
                ),
            }
        })
        .collect()
}

/// Подсчёт упоминаний темы в тексте главы
pub fn count_in_text(theme_name: &str, text: &str) -> usize {
    let lower_text = text.to_lowercase();
    let mut total = 0;
    for (keyword, theme) in THEME_KEYWORDS {
        if *theme != theme_name {
            continue;
        }
        let pattern = format!(
            r"(?<![a-zа-яё\x{{0400}}-\x{{04FF}}]){}(?![a-zа-яё\x{{0400}}-\x{{04FF}}])",
            fancy_regex::escape(keyword)
        );
        if let Ok(re) = Regex::new(&pattern) {
            total += re.find_iter(&lower_text).count();
        }
    }
    total
}
