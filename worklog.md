# LitGraph Reasoning Engine — Work Log

Shared multi-agent log for development of `src-tauri/src/reasoning/`.

Protocol:
1. Before starting work, read this file fully.
2. After finishing, append a new section starting with `---`.
3. Use the template specified in the project instructions.

---
Task ID: 0
Agent: main (coordinator)
Task: Architecture reconnaissance + SPEC creation

Work Log:
- Read existing codebase: models/{node,edge,project}.rs, parser/{mod,chapters,characters}.rs,
  commands/{parse_md_full,ner,conflict}.rs, ai/{mod,prompts}.rs
- Inspected Python scripts: ner_extract.py, svo_extract.py, conflict_graph.py
- Identified existing types: LitNode (9 types), LitEdge (9 kinds), ParsedChapter (with num/pos/end),
  SVO triplets (subject/verb/object/polarity), ConflictGraph (J-matrix)
- Designed reasoning engine architecture with 14 modules organized in 5 waves
- Wrote shared SPEC.md at docs/reasoning/SPEC.md (250+ lines)
- Created src-tauri/src/reasoning/mod.rs skeleton with public re-exports

Stage Summary:
- SPEC.md defines unified type contracts (FactId, EventId, EntityId, Action enum,
  FactValue enum, TemporalAnchor, Rule, Constraint, Hypothesis, CycleReport)
- All subagents MUST follow SPEC.md — non-conforming code will be rejected
- Integration boundary: reasoning engine consumes ParseResult + SVO triplets +
  ConflictGraph, produces WorldState + FactLog + ContradictionReport
- LLM is reachable only through llm_bridge.rs; reasoning modules are sync,
  no tokio dependency
- 5-wave build plan: Wave 1 (data) → Wave 2 (logic) → Wave 3 (semantic) →
  Wave 4 (orchestration) → Wave 5 (integration)
- Ready to dispatch Wave 1: facts.rs, state.rs, timeline.rs, rules.rs (4 parallel subagents)

---
Task ID: 1-b
Agent: full-stack-developer (state.rs)
Task: Build state.rs — WorldState and StateTransition

Work Log:
- Read mandatory context: docs/reasoning/SPEC.md (§2.7 WorldState, §2.1 IDs, §2.2 TemporalAnchor,
  §2.5 FactValue), worklog.md (Task 0 architecture recon), src-tauri/src/models/node.rs
  (LitNode contract — confirmed EntityId = String == LitNode.id).
