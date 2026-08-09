//! Reasoning Engine v0.1
//!
//! Архитектурный слой, превращающий LitGraph из «графовой оболочки над LLM»
//! в автономный reasoning-движок.
//!
//! Принцип: **понимание — это свойство алгоритма, а не LLM.**
//! LLM — речевой генератор, подчиняющийся WorldState.
//!
//! См. docs/reasoning/SPEC.md для полного контракта.

// === Wave 1: data layer (ready) ===
pub mod facts;
pub mod state;
pub mod rules;
pub mod timeline;

// === Wave 2: logic layer (ready) ===
pub mod inference;
pub mod causality;
pub mod constraints;
pub mod contradictions;

// === Wave 3: semantic layer (ready) ===
pub mod semantic_parser;
pub mod memory;

// === Wave 4: orchestration (pending) ===
// pub mod hypotheses;
// pub mod planner;
// pub mod cycle;
// pub mod llm_bridge;

// Реэкспорт ключевых типов (только типы, не функции).
pub use facts::{Action, Event, EventId, Fact, FactId, FactLog, FactValue, Provenance, VerbPolarity};
pub use state::{StateTransition, WorldSnapshot, WorldState};
pub use timeline::{TemporalAnchor, Timeline, TimeInterval};
pub use rules::{Precondition, Rule, RuleEffect, RuleEntity, RuleSet};
pub use inference::{InferenceEngine, InferredFact};
pub use causality::{CausalLink, CausalityEngine};
pub use constraints::{Constraint, ConstraintCondition, ConstraintEngine, ConstraintViolation};
pub use contradictions::{CausalLoop, ContradictionDetector, ContradictionReport, TemporalParadox};
pub use semantic_parser::{EntityResolver, SvoTriplet};
pub use memory::{KnowledgeBase, Subgraph};

// === Integration entry points (Wave 5) ===

/// Точка входа для Tauri-команд.
///
/// Создаётся один раз на проект, переиспользуется между вызовами.
/// Полная реализация появится в Wave 5 после того, как cycle.rs / llm_bridge.rs
/// будут готовы. Пока — пустая заглушка, чтобы lib.rs компилировался.
pub struct ReasoningEngine;

impl ReasoningEngine {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReasoningEngine {
    fn default() -> Self {
        Self::new()
    }
}
