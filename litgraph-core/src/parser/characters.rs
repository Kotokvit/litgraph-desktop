//! Детекция персонажей: capitalized слова с частотой 5+.
//! Переписано с detectCharacters() из parse-md/route.ts.

use fancy_regex::Regex;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub struct ParsedCharacter {
    pub name: String,
    pub aliases: Vec<String>,
    pub count: usize,
    pub description: String,
}

/// Стоп-слово (местоимения, союзы, предлоги на 3 языках).
/// Полный список перенесён из parse-md/route.ts.
pub const STOP_WORDS: &[&str] = &[
    // УКР
    "Цей", "Ця", "Це", "Той", "Та", "Те", "Він", "Вона", "Воно", "Вони",
    "Його", "Її", "Їх", "Мій", "Твій", "Наш", "Ваш", "Свій", "Своя", "Своє",
    "Бо", "Що", "Як", "Де", "Куди", "Звідки", "Коли", "Чому", "Чи", "Тож",
    "Тут", "Там", "Так", "Ні", "Якщо", "Але", "Однак", "Отже", "Проте", "Також",
    "Був", "Була", "Було", "Були", "Є", "Бути", "Єсть",
    "Крім", "Замість", "Після", "Перед", "Між", "Біля", "Над", "Під", "За", "На",
    "Одного", "Першого", "Другого", "Третього", "Кожен", "Кожна", "Кожне", "Усі", "Всі",
    "Сьогодні", "Вчора", "Завтра", "Тепер", "Тоді", "Потім", "Раптом", "Незабаром",
    "Швидко", "Повільно", "Знову", "Ще", "Вже", "Тільки", "Навіть", "Можливо",
    "Дякую", "Вибачте", "Пробачте", "Будь", "Ласка", "Скажи", "Подивися", "Послухай",
    "Боже", "Господи", "Діду", "Бабусю", "Мамо", "Тату", "Сину", "Донько",
    "Так", "Ні", "Авжеж", "Звичайно", "Добре", "Погано",
    "Світло", "Темрява", "Тиша", "Вогонь", "Вода", "Повітря", "Земля", "Небо",
    // РУС
    "Этот", "Эта", "Эти", "Тот", "Та", "Те", "Он", "Она", "Оно", "Они",
    "Его", "Её", "Их", "Мой", "Твой", "Наш", "Ваш", "Свой", "Своя", "Своё",
    "Потому", "Что", "Как", "Где", "Куда", "Откуда", "Когда", "Почему", "Ли", "Итак",
    "Здесь", "Там", "Так", "Нет", "Если", "Но", "Однако", "Следовательно", "Против", "Также",
    "Был", "Была", "Было", "Были", "Есть", "Быть",
    "Кроме", "Вместо", "После", "Перед", "Между", "Около", "Над", "Под", "За", "На",
    "Каждый", "Каждая", "Все", "Всё",
    "Сегодня", "Вчера", "Завтра", "Теперь", "Тогда", "Потом", "Внезапно", "Скоро",
    "Быстро", "Медленно", "Снова", "Ещё", "Уже", "Только", "Даже", "Возможно",
    "Спасибо", "Извините", "Прости", "Пожалуйста", "Скажи", "Посмотри", "Послушай",
    "Боже", "Господи", "Дед", "Бабушка", "Мама", "Папа", "Сын", "Дочь",
    "Да", "Нет", "Конечно", "Хорошо", "Плохо",
    "Свет", "Тьма", "Тишина", "Огонь", "Вода", "Воздух", "Земля", "Небо",
    // EN
    "The", "This", "That", "These", "Those", "He", "She", "It", "They", "We", "You",
    "His", "Her", "Its", "Their", "My", "Your", "Our",
    "But", "And", "Or", "Not", "Yes", "No", "Oh", "Ah",
    "When", "Where", "What", "Who", "Why", "How", "Which",
    "Here", "There", "Now", "Then", "Today", "Yesterday", "Tomorrow",
    "Because", "If", "Although", "However", "So", "Therefore", "Also", "Too",
    "Was", "Were", "Been", "Have", "Has", "Had", "Being", "Having",
    "Some", "Any", "All", "Every", "Each", "Both", "Either", "Neither",
    "One", "Two", "Three", "First", "Second", "Third",
    "Good", "Bad", "Please", "Thanks", "Thank",
    "Mr", "Mrs", "Dr", "Ms",
];

