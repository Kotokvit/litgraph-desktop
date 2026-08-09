use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LitNodeType {
    Scene,
    Character,
    Plotpoint,
    Conflict,
    Dialogue,
    Location,
    Idea,
    Chapter,
    Theme,
    /// v0.4.2: Абстрактное существительное или понятие, не являющееся
    /// персонажем (Бездна, Эхо, Архив-как-здание, Секвестр, Голос Мира).
    Concept,
    /// v0.4.2: Коллективный субъект — политическая структура, клан, совет,
    /// корпорация. Упоминается с глаголами коллективного действия.
    Organization,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LitNodeData {
    pub title: String,
    pub body: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub tags: Vec<String>,
    pub meta: Option<serde_json::Value>,
    pub full_text: Option<String>,
    pub versions: Option<Vec<crate::models::ChapterVersion>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LitNode {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub position: Position,
    pub data: LitNodeData,
}
