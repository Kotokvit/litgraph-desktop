//! Детекция локаций: capitalized слова после предлогов места.
//! Переписать с detectLocations() из parse-md/route.ts.

pub struct ParsedLocation {
    pub name: String,
    pub aliases: Vec<String>,
    pub count: usize,
    pub description: String,
}

pub fn detect(text: &str) -> Vec<ParsedLocation> {
    // TODO: реализовать — этап 4 в PROMPT_PLAN.md
    // Регэксп: (?<![...])(?:у|в|на|біля|під|над|за|до|із|від|через|крізь|около|под|возле|перед|in|at|on|near|under|over|behind|from|through)\s+([Capitalized]{3,})(?![...])
    // Группировка по префиксу, минимум 3 упоминания, топ-15
    Vec::new()
}