- Read existing reasoning/timeline.rs (parallel Wave-1 agent's output) to confirm
  TemporalAnchor's public API (fields, `before()` semantics) matches SPEC §2.2.
- Wrote src-tauri/src/reasoning/state.rs (≈540 LOC including tests):
  * Module doc comment (Russian) explaining purpose, conventions for special
    attributes (alive/location/knowledge/goals/relationships/emotional_state/physical_state),
    and pointer to SPEC §2.7.
  * `pub type EntityId = String;` and `pub type Attribute = String;` per SPEC §2.1.
  * `StateTransition` struct — fields match SPEC §2.7 exactly (entity, attribute,
    old_value: Option<FactValue>, new_value: FactValue, caused_by_event: Option<EventId>,
    at: TemporalAnchor); derives Debug, Clone, Serialize, Deserialize.
  * `WorldSnapshot` struct with public `current` HashMap and `now` TemporalAnchor;
    derives Debug, Clone, Serialize, Deserialize.
  * `WorldState` struct with private fields (current, history, now) per SPEC §2.7.
    All public types derive Debug, Clone, Serialize, Deserialize.
  * 11 methods: new, get, set, has_attribute, entities_with, advance_to (panics on
    backward), now, history, snapshot, restore (appends synthetic __restore__ note),
    invalidate (sets Unknown + records transition). Default impl provided.
  * Private `fact_value_eq` helper for structural FactValue comparison (SPEC does
    not derive PartialEq on FactValue; needed by entities_with).
  * 6 unit tests covering all required scenarios (set/get, history audit trail,
    invalidate→Unknown, snapshot+restore round-trip, advance_to monotonic,
    entities_with including Unknown-matching edge case).
- Verification: created /tmp/check_proj with stub `facts` and `timeline` modules
  mirroring SPEC contracts, ran `cargo check --lib` (clean) and `cargo test --lib`
  (6/6 passing). Could not run `cargo check` in src-tauri directly because
  Tauri's gdk-sys build needs system libs not present in sandbox, and because
  facts.rs/rules.rs/etc. from Wave 1 are still in flight — but my code is
  syntactically and semantically correct against the SPEC contract.
- Wrote agent-ctx work record at /home/z/my-project/litgraph-desktop/agent-ctx/1-b-state.md.

Stage Summary:
- Types implemented: EntityId (alias), Attribute (alias), StateTransition,
  WorldSnapshot, WorldState. WorldState methods cover full read/write/audit/snapshot
  lifecycle required by SPEC §2.7.
- Key decisions:
  (1) `&str` parameters for entity/attr in get/set/has_attribute/entities_with/
      invalidate — matches task-brief signatures, ergonomically compatible with
      SPEC's `&EntityId`/`&Attribute` via deref coercion.
  (2) `set` trusts caller-provided transition (does not re-derive old_value) —
      keeps set cheap; rules/inference own consistency.
  (3) `restore` appends a synthetic `__restore__` StateTransition to history
      (empty entity, descriptive Russian message) so audit trail survives rollback.
  (4) `advance_to` panics on backward anchors per brief — message is in Russian,
      includes both anchors for debugging.
  (5) `has_attribute` returns true even when value is Unknown — invalidate sets
      Unknown but does not remove the attribute (matches SPEC's "value lost,
      but fact was set" semantics for FactValue::Unknown).
  (6) Private `fact_value_eq` for structural FactValue equality (recursive over
      List, NaN-aware for Float) since SPEC §2.5 doesn't derive PartialEq.
- No `pub use` from other reasoning modules — only `use crate::reasoning::...`.
- No tokio, no async, no unwrap() on external data.
- Ready for Wave 1 sibling modules (facts.rs, rules.rs) and Wave 2 consumers
  (constraints.rs, inference.rs, cycle.rs).

---
Task ID: 1-a
Agent: full-stack-developer (facts.rs)
Task: Build facts.rs — fact/event data layer

Work Log:
- Read mandatory inputs: docs/reasoning/SPEC.md, worklog.md (Task 0 entry),
  src-tauri/src/models/node.rs (LitNode.id format confirmed: `String`),
  src-tauri/src/reasoning/mod.rs (module declarations + re-exports already
  reference facts:: types — must match), Cargo.toml (serde available).
- Inspected existing `pub use facts::{...}` in reasoning/mod.rs to confirm
  required public type set: Event, EventId, Fact, FactId, FactLog, FactValue,
  Action, VerbPolarity, Provenance. All implemented.
- Implemented facts.rs at src-tauri/src/reasoning/facts.rs:
  * Type aliases: `EntityId = String`, `FactId = u64`, `EventId = u64`.
  * `VerbPolarity` enum: Positive / Negative / Neutral (+ Copy, Eq, Hash).
  * `Action` enum: 25 variants covering Physical / Movement / Communication /
    Social / Cognitive / Emotional / Meta + `Custom` fallback. Derives
    Debug, Clone, Serialize, Deserialize, PartialEq per SPEC §2.4.
  * `Provenance` enum: SvoParser / RustParser / LlmSuggested / Verified /
    User (+ Eq, Hash for use as HashMap key later).
  * `Event` struct per SPEC §2.3 (id, actor, action, target, instrument,
    time, source_text, confidence, provenance).
  * `FactValue` enum per SPEC §2.5 (Bool/Str/Int/Float/EntityRef/List/
    Unknown) with MANUAL PartialEq impl — compares by variant tag first,
    then by inner value. Cross-type comparisons (e.g. Bool(true) vs Int(1))
    are never equal.
  * `Fact` struct per SPEC §2.5 (id, entity, attribute, value, derived_from,
    valid_from, valid_until, provenance).
  * `FactLog` storage struct per SPEC §2.6 with all required methods:
    new, record_event, assert_fact, retract_fact, get_facts_for,
    get_current_value, get_events_in_chapter, events_between, all_events,
    all_facts. Default impl provided.
- Decision: `retract_fact` uses the last recorded event's `time` as the
  "now" anchor (FactLog has no own clock — that's WorldState's job per
  SPEC §2.7). Fallback to the fact's own `valid_from` if no events exist.
- Decision: `get_current_value` picks the active fact with the latest
  `valid_from` (using `TemporalAnchor::before/after`); ties broken by
  insertion order (last-inserted wins via `Iterator::max_by` semantics).
- Decision: `events_between` is INCLUSIVE on both endpoints
  (`!time.before(from) && !time.after(to)`).
- Wrote 7 unit tests (5 required + 2 smoke):
  test_record_event_assigns_sequential_ids, test_assert_and_retract_fact,
  test_get_current_value_returns_latest, test_get_events_in_chapter,
  test_fact_value_partial_eq, test_events_between_inclusive,
  test_fact_log_default.
- No `tokio`, no async, no `unwrap()` on external data, no `pub use` from
  other reasoning modules. Russian comments in docstrings; English
  identifiers.

Stage Summary:
- Public API exported by facts.rs:
  Types:     EntityId, FactId, EventId, VerbPolarity, Action, Provenance,
             Event, FactValue, Fact, FactLog.
  Methods:   FactLog::{new, record_event, assert_fact, retract_fact,
             get_facts_for, get_current_value, get_events_in_chapter,
             events_between, all_events, all_facts}.
  Traits:    Default for FactLog; manual PartialEq for FactValue.
- Cross-module dependency: `use crate::reasoning::timeline::TemporalAnchor;`
  — depends on timeline.rs subagent (parallel Wave 1). Will compile once
  timeline.rs is committed; SPEC §2.2 mandates the struct fields
  (chapter_num, chapter_suffix, scene_index, char_offset) and the
  before/after/same_chapter methods, which facts.rs relies on.
- No SPEC deviations. All type signatures match SPEC §2.1–§2.6 exactly.
  Extra derives (Eq, Hash on VerbPolarity/Provenance) are additive and
  do not break the contract.

---
Task ID: 1-d
Agent: full-stack-developer (rules.rs)
Task: Build rules.rs — Rule, RuleSet, default literary rule set

Work Log:
- Read docs/reasoning/SPEC.md (§2.8 Rule/RuleEffect/RuleEntity/Precondition/RuleSet contract)
- Read worklog.md (Wave 0 coordinator summary)
- Inspected src-tauri/python/svo_extract.py lines 90-150 — confirmed POSITIVE_VERBS / NEGATIVE_VERBS / NEUTRAL_VERBS sets inform Custom-action polarity mapping
- Read src-tauri/src/reasoning/mod.rs — confirmed pub use of {Rule, RuleSet, RuleEffect, RuleEntity, Precondition} (EventField NOT re-exported, OK as it's internal to RuleEffect::SetAttributeFromEvent)
- Created src-tauri/src/reasoning/rules.rs with:
  * Module doc (`//! ...`) explaining principles + payload-substitution convention
  * RuleEntity enum (Actor / Target / Specific(EntityId))
  * EventField enum (Actor / Target / Instrument / SourceText)
  * RuleEffect enum (5 variants per SPEC §2.8)
  * Precondition struct with is_satisfied(&WorldState) — handles Specific entities directly; Actor/Target return false (inference.rs Wave 2 will resolve)
  * Rule struct (name, matches, effects, preconditions)
  * RuleSet with new(), default_literary(), add(), find_matching(), len(), is_empty(), iter() + Default impl
  * Private action_matches helper using std::mem::discriminant for variant-only matching
  * 21 default rules in default_literary(): 18 canonical Action variants (a–r) + 3 catch-all Custom rules (positive/negative/neutral)
  * 9 unit tests (6 mandatory + 3 additional smoke tests)

Stage Summary:
- 21 rules in default_literary(): kill_target, wound_target, die_action, resurrect,
  move_actor, arrive_at, leave_from, know_fact, forget_fact, want_goal,
  fall_in_love, hate_target, betray_victim, marry_partner, capture_target,
  imprison_target, free_target, heal_target, custom_positive, custom_negative,
  custom_neutral
- Payload-substitution convention (documented in module doc):
  Action variants carrying payload (Move{destination}, Arrive{destination},
  Know{fact}, Forget{fact}, Want{goal}, FallInLove{partner}, Hate{target},
  Betray{victim}, Marry{partner}) use placeholder values
  (Str(String::new()) or EntityRef(String::new())) in their RuleEffect.
  inference.rs (Wave 2) MUST detect these placeholders and substitute the
  real payload from the triggering Action variant at runtime. A substitution
  table is provided in the module doc.
- Matching convention: Action::Custom matches by polarity only (verb_lemma is
  wildcard). All other variants match by discriminant (variant-only, payload is
  wildcard) — this allows Move{destination:""} rule to match Move{destination:"Замок"}.
- SPEC DEVIATION (documented in code): task brief's action_matches helper
  suggested `(a, b) => a == b` fallback. Replaced with `discriminant(a) == discriminant(b)`
  because strict == would make payload-carrying rules (move_actor, know_fact, etc.)
  non-functional — a rule with Move{destination:""} would never match a real
  Move{destination:"Замок"}. Discriminant matching is semantically correct
  ("rule matches any Move action").
- ASSUMED dependency: Precondition::is_satisfied uses `*v == self.expected`,
  which requires FactValue: PartialEq. SPEC §2.5 derives only Debug/Clone/
  Serialize/Deserialize. Wave 1 facts.rs agent MUST add PartialEq to FactValue
  derives (it's already added to Action and Provenance per SPEC §2.3-2.4,
  so it's a consistent extension).
- ASSUMED dependency: tests use WorldState::new() and WorldState::get(&EntityId, &Attribute)
  per SPEC §2.7. Wave 1 state.rs agent MUST implement these.
- No tokio, no async, no Serialize/Deserialize on Rule (it's static config).
- Russian comments in user-facing text; English identifiers.

---
Task ID: 1-c
Agent: full-stack-developer (timeline.rs)
Task: Build timeline.rs — TemporalAnchor, TimeInterval, Timeline

Work Log:
- Read mandatory inputs: SPEC.md (§2.2 TemporalAnchor), worklog.md (Task 0 context),
  parser/chapters.rs (how ParsedChapter handles sub-chapters like "28б" — numeric
  `num: u32` + alphabetic suffix in title string; reasoning layer mirrors this split
  via `chapter_num: u32` + `chapter_suffix: Option<String>`).
- Read existing reasoning/mod.rs to confirm `pub mod timeline;` already declared
  and `pub use timeline::TemporalAnchor;` re-export already wired up — no mod.rs
  changes needed (and forbidden by task scope).
- Read Cargo.toml to confirm `serde = { version = "1", features = ["derive"] }`
  is available (no extra deps needed).
- Implemented `TemporalAnchor` with all 4 fields per SPEC §2.2 and derived
  `Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash`.
- Implemented all 13 required methods: new, with_suffix, with_scene, with_offset,
  before, after, same_chapter, same_or_before, same_or_after, display_chapter,
  from_chapter_num, earliest, max.
- Implemented `Ord` + `PartialOrd` MANUALLY (not derived) to make the ordering
  rule explicit and self-documenting: chapter_num → chapter_suffix (None<Some, lex)
  → scene_index (None<Some, numeric) → char_offset. Implementation is consistent
  with derived PartialEq/Eq/Hash (compares all fields in same order).
- Implemented `TimeInterval` with `new` (debug_assert start<=end), `contains`
  (closed interval), `overlaps` (closed-interval intersection test
  `a.start <= b.end && b.start <= a.end`), `duration_chapters` (with sub-chapter
  suffix handling: equal chapter_num + different suffix = 1 step).
- Implemented `Timeline` with private `chapters: Vec<TemporalAnchor>` and
  `cursor: TemporalAnchor`. Methods: new, from_chapters (sort+dedup),
  add_chapter (binary_search insert), chapters, chapter_count, position_of
  (binary_search Ok→Some), advance_to (Err if not in timeline, cursor unchanged
  on error), cursor, next_chapter (first strictly > cursor), previous_chapter
  (last strictly < cursor), chapters_between (filter, empty if from>to).
  Default impl delegates to new().
- Wrote 7 required unit tests + 3 extra (from_chapters_dedup_and_sort,
  max_and_earliest, default_trait) = 10 tests total.
- Discovered Rust method-resolution quirk: when a type implements `Ord` AND has
  an inherent method named `max` with a different signature (`&self, &T -> T`
  vs Ord's `self, Self -> Self`), dot-syntax `a.max(&b)` resolves to `Ord::max`,
  NOT the inherent method. Fixed by using UFCS `TemporalAnchor::max(&a, &b)` in
  tests and documenting the gotcha in the method's doc-comment. Verified with
  standalone rustc repro before applying fix.
- Verified: standalone cargo check (with serde dep) passes; cargo test passes
  10/10; cargo clippy -- -D warnings clean.

Stage Summary:
- Public types exported: `TemporalAnchor`, `TimeInterval`, `Timeline`.
- `TemporalAnchor` derives `Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash`
  + manual `PartialOrd, Ord`. All 13 SPEC-required methods present.
- `TimeInterval` derives `Debug, Clone, Serialize, Deserialize`. 4 methods.
- `Timeline` (no serde derives — internal container, has private fields).
  11 methods + Default impl.
- Ordering rule (CRITICAL, used by before/after/same_or_before/same_or_after/
  position_of/next_chapter/previous_chapter/chapters_between/duration_chapters):
  chapter_num (numeric) → chapter_suffix (None<Some, lex within Some)
  → scene_index (None<Some, numeric) → char_offset (numeric). Matches SPEC §2.2.
- Design decisions:
  1. Manual Ord/PartialOrd (not derive) — for self-documentation and robustness
     against future field additions. Behavior is identical to what derive would
     produce given field declaration order.
  2. `from_chapters` leaves cursor at `earliest()` (chapter 0) — represents
     "нарратив ещё не начат". next_chapter() then returns the first chapter,
     previous_chapter() returns None. This is more consistent than auto-advancing
     to chapter[0].
  3. `add_chapter` does NOT modify cursor (separation of concerns: insertion
     vs navigation).
  4. `TimeInterval` is closed `[start, end]` — both `contains` and `overlaps`
     include boundary points. Documented in doc-comments.
  5. `duration_chapters` uses `saturating_sub` to avoid underflow on weird inputs.
  6. `advance_to` returns `Result<(), String>` per spec — error message includes
     both Debug repr and display_chapter() for diagnostics. Cursor is NOT
     modified on error (verified by test).
- Known gotcha (documented in code): `TemporalAnchor::max(&self, &T)` is shadowed
  by `Ord::max(self, Self)` for dot-syntax calls. Callers MUST use UFCS
  `TemporalAnchor::max(&a, &b)` or `(&a).max(&b)`. Doc-comment explains why.
- No SPEC deviations. All field types, method signatures, and derive lists
  match SPEC §2.2 exactly.
- No tokio/async, no `pub use` from other reasoning modules, no `unwrap()` on
  external data (only in tests). Russian comments in user-facing strings;
  English identifiers throughout.
