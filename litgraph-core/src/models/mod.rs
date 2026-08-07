//! Типы данных — зеркало types.ts из прототипа.
//! См. docs/PROMPT_PLAN.md раздел 3.3.

pub mod node;
pub mod edge;
pub mod project;
pub mod version;

pub use node::{LitNode, LitNodeData, LitNodeType, Position};
pub use edge::{LitEdge, EdgeData, EdgeKind};
pub use project::{Project, ProjectMeta, GraphData, ParseParams, ParseResult, ParseStats};
pub use version::ChapterVersion;
