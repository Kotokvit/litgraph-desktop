use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterVersion {
    pub id: String,
    pub timestamp: u64,
    pub full_text: String,
    pub word_count: usize,
    pub label: Option<String>,
    pub source: Option<String>, // "auto" | "manual" | "ai" | "restore" | "import"
}
