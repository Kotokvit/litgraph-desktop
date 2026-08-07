use serde::{Deserialize, Serialize};

use super::{LitEdge, LitNode};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub title: String,
    pub author: String,
    pub description: String,
    pub nodes: Vec<LitNode>,
    pub edges: Vec<LitEdge>,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    pub size_bytes: u64,
    pub node_count: usize,
    pub edge_count: usize,
}

/// Данные графа (для парсера)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphData {
    pub nodes: Vec<LitNode>,
    pub edges: Vec<LitEdge>,
}

/// Параметры для парсера .md
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseParams {
    pub markdown: String,
    pub project_title: String,
    pub author: String,
}

/// Результат парсинга
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseResult {
    pub title: String,
    pub author: String,
    pub description: String,
    pub nodes: Vec<LitNode>,
    pub edges: Vec<LitEdge>,
    pub created_at: u64,
    pub updated_at: u64,
    pub stats: ParseStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseStats {
    pub chapters: usize,
    pub characters: usize,
    pub locations: usize,
    pub themes: usize,
    pub edges: usize,
    pub words: usize,
}