pub fn detect(text: &str) -> Vec<ParsedCharacter> {
    let stop: HashSet<&str> = STOP_WORDS.iter().copied().collect();

    // Регэксп для capitalized слов: кириллица + латиница
    // (?<![a-zA-Z\u0400-\u04FF])([А-ЯЁA-Z][а-яёa-z\u0400-\u04FF]{2,})(?![a-zA-Z\u0400-\u04FF])
    let re = Regex::new(
        r"(?<![a-zA-Z\x{0400}-\x{04FF}])([А-ЯЁA-Z][а-яёa-z\x{0400}-\x{04FF}]{2,})(?![a-zA-Z\x{0400}-\x{04FF}])",
    ).expect("invalid regex");

    let mut word_counts: HashMap<String, usize> = HashMap::new();

    for caps_result in re.captures_iter(text) {
        let caps = match caps_result {
            Ok(c) => c,
            Err(_) => continue,
        };
        if let Some(m) = caps.get(1) {
            let word = m.as_str();
            // Проверим, что это не первое слово в предложении
            let start = m.start();
            if start == 0 {
                continue;
            }
            // Возьмём до 3 байт перед словом, но подгоним под char boundary
            let mut preceding_start = if start >= 3 { start - 3 } else { 0 };
            while preceding_start < start && !text.is_char_boundary(preceding_start) {
                preceding_start += 1;
            }
            let preceding = &text[preceding_start..start];
            // Если перед словом . ! ? … " ' » то это первое слово предложения — пропускаем
            let re_sent_end = Regex::new(r#"[.!?…]["'»]?\s*$"#).unwrap();
            if re_sent_end.is_match(preceding).unwrap_or(false) {
                continue;
            }
            if stop.contains(word) {
                continue;
            }
            *word_counts.entry(word.to_string()).or_insert(0) += 1;
        }
    }

    // Группировка по 4-символьному префиксу
    let mut groups: HashMap<String, (String, usize, HashSet<String>)> = HashMap::new();
    for (word, count) in &word_counts {
        if *count < 5 {
            continue;
        }
        let key = word.chars().take(4).collect::<String>().to_lowercase();
        let entry = groups.entry(key).or_insert_with(|| (word.clone(), 0, HashSet::new()));
        entry.1 += count;
        entry.2.insert(word.clone());
        // Самая короткая форма — каноничное имя
        if word.len() < entry.0.len() {
            entry.0 = word.clone();
        }
    }

    let mut sorted: Vec<_> = groups.into_values().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    sorted.truncate(25);

    sorted
        .into_iter()
        .map(|(rep, count, forms)| {
            let forms_vec: Vec<String> = forms.into_iter().collect();
            let aliases_str = forms_vec.iter().take(6).cloned().collect::<Vec<_>>().join(", ");
            ParsedCharacter {
                name: rep,
                aliases: forms_vec,
                count,
                description: format!(
                    "Персонаж, упомянутый {} раз в тексте. Формы: {}.",
                    count, aliases_str
                ),
            }
        })
        .collect()
}

/// Проверка присутствия персонажа в тексте главы (минимум n упоминаний)
pub fn count_in_text(aliases: &[String], text: &str) -> usize {
    let mut total = 0;
    for alias in aliases {
        let pattern = format!(
            r"(?<![a-zA-Z\x{{0400}}-\x{{04FF}}]){}(?![a-zA-Z\x{{0400}}-\x{{04FF}}])",
            fancy_regex::escape(alias)
        );
        if let Ok(re) = Regex::new(&pattern) {
            total += re.find_iter(text).filter_map(|r| r.ok()).count();
        }
    }
    total
}
