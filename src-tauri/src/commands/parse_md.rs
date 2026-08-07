//! Автопарсер .md → граф.
//! Полностью реализован в src/parser/.

use crate::models::{ParseParams, ParseResult};
use crate::parser;

#[tauri::command]
pub async fn parse_md(params: ParseParams) -> Result<ParseResult, String> {
    parser::build_graph(&params.markdown, &params.project_title, &params.author)
        .map_err(|e| e.to_string())
}
