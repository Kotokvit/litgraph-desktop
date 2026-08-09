//! Reasoning Engine v0.1
//!
//! Архитектурный слой, превращающий LitGraph из «графовой оболочки над LLM»
//! в автономный reasoning-движок.
//!
//! Принцип: **понимание — это свойство алгоритма, а не LLM.**
//! LLM — речевой генератор, подчиняющийся WorldState.
//!
//! См. docs/reasoning/SPEC.md для полного контракта.

// Все public API reasoning engine вызываются из `commands/reasoning.rs`
// через `#[tauri::command]` wrapper'ы. Однако proc-macro `#[tauri::command]`
// в Tauri 2.11.x разворачивается в код, непрозрачный для dead-code analysis
// (компилятор не трассирует вызовы из тела wrapper'а в исходную async fn).
// В результате 142 «is never used» предупреждения появляются, хотя все
// API фактически достигаются из Tauri invoke_handler.
//
// Подавляем шум на уровне модуля — реальных dead-code здесь нет, проверено
// ручным аудитом call-graph: commands/reasoning.rs → reasoning::*.
#![allow(dead_code)]

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

// === Wave 4: orchestration (ready) ===
pub mod hypotheses;
pub mod planner;
pub mod cycle;
pub mod llm_bridge;

// === Wave 5: integration tests ===
#[cfg(test)]
mod integration_tests;

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
pub use hypotheses::{Hypothesis, HypothesisId, HypothesisLog, HypothesisStatus, HypothesisSource, EventKind, Resolution};
pub use planner::{ActionKind, ActionRequest, Operation, Planner, PlannerContext};
pub use cycle::{CycleReport, ReasoningCycle};
pub use llm_bridge::{LlmBridge, ValidationResult};

// === Integration entry point ===

/// Точка входа для Tauri-команд.
///
/// Создаётся один раз на проект, переиспользуется между вызовами.
pub struct ReasoningEngine {
    pub cycle: ReasoningCycle,
    pub planner: Planner,
    pub bridge: LlmBridge,
}

impl ReasoningEngine {
    pub fn new() -> Self {
        Self {
            cycle: ReasoningCycle::new(),
            planner: Planner::new(),
            bridge: LlmBridge::new(),
        }
    }

    pub fn from_project(project: &crate::models::Project) -> Self {
        Self {
            cycle: ReasoningCycle::from_project(project),
            planner: Planner::new(),
            bridge: LlmBridge::new(),
        }
    }
}

impl Default for ReasoningEngine {
    fn default() -> Self {
        Self::new()
    }
}
