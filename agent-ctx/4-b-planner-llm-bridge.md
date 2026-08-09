# Task 4-b: planner.rs + llm_bridge.rs

**Agent:** full-stack-developer
**Task:** Build planner.rs (action planner) and llm_bridge.rs (LLM-as-generator
bridge with state enforcement). Part of Wave 4 / Orchestration of LitGraph
Reasoning Engine.

## Files written

- `/home/z/my-project/litgraph-desktop/src-tauri/src/reasoning/planner.rs` (~590 LOC)
- `/home/z/my-project/litgraph-desktop/src-tauri/src/reasoning/llm_bridge.rs` (~1100 LOC)

## Public API

### planner.rs

- `Operation` enum (8 variants): `Observe{raw_text}`, `BuildState`, `Reason`,
  `Hypothesize`, `Verify{hypothesis_id}`, `UpdateState`, `Query{question}`,
  `Act{action_request}`, `Idle`. Derives `Debug, Clone, Serialize, Deserialize`.
- `ActionRequest` struct: `kind: ActionKind`, `constraints: Vec<String>`,
  `allowed: Vec<String>`, `forbidden: Vec<String>`, `task: String`,
  `context_subgraph: Option<Subgraph>`. Derives `Debug, Clone, Serialize, Deserialize`.
- `ActionKind` enum (5 variants): `WriteScene`, `ContinueChapter`, `AnalyzePlot`,
  `AnswerQuestion`, `GenerateHypothesis`. Derives `Debug, Clone, Serialize,
  Deserialize, PartialEq, Eq, Hash`.
- `PlannerContext` struct: `pending_events: usize`, `unverified_hypotheses:
  usize`, `last_contradiction_count: usize`, `user_query: Option<String>`.
  Derives `Debug, Clone, Default`.
- `Planner` unit struct. Derives `Debug, Clone, Default`. Methods:
  - `new() -> Self`
  - `next_operation(&PlannerContext) -> Operation`
  - `plan_for_user_query(&str) -> Operation`

### llm_bridge.rs

- `ValidationResult` enum (3 variants): `Accept{events, report}`, `Reject{
  violations, feedback_prompt}`, `Retry{reason}`. Derives `Debug, Clone`.
- `LlmBridge` unit struct. Derives `Debug, Clone, Default`. Methods:
  - `new() -> Self`
  - `build_prompt(&ActionRequest, &WorldState, &FactLog) -> (String, String)`
    — returns (system_prompt, user_prompt).
  - `validate_response(&str, &ActionRequest, &WorldState, &FactLog,
    &EntityResolver, &[ParsedChapter]) -> ValidationResult`
  - `build_feedback_prompt(&ActionRequest, &str, &[ConstraintViolation]) -> String`
- Private helper: `format_fact_value(&FactValue) -> String`.
- Const: `SYSTEM_PROMPT_TEMPLATE` (verbatim Russian system prompt per brief §3).

## Decision tree (Planner::next_operation)

1. `user_query.is_some() && pending_events == 0` → `Act{AnswerQuestion}`
2. `pending_events > 0` → `BuildState`
3. `last_contradiction_count > 0 && unverified_hypotheses == 0` → `Hypothesize`
4. `unverified_hypotheses > 0` → `Verify{hypothesis_id: 1}`
5. `user_query.is_some()` → `Act{AnswerQuestion}` (defensive fallback per spec)
6. else → `Idle`

Priority: pending events > contradictions > hypotheses > user query > idle.
Stateless: same context → same operation, deterministic.

## Validation algorithm (LlmBridge::validate_response)

1. `parse_text_fallback(generated_text, resolver, chapters)` → events
2. `events.is_empty()` → `Retry{reason: "Не удалось извлечь события..."}`
3. For each event: `ConstraintEngine::default_literary().check(world, event)`;
   collect all `ConstraintViolation`.
4. `violations.is_empty()` → `Accept{events, report: ContradictionDetector::
   detect_all(Vec::new(), facts, &events, Vec::new())}`
