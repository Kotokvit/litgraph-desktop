//! Детекция персонажей: capitalized слова с частотой 5+.
//! Переписать с detectCharacters() из parse-md/route.ts.

pub struct ParsedCharacter {
    pub name: String,
    pub aliases: Vec<String>,
    pub count: usize,
    pub description: String,
}

/// Стоп-слово (местоимения, союзы, предлоги на 3 языках).
/// Перенести список из STOP_WORDS в parse-md/route.ts.
const STOP_WORDS: &[&str] = &[
    // TODO: перенести полный список ~200 слов из TS
    "Цей", "Ця", "Це", "Той", "Та", "Те", "Він", "Вона", "Воно", "Вони",
    "The", "This", "That", "He", "She", "It", "They",
    // ... и т.д.
];

pub fn detect(text: &str) -> Vec<ParsedCharacter> {
    // TODO: реализовать — этап 4 в PROMPT_PLAN.md
    // 1. Регэксп: (?<![a-zA-Z\u0400-\u04FF])([А-ЯЁA-Z][а-яёa-z\u0400-\u04FF]{2,})(?![a-zA-Z\u0400-\u04FF])
    // 2. Фильтр стоп-слов
    // 3. Группировка по 4-символьному префиксу
    // 4. Минимум 5 упоминаний, топ-25
    Vec::new()
}
