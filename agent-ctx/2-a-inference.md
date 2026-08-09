# Task 2-a — inference.rs (Wave 2 / Logic Layer)

## Agent
full-stack-developer (inference.rs)

## File written
`/home/z/my-project/litgraph-desktop/src-tauri/src/reasoning/inference.rs` (≈1195 LOC including tests)

## Public types/APIs exported

| Type | Kind | Notes |
|------|------|-------|
| `InferredFact` | struct | Fields: `fact_id: FactId`, `from_event: EventId`, `rule_name: &'static str`. Derives `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`. |
| `InferenceEngine` | struct + impl + Default | Holds `rule_set: RuleSet` (private). Methods: `new(rule_set)`, `default_literary()`, `rule_set() -> &RuleSet`, `apply_event(&Event, &mut WorldState, &mut FactLog) -> Vec<InferredFact>`, `apply_events<I: IntoIterator<Item = Event>>(...) -> Vec<InferredFact>`. `Default::default()` delegates to `default_literary()`. |

## Payload substitution table (all covered)

| Action variant | Placeholder pattern | Substituted from |
|---|---|---|
| `Move{destination}` | `Str("")` on attr "location" (SetAttribute) | `FactValue::Str(action.destination)` |
| `Arrive{destination}` | `Str("")` on attr "location" (SetAttribute) | `FactValue::Str(action.destination)` |
| `Leave{source}` | n/a (InvalidateAttribute) | n/a |
| `Know{fact}` | `Str("")` in AppendToList for "knowledge" | `FactValue::Str(action.fact)` |
| `Forget{fact}` | `Str("")` in AppendToList for "knowledge" | SPECIAL: removes `Str(fact)` from list (see `apply_forget_removal`) |
| `Want{goal}` | `Str("")` in AppendToList for "goals" | `FactValue::Str(action.goal)` |
| `Plan{goal}` | `Str("")` in AppendToList for "plans" | `FactValue::Str(action.goal)` |
| `FallInLove{partner}` | `EntityRef("")` in AppendToList for "relationships" | `FactValue::EntityRef(action.partner)` |
| `Hate{target}` | `EntityRef("")` in AppendToList for "relationships" | `FactValue::EntityRef(action.target)` |
| `Betray{victim}` | `EntityRef("")` in AppendToList for "betrayals" | `FactValue::EntityRef(action.victim)` |
| `Marry{partner}` (Actor effect) | `EntityRef("")` on attr "spouse" | `FactValue::EntityRef(action.partner)` |
| `Marry{partner}` (Target effect) | `EntityRef("")` on attr "spouse" | `FactValue::EntityRef(event.actor)` (symmetry) |
| `Tell{topic,to}` | n/a (handled by RecordKnowledge) | n/a |
| `Custom{verb_lemma,..}` | n/a (catch-all uses RecordKnowledge only) | n/a |

## Private helpers

