use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    Flow,
    Cause,
    Character,
    Location,
    Reference,
    Conflict,
    Foreshadow,
    Alternative,
    Theme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeData {
    pub kind: Option<EdgeKind>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LitEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub source_handle: Option<String>,
    pub target_handle: Option<String>,
    #[serde(rename = "type")]
    pub edge_type: Option<String>,
    pub animated: Option<bool>,
    pub data: Option<EdgeData>,
}
