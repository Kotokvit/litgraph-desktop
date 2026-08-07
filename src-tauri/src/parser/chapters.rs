//! Детекция глав по 9 паттернам.
//! Переписать с detectChapters() из parse-md/route.ts.

pub struct ParsedChapter {
    pub num: u32,
    pub title: String,
    pub body: String,
    pub full_text: String,
}

/// 9 паттернов для разных форматов заголовков:
/// "Глава N", "Розділ N", "Частина N", "Chapter N", "Part N", "Часть N",
/// "# N", "## N", "### N"
pub fn detect(text: &str) -> (Vec<ParsedChapter>, String /* prologue */) {
    // TODO: реализовать — этап 4 в PROMPT_PLAN.md
    (Vec::new(), String::new())
}