- `resolve_entity(rule_entity, event) -> Option<String>` — resolves RuleEntity::Actor/Target/Specific to actual EntityId; returns None if Target required but event.target is None.
- `is_precondition_satisfied(precondition, event, world) -> bool` — resolves Actor/Target through event context, then delegates to same `world.get` + `==` comparison as `Precondition::is_satisfied` (which can't resolve Actor/Target alone).
- `substitute_payload(effect, event) -> RuleEffect` — returns a NEW RuleEffect with placeholder values substituted from event payload. For Forget, leaves the placeholder unchanged (special handling in `apply_event`).
- `substitute_value(value, entity, attribute, event) -> FactValue` — dispatches to `substitute_str_value` or `substitute_entity_ref_value`.
- `substitute_str_value(attribute, event, original) -> FactValue` — matches `(Action variant, attribute)` for Str placeholders.
- `substitute_entity_ref_value(entity, attribute, event, original) -> FactValue` — matches `(Action variant, entity, attribute)` for EntityRef placeholders, with special Marry symmetry.
- `apply_effect(effect, event, world, facts, rule_name, inferred)` — dispatches on RuleEffect variant to specific appliers.
- `apply_set_attribute(entity_id, attribute, value, event, world, facts, rule_name, inferred)` — builds StateTransition, calls `world.set`, asserts Fact, pushes InferredFact.
- `apply_invalidate_attribute(entity_id, attribute, event, world, facts, rule_name, inferred)` — same pattern, new_value = FactValue::Unknown.
- `apply_append_to_list(entity_id, attribute, value, event, world, facts, rule_name, inferred)` — reads current value, pushes if List, creates [value] if Unknown/None, logs and skips if other type.
- `apply_forget_removal(entity_id, fact, event, world, facts, rule_name, inferred)` — filters `Str(fact)` from knowledge list; no-op if list missing/not-list/item-absent.

## Tests (8 required)

All 8 tests pass under `cargo test --lib`:
- test_kill_action_marks_target_dead ✓
- test_die_action_marks_actor_dead ✓
- test_move_action_updates_location ✓
- test_know_action_appends_to_knowledge_list ✓ (also verifies list-append semantics on second Know)
- test_forget_action_removes_from_knowledge_list ✓ (also covers no-op when fact not in list)
- test_precondition_blocks_rule ✓ (custom RuleSet with Actor precondition; also verifies rule fires when precondition satisfied)
- test_record_knowledge_creates_audit_trail ✓ (verifies format of knowledge string + history transition + provenance)
- test_marry_action_sets_spouse_both_ways ✓ (verifies symmetry: Anna.spouse=Bob, Bob.spouse=Anna)

## SPEC deviations (with justification)

1. **Precondition checking does NOT call `Precondition::is_satisfied(&world)` directly.** Brief said "using `Precondition::is_satisfied(&world)`", but Wave 1 rules.rs's `is_satisfied` returns `false` for `Actor`/`Target` entities (cannot resolve without event context — explicitly noted in Wave 1 worklog: "inference.rs Wave 2 will resolve"). Implemented `is_precondition_satisfied` free function that resolves Actor/Target through event context first, then uses the same `world.get` + `PartialEq` comparison logic as `is_satisfied`. For `Specific` entities, behavior is identical to calling `is_satisfied`.

2. **`SetAttributeFromEvent` variant not fully implemented.** Default_literary rule set does not use this variant. Stub logs via `eprintln!` and skips. `EventField` import omitted (not in brief's mandated import list). Documented in code comment.

3. **`Rule` and `TemporalAnchor` imports marked `#[allow(unused_imports)]`.** Brief mandates these imports, but they are only named explicitly in test code (which `cargo check --lib` without `--tests` does not see). Both are used in tests (Rule for constructing custom RuleSet in test_precondition_blocks_rule; TemporalAnchor for constructing anchors in all tests). The `#[allow(unused_imports)]` attribute suppresses the warning in non-test compilation contexts while keeping the imports available for tests.

4. **Forget no-op when fact not in list.** Brief said "remove `FactValue::Str(fact.clone())` from the list (if present)". My implementation returns early without recording any transition/fact if the list doesn't contain the fact, doesn't exist, or isn't a list. This is semantically correct (no change → no transition needed) and is covered by `test_forget_action_removes_from_knowledge_list` (second Forget asserts no InferredFact produced).

## Verification

Standalone check at `/tmp/check_inference/` with stub lib.rs that mirrors `reasoning/mod.rs` re-exports plus `pub mod inference;`:
- `cargo check --lib --tests` → clean (no errors, no warnings on inference.rs)
- `cargo test --lib` → 41 tests pass (8 inference + 33 Wave 1 sibling tests)
- `cargo clippy --lib --tests` → no warnings on inference.rs (Wave 1 files have pre-existing warnings, not in scope)

## Dependencies on other modules (all satisfied by Wave 1)

- `crate::reasoning::facts::{Action, Event, EventId, Fact, FactId, FactLog, FactValue, Provenance}` ✓ (1-a facts.rs)
- `crate::reasoning::state::{Attribute, StateTransition, WorldState}` ✓ (1-b state.rs)
- `crate::reasoning::rules::{Rule, RuleSet, RuleEffect, RuleEntity, Precondition}` ✓ (1-d rules.rs)
- `crate::reasoning::timeline::TemporalAnchor` ✓ (1-c timeline.rs)

## Worklog
Appended to `/home/z/my-project/litgraph-desktop/worklog.md` under section `Task ID: 2-a`.

## Status
✅ Complete. All 8 unit tests pass. Ready for integration into `mod.rs` (Wave 5) — coordinator should uncomment `pub mod inference;` and add `pub use inference::{InferenceEngine, InferredFact};` to re-exports.