5. else → `Reject{violations, feedback_prompt: build_feedback_prompt(...)}`

## Sync/async boundary (CRITICAL)

- `llm_bridge.rs` is fully SYNC — no tokio, no async, no reqwest.
- Does NOT import `crate::ai::*` — bridge only builds prompts and parses
  responses. Actual async LLM call (`ai::chat`) happens at Tauri command layer
  (caller's responsibility), typically via `tokio::task::spawn_blocking` per
  SPEC §5.7.

## Tests

- planner.rs: 11 tests (5 required + 6 extra) — all pass.
- llm_bridge.rs: 10 tests (5 required + 5 extra) — all pass.
- Total: 109/109 tests pass in standalone cargo project at `/tmp/check_4b/`
  (includes Wave 1+2+3 sibling tests).

## SPEC deviations

1. `Operation` does NOT derive `PartialEq` (brief lists only Debug/Clone/
   Serialize/Deserialize). `ActionRequest` contains `Option<Subgraph>` which
   holds Vec types that don't all derive PartialEq. Tests use `matches!()`.
2. `ActionKind` derives `PartialEq, Eq, Hash` (brief lists PartialEq only) —
   Eq and Hash come for free with unit variants, useful for future HashMap.
3. `PlannerContext` does NOT derive Serialize/Deserialize (brief lists
   Debug/Clone/Default only) — transient snapshot, not persisted.
4. `Verify{hypothesis_id}` hardcodes `hypothesis_id=1`. Planner doesn't have
   access to hypothesis store (would require importing `crate::reasoning::
   hypotheses` which is not yet ready in Wave 4). PlannerContext can be
   extended with `first_pending_hypothesis_id: Option<u64>` in Wave 5.
5. `LlmBridge::build_prompt` takes `_world: &WorldState` (unused) — brief
   signature includes it for API stability and future use. State sourced
   from FactLog (Fact has `valid_from` for «since Глава N» annotation).
6. `LlmBridge::validate_response` constructs fresh
   `ConstraintEngine::default_literary()` per call. Brief signature doesn't
   include a ConstraintEngine parameter — default literary is the contract.

## Verification

- `cargo check --lib --tests` — clean (no warnings on planner.rs / llm_bridge.rs).
- `cargo test --lib` — 109/109 passing.
- `cargo clippy --lib --tests` — zero warnings/errors on planner.rs /
  llm_bridge.rs. Pre-existing errors in Wave 1 facts.rs (approx_constant)
  and state.rs (explicit_auto_deref) are NOT from my code.

## Cross-module dependencies

- planner.rs: `use crate::reasoning::memory::Subgraph;` + `use serde::{...};`
- llm_bridge.rs: `use crate::parser::chapters::ParsedChapter;` + 6 sibling
  reasoning module imports (constraints, contradictions, facts, planner,
  semantic_parser, state). NO `crate::ai::*` import (sync/async boundary).

## Ready for Wave 5 integration

Coordinator should:
1. Uncomment `pub mod planner;` and `pub mod llm_bridge;` in mod.rs.
2. Add to re-exports:
   - `pub use planner::{ActionKind, ActionRequest, Operation, Planner, PlannerContext};`
   - `pub use llm_bridge::{LlmBridge, ValidationResult};`
3. Wire into cycle.rs (Wave 4 sibling, separate task):
   ```rust
   let op = planner.next_operation(&ctx);
   match op {
       Operation::Act { action_request } => {
           let (system, user) = bridge.build_prompt(&action_request, &world, &facts);
           // ... async LLM call at Tauri command layer ...
           match bridge.validate_response(&generated, &action_request, &world, &facts, &resolver, &chapters) {
               ValidationResult::Accept { events, report } => { /* commit */ }
               ValidationResult::Reject { feedback_prompt, .. } => { /* retry */ }
               ValidationResult::Retry { reason } => { /* retry differently */ }
           }
       }
       Operation::BuildState => { /* apply inference rules */ }
       // ... etc.
   }
   ```
