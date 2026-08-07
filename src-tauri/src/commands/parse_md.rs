//! Автопарсер .md → граф.
//!
//! Переписать с `src/app/api/parse-md/route.ts` (705 строк TS).
//! Алгоритмы сохранить 1:1 — см. docs/PROMPT_PLAN.md раздел 3.1.

use crate::models::{GraphData, ParseParams, ParseResult};
use crate::parser;

#[tauri::command]
pub async fn parse_md(params: ParseParams) -> Result<ParseResult, String> {
    // TODO: реализовать — этап 4 в PROMPT_PLAN.md
    // Пока заглушка: вызываем parser::build_graph
    parser::build_graph(&params.markdown, &params.project_title, &params.author)
        .map_err(|e| e.to_string())
}
