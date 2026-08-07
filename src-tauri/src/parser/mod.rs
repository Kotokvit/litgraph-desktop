//! Автопарсер .md → граф.
//!
//! См. docs/PROMPT_PLAN.md раздел 3.1.
//! Переписать с src/app/api/parse-md/route.ts (705 строк TS).

pub mod chapters;
pub mod characters;
pub mod locations;
pub mod themes;

use crate::models::{GraphData, LitEdge, LitNode, ParseResult, ParseStats};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Пустой текст")]
    Empty,
    #[error("Regex error: {0}")]
    Regex(#[from] fancy_regex::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn build_graph(
    markdown: &str,
    project_title: &str,
    author: &str,
) -> Result<ParseResult, ParseError> {
    if markdown.trim().is_empty() {
        return Err(ParseError::Empty);
    }

    // TODO: реализовать полностью — этап 4 в PROMPT_PLAN.md
    // 1. chapters::detect(markdown) → Vec<ParsedChapter>
    // 2. characters::detect(markdown) → Vec<ParsedCharacter>
    // 3. locations::detect(markdown) → Vec<ParsedLocation>
    // 4. themes::detect(markdown) → Vec<ParsedTheme>
    // 5. Сборка графа + раскладка

    let word_count = markdown.split_whitespace().count();
    let graph = GraphData {
        nodes: vec![],
        edges: vec![],
    };

    Ok(ParseResult {
        title: project_title.to_string(),
        author: author.to_string(),
        description: format!(
            "Автоматически разобранный текст: 0 глав, 0 персонажей, 0 локаций, 0 тем, 0 связей. Всего {} слов.",
            word_count
        ),
        nodes: graph.nodes,
        edges: graph.edges,
        created_at: 0,
        updated_at: 0,
        stats: ParseStats {
            chapters: 0,
            characters: 0,
            locations: 0,
            themes: 0,
            edges: 0,
            words: word_count,
        },
    })
}
