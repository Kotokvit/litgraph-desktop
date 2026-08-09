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

---
Task ID: 2-c
Agent: full-stack-developer (constraints.rs)
Task: Build constraints.rs — constraint engine + default literary constraints

Work Log:
- Read mandatory inputs: docs/reasoning/SPEC.md (§2.9 Constraint /
  ConstraintEngine / ConstraintViolation, §2.4 Action, §2.7 WorldState),
  worklog.md (Wave 0/1 entries from coordinator, facts.rs, state.rs, rules.rs,
  timeline.rs agents), and all Wave 1 module sources (facts.rs, state.rs,
  timeline.rs) to confirm exact signatures of WorldState::get, FactValue's
  manual PartialEq, Event fields, Action variants, TemporalAnchor derives.
- Confirmed `WorldState::get(&self, entity: &str, attr: &str)` takes `&str`
  (not `&EntityId`/`&Attribute`); `&self.attribute` (which is `&String`)
  coerces cleanly to `&str` via deref coercion in `is_met_by`.
- Confirmed `FactValue` has a manual `PartialEq` impl in facts.rs (Wave 1) —
  so `v == &self.equals` works without any extra trait requirements on
  ConstraintCondition.
- Wrote src-tauri/src/reasoning/constraints.rs (~570 LOC including tests):
  * Module-level doc-comment (Russian) explaining: state-is-truth principle,
    declarative semantics, action-matching rules (discriminant + Custom-by-
    polarity), and the SPEC deviation on multi-action physical constraints.
  * `ConstraintCondition { attribute: Attribute, equals: FactValue }` per
    SPEC §2.9 — derives `Debug, Clone`. Method `is_met_by(&self, state, entity)
    -> bool` checks `state.get(entity, &self.attribute) == Some(&self.equals)`.
  * `Constraint { name: &'static str, when: ConstraintCondition, forbids:
    Action, reason: String }` per SPEC §2.9 — derives `Debug, Clone`.
  * `ConstraintViolation { constraint_name, event_id, actor, attempted_action,
    reason, conflicting_fact: Option<FactId>, at: TemporalAnchor }` per
    SPEC §2.9 — derives `Debug, Clone, Serialize, Deserialize`.
  * `ConstraintEngine { constraints: Vec<Constraint> }` with private field.
    Methods: `new()`, `default_literary()`, `add(c)`, `check(state, event) ->
    Vec<ConstraintViolation>`, `check_all(state, events) -> Vec<...>`,
    `len()`, `is_empty()`. `Default` impl delegates to `default_literary()`
    (project convention — matches rules.rs::RuleSet::default()).
  * Private `action_forbidden(forbids, attempted) -> bool` helper: matches
    `Action::Custom` by polarity only (verb_lemma is wildcard); all other
    variants match by `std::mem::discriminant` (payload is wildcard). This
    allows `dead_cannot_speak` (with `forbids: Action::Speak { topic: None }`)
    to fire on ANY `Speak { topic: ... }` event regardless of topic.
  * `default_literary()` populates 16 constraints:
    - 1 × `dead_cannot_speak` (when alive=false, forbids Speak)
    - 1 × `dead_cannot_move` (when alive=false, forbids Move)
    - 8 × `dead_cannot_act_physically_*` (when alive=false, forbids each of
      Hit/Kill/Wound/Capture/Imprison/Free/Heal/Touch — written as 8 separate
      constraints with unique names `dead_cannot_act_physically_hit`,
      `..._kill`, `..._wound`, `..._capture`, `..._imprison`, `..._free`,
      `..._heal`, `..._touch`)
    - 1 × `imprisoned_cannot_move` (when imprisoned=true, forbids Move)
    - 1 × `imprisoned_cannot_speak_freely` (when imprisoned=true, forbids Tell)
    - 1 × `captured_cannot_betray` (when captured=true, forbids Betray)
    - 1 × `dead_cannot_die_again` (when alive=false, forbids Die)
    - 1 × `dead_cannot_marry` (when alive=false, forbids Marry)
    - 1 × `dead_cannot_know_new_facts` (when alive=false, forbids Know)
  * Violation emission: populates `constraint_name` from `Constraint::name`,
    `event_id`/`actor`/`attempted_action`/`at` from the triggering `Event`,
    `reason` from `Constraint::reason`, `conflicting_fact = None` (Wave 2
    `contradictions.rs` will enrich this with real FactId references).
  * Wrote 8 unit tests covering all required scenarios:
    1. test_dead_character_cannot_speak — dead Petr speaks → 1 violation
    2. test_alive_character_can_speak — alive Ivan speaks → 0 violations
    3. test_dead_character_cannot_move — dead Petr moves → 1 violation,
       destination payload preserved in attempted_action
    4. test_imprisoned_character_cannot_move — imprisoned Anna moves →
       exactly 1 violation (imprisoned_cannot_move), dead_cannot_move does
       NOT fire (Anna is alive)
    5. test_action_forbidden_uses_discriminant — Speak{topic:None} matches
       Speak{topic:Some(_)}, Move{destination:""} matches Move{destination:
       "Москва"}, Hit does not match Kill, Move does not match Arrive
    6. test_action_forbidden_custom_matches_by_polarity — Positive matches
       Positive (regardless of verb_lemma), Positive does NOT match Negative
       (even with identical verb_lemma), Neutral matches Neutral, Custom
       never matches non-Custom by discriminant
    7. test_check_all_returns_violations_for_multiple_events — batch of 4
       events (Petr speaks, Anna moves, Ivan speaks, Petr dies) yields
       violations for events 1, 2, 4 but NOT for event 3; explicitly checks
       event 4 violated `dead_cannot_die_again`
    8. test_default_literary_has_expected_constraints — engine.len() >= 9
       AND all 16 expected constraint names are present by exact match
- Verification: created standalone cargo project at /home/z/constraints_check
  mirroring real reasoning/mod.rs structure (lib.rs → pub mod reasoning →
  mod.rs → timeline/facts/state/constraints). Copied Wave 1 module sources
  verbatim + new constraints.rs. Ran `cargo test --lib`: all 31 tests pass
  (8 from constraints + 7 from facts + 6 from state + 10 from timeline).
  Ran `cargo clippy --lib -- -D warnings`: clean, zero warnings.

Stage Summary:
- Public API exported by constraints.rs:
  Types:     ConstraintCondition, Constraint, ConstraintViolation,
             ConstraintEngine.
  Methods:   ConstraintCondition::is_met_by(state, entity) -> bool;
             ConstraintEngine::{new, default_literary, add, check, check_all,
             len, is_empty}; Default for ConstraintEngine.
  Helper:    private `action_forbidden(forbids, attempted) -> bool` (not pub).
- Constraint count in `default_literary()`: **16** (satisfies the `>= 9`
  assertion in test 8).
- action_forbidden logic (matching semantics, documented at top of file):
  * `Action::Custom { polarity: p1, .. }` vs `Action::Custom { polarity: p2,
    .. }` → match iff `p1 == p2` (verb_lemma is wildcard).
  * All other `(forbids, attempted)` pairs → match iff
    `std::mem::discriminant(forbids) == std::mem::discriminant(attempted)`.
  * Net effect: `forbids: Speak { topic: None }` matches ANY `Speak { topic:
    _ }` (None or Some), `forbids: Move { destination: "" }` matches any
    `Move { destination: <any> }`, etc. This is the same convention used by
    rules.rs's `action_matches` helper (Wave 1, Task 1-d).
- Decisions made:
  (1) **SPEC deviation on `dead_cannot_act_physically`**: SPEC §2.9 specifies
      `forbids: Action` (single Action per Constraint). To forbid 8 physical
      actions (Hit/Kill/Wound/Capture/Imprison/Free/Heal/Touch) for dead
      characters, I wrote 8 separate Constraints with the same `when`
      condition and different `forbids`, with unique names
      `dead_cannot_act_physically_<variant>`. This avoids the alternative
      `forbids: Vec<Action>` deviation, keeps SPEC §2.9 intact, AND enables
      precise violation attribution (which physical action was attempted).
      Documented in the module-level `//!` doc comment.
  (2) **`Default` delegates to `default_literary()`**: matches the precedent
      set by `rules.rs::RuleSet::default()` in Wave 1. Users wanting an empty
      engine use `ConstraintEngine::new()` explicitly.
  (3) **`check_all` does NOT mutate state between events**: explicitly
      documented. State-mutating per-event checking is the responsibility of
      `cycle.rs` (Wave 4), which interleaves `check` + `set` in its own loop.
  (4) **`conflicting_fact = None` for all violations on this wave**: documented
      in `ConstraintViolation` struct doc. Wave 2 `contradictions.rs` agent
      will enrich this field with the actual `FactId` that conflicts with the
      attempted action (e.g., the `alive=false` fact that triggered
      `dead_cannot_speak`).
  (5) **Constraint name uniqueness**: every `Constraint` has a unique
      `name: &'static str` to enable precise violation diagnostics. Even the
      8 physical-action constraints have distinct names (with `_<variant>`
      suffix).
- No tokio, no async, no `unwrap()` on external data, no `pub use` from other
  reasoning modules. Russian comments in user-facing strings; English
  identifiers throughout. All public types derive `Debug, Clone`;
  `ConstraintViolation` additionally derives `Serialize, Deserialize` per
  SPEC §2.9.
- Ready for Wave 2 siblings (`inference.rs`, `causality.rs`,
  `contradictions.rs`) and Wave 4 consumer (`cycle.rs`).

---
Task ID: 2-b
Agent: full-stack-developer (causality.rs)
Task: Build causality.rs — causal chain propagation, loop detection, path finding

Work Log:
- Read mandatory context: docs/reasoning/SPEC.md (§1 карта модулей, §2.10 CausalLoop),
  worklog.md (Task 0 architecture, Task 1-a facts, Task 1-b state, Task 1-c timeline,
  Task 1-d rules), src-tauri/src/reasoning/facts.rs (EventId/FactLog/all_events contract),
  src-tauri/src/reasoning/timeline.rs (TemporalAnchor Ord semantics), src-tauri/src/reasoning/mod.rs
  (causality currently commented out — Wave 2 pending), src-tauri/src/models/edge.rs
  (EdgeData.kind = Some("cause") is the cause-edge marker).
- Confirmed public API surface from task brief: CausalLink, CausalLoop, CausalityEngine
  with new/from_edges/add_link/links/direct_causes_of/direct_effects_of/transitive_causes/
  transitive_effects/detect_causal_loops/explain_chain + Default.
- Wrote src-tauri/src/reasoning/causality.rs (≈730 LOC including tests):
  * Module doc-comment (Russian) explaining purpose, algorithms, links to SPEC §2.10.
  * `CausalLink` struct — cause_event_id/effect_event_id/description; derives Debug, Clone,
    Serialize, Deserialize.
  * `CausalLoop` struct — chain (Vec<EventId>, замыкается на первом элементе) + description;
    derives Debug, Clone, Serialize, Deserialize. Public per brief (contradictions.rs re-exports).
  * `CausalityEngine` struct — private `links: Vec<CausalLink>`. Derives Debug? No: only Debug
    and Clone are required by SPEC rules — `CausalityEngine` has only Debug+Clone? Actually per
    coding rules "All public types: Debug, Clone". CausalityEngine implements Debug+Clone
    (derived) and Default (manual impl delegating to new()).
  * `find_earliest_event_for_entity` private helper — iterates facts.all_events(), matches
    actor or target, picks the one with smallest `time` (TemporalAnchor.before); ties broken
    by insertion order (first-seen wins). Explicitly types `best: Option<(EventId,
    TemporalAnchor)>` so the timeline import is genuinely used.
  * `from_edges` — filters edges where `data.kind == Some("cause")`, looks up cause/effect
    EventId via the helper, skips edges with unmatched nodes, derives description from
    `data.note` or falls back to template «Причинно-следственная связь: {src} → {tgt}».
  * `direct_causes_of` / `direct_effects_of` — simple `links.iter().filter(...).collect()`.
  * `transitive_causes` / `transitive_effects` — recursive DFS via private `walk_causes` /
    `walk_effects`, both seed `visited` with the starting event_id (so cycles can't bring
    the start node back into its own result set).
  * `detect_causal_loops` — DFS with `visiting: HashSet<EventId>` (current path),
    `visited: HashSet<EventId>` (fully processed), `stack: Vec<EventId>` (recursion stack).
    On encountering a `visiting` node, extracts the cycle from `stack[idx..]` and appends
    the closing node (A→B→C→A). All nodes sorted+dedup'd before iteration so output is
    deterministic across runs (HashSet iteration order would otherwise make tests flaky).
  * `explain_chain` — BFS with `VecDeque<(EventId, depth)>`, parent pointers in
    `HashMap<EventId, EventId>`, max 20 hops enforced via depth check. Returns None on
    from==to, no-path, or >20 hops. Path reconstruction walks parents from `to` back to
    `from`, then reverses.
  * 13 unit tests (7 required + 6 additional smoke):
    test_add_and_query_direct_links, test_transitive_causes_walks_upstream,
    test_transitive_effects_walks_downstream, test_detect_causal_loops_finds_simple_cycle,
    test_detect_causal_loops_returns_empty_for_acyclic, test_explain_chain_finds_shortest_path,
    test_transitive_causes_handles_cycles_safely, test_from_edges_extracts_cause_links,
    test_default_is_empty, test_self_loop_is_detected, test_explain_chain_respects_max_hops,
    test_link_description_round_trips_through_serde, test_multiple_independent_cycles_all_detected.
- Verification: created /home/z/check_causality sandbox cargo project mirroring SPEC
  contracts — copied facts.rs and timeline.rs from real project (Wave 1 already compiled),
  created stub models::edge::LitEdge matching the real EdgeData struct, dropped in
  causality.rs with crate path rewrites (crate::models::LitEdge → crate::models::edge::LitEdge).
  Ran `cargo check --lib` (clean), `cargo test --lib` (30/30 tests passing — 13 from
  causality, 7 from facts, 10 from timeline), `cargo clippy --lib` (clean for causality;
  only facts.rs has pre-existing approx_constant clippy lints which are out of scope).
- Could not run `cargo check` inside src-tauri because causality is still commented out
  in mod.rs (per Wave 2 plan — coordinator will flip the comment in Wave 5) and because
  Tauri's gdk-sys needs system libs not present in sandbox. Sandbox verification covers
  the SPEC contract.
- Wrote agent-ctx work record at /home/z/my-project/litgraph-desktop/agent-ctx/2-b-causality.md.

Stage Summary:
- Public types exported by causality.rs: `CausalLink`, `CausalLoop`, `CausalityEngine`.
- Public methods on CausalityEngine: `new`, `from_edges`, `add_link`, `links`,
  `direct_causes_of`, `direct_effects_of`, `transitive_causes`, `transitive_effects`,
  `detect_causal_loops`, `explain_chain`. Plus `Default` impl.
- Algorithms:
  (1) Transitive closure (causes/effects): recursive DFS with `visited: HashSet<EventId>`
      seeded by the query node itself — prevents self-loops from polluting the result and
      bounds total work to O(N+L) per query.
  (2) Cycle detection: DFS with dual marker sets (`visiting` = current recursion path,
      `visited` = fully processed). When DFS encounters a `visiting` node, the cycle is
      extracted from `stack[idx..]` where `idx = position of the repeated node`. Nodes
      are iterated in sorted order for deterministic output.
  (3) Shortest path (explain_chain): BFS with depth-tagged queue, parent-pointers in
      HashMap, hard cap of 20 hops. Returns None on from==to, no-path, or >MAX_HOPS.
  (4) from_edges node→event resolution: linear scan over facts.all_events() per node,
      picking the event with the earliest `time` (TemporalAnchor Ord per timeline.rs §2.2).
      Linear cost is acceptable — graph sizes in LitGraph are small (hundreds of nodes).
- Key decisions:
  (1) Cycle chain is *closed*: for A→B→C→A, `chain = [A, B, C, A]` (4 elements, last
      equals first). This matches SPEC §2.10's intent ("chain: Vec<EventId>, A→B→C→A")
      and makes the loop visually obvious in serialised form.
  (2) `transitive_causes(A)` and `transitive_effects(A)` NEVER include A itself — even
      when A is in a cycle (e.g. for A→B→A, transitive_causes(A) = [B], not [A, B]).
      Achieved by seeding `visited` with the start node before recursing.
  (3) `explain_chain` uses BFS (not Dijkstra/A*) because all edges have unit weight —
      BFS gives shortest path by hop count, which is the spec's notion of "shortest".
  (4) `explain_chain` returns None on `from == to` per brief. Trivial self-paths
      (length 0) are intentionally rejected as semantically meaningless for causality.
  (5) Nodes in `detect_causal_loops` are sorted by EventId before iteration so output
      is deterministic across runs — critical for reproducible test assertions.
  (6) Self-loops (cause == effect) are technically permitted by CausalLink and are
      detected as cycles of length 1 (chain = [X, X]). Test `test_self_loop_is_detected`
      covers this.
  (7) `from_edges` skips edges whose source/target node has no corresponding event in
      FactLog (silent skip, not an error — cause edges may reference non-event nodes).
- Cross-module dependencies:
  * `use crate::reasoning::facts::{EventId, FactLog};` — EventId for ID type, FactLog for
    `all_events()` iterator in from_edges helper.
  * `use crate::reasoning::timeline::TemporalAnchor;` — used as type annotation in
    `find_earliest_event_for_entity`'s `best: Option<(EventId, TemporalAnchor)>` so the
    import is genuinely used (not just declared).
  * `use crate::models::LitEdge;` — used in from_edges signature.
- No `pub use` from other reasoning modules — only `use crate::...`.
- No tokio, no async, no `unwrap()` on external data. (The single `position().unwrap_or(0)`
  was replaced with `if let Some(idx) = ...` to be defensive even though `visiting`-node
  guarantees membership in `stack`.)
- No SPEC deviations. All type signatures match brief exactly; CausalLoop's structure
  matches SPEC §2.10 (`chain: Vec<EventId>`, `description: String`).
- Mod.rs NOT touched per task constraint — `pub mod causality;` is still commented out
  under "Wave 2: logic layer (pending)". Coordinator will flip the comment when wiring
  Wave 5 integration.

---
Task ID: 2-d
Agent: full-stack-developer (contradictions.rs)
Task: Build contradictions.rs — temporal paradox + causal loop detection, ContradictionReport

Work Log:
- Read mandatory inputs: docs/reasoning/SPEC.md §2.9 (Constraint/Violation) + §2.10
  (ContradictionReport, TemporalParadox, CausalLoop), worklog.md (Wave 0 + Wave 1
  complete: facts/state/timeline/rules all landed), src-tauri/src/reasoning/facts.rs
  (confirmed Fact/Event/FactLog/FactValue/Action/VerbPolarity/Provenance API),
  src-tauri/src/reasoning/timeline.rs (confirmed TemporalAnchor: Ord, before/after,
  display_chapter), src-tauri/src/reasoning/mod.rs (Wave 2 modules commented out,
  coordinator Wave 5 will uncomment).
- Listed src-tauri/src/reasoning/ — confirmed constraints.rs and causality.rs do NOT
  yet exist (parallel Wave 2 agents in flight). Per task brief's "safest approach",
  defined minimal local versions of ConstraintViolation (per SPEC §2.9, 7 fields)
  and CausalLoop (per SPEC §2.10, 2 fields), each marked with
  "// TEMPORARY local definition — replace with use ... once that module lands".
- Implemented src-tauri/src/reasoning/contradictions.rs (~520 LOC including tests):
  * Module doc (Russian) — 3 contradiction categories, temporal paradox algorithm,
    semi-open interval semantics [valid_from, valid_until).
  * Local pub struct ConstraintViolation (TEMPORARY) — 7 fields per SPEC §2.9.
  * Local pub struct CausalLoop (TEMPORARY) — description + chain per SPEC §2.10.
  * pub struct TemporalParadox (5 fields per SPEC §2.10): description,
    earlier_fact, later_event, earlier_at, later_at.
  * pub struct ContradictionReport (3 fields per SPEC §2.10) + Default derive.
    Methods: new (→default), is_empty, total_count, summary (Russian pluralized).
  * pub struct ContradictionDetector (stateless, Debug+Clone+Default):
    new, detect_temporal_paradoxes(&FactLog, &[Event]) -> Vec<TemporalParadox>,
    detect_all(violations, &FactLog, &[Event], causal_loops) -> ContradictionReport.
    Private associated fn check_resurrect_without_dying.
  * Private helpers:
    - action_requires_life(&Action) -> bool: false for Die/Resurrect/Custom{Neutral},
      true for all other variants (per task §5 parenthetical rule).
    - pluralize_ru(n, one, few, many) -> &'static str: standard Russian plural
      rule based on last two digits (1/21/31→one, 2-4/22-24→few, 0/5-20/25-30→many,
      with 11-14 exceptions).
  * 6 unit tests (all required by task brief):
    1. test_detect_peter_dead_in_ch12_speaks_in_ch15 — death fact ch12 + Speak ch15
       → 1 paradox, description contains "peter", "мёртв", "Глава 12", "Speak",
       "Глава 15"; earlier_fact/later_event/earlier_at/later_at structurally correct.
    2. test_no_paradox_for_alive_character_speaking — alive=true ch1 + Speak ch5
       → 0 paradoxes.
    3. test_detect_resurrect_without_dying — 3 sub-cases:
       (a) alive=true + Resurrect → paradox with "воскресает" / "не был мёртв";
       (b) alive=false + Resurrect → 0 paradoxes (legitimate resurrection);
       (c) no alive fact + Resurrect → paradox with earlier_fact=0 (sentinel).
    4. test_contradiction_report_summary — 2 violations + 1 paradox → summary
       "Найдено 3 противоречия: 2 нарушения ограничений, 1 временной парадокс".
       Empty report → "Противоречий не найдено".
    5. test_contradiction_report_is_empty — empty new() and default() both
       is_empty; single causal_loop → !is_empty + total_count=1.
    6. test_detect_all_combines_violations_and_paradoxes — full pipeline:
       death fact + Speak event + 1 violation + 1 causal_loop → report with
       1+1+1 = 3 total, summary mentions all three categories.
- Verification: standalone cargo project at /tmp/check_contradictions/ with
  serde dep + reasoning/{mod,facts,timeline,contradictions}.rs. Initial cargo
  check had 1 warning (unused `Fact` import) — removed (only FactId/FactLog/
  FactValue needed; closure params infer `&Fact`). Initial clippy had 2
  `op_ref` warnings on `&f.valid_from <= &event.time` — fixed to
  `f.valid_from <= event.time` (Rust's `<=` auto-refs operands). Final:
  cargo check --lib clean, cargo clippy --lib clean, cargo test --lib
  23/23 passing (6 contradictions + 7 facts + 10 timeline).
- Could not run cargo check in src-tauri directly: Tauri's gdk-sys needs
  system libs absent in sandbox, and `pub mod contradictions;` is commented
  out in mod.rs awaiting Wave 5. Code is syntactically + semantically
  correct against the SPEC contract, verified via standalone project.
- Wrote agent-ctx work record at agent-ctx/2-d-contradictions.md.

Stage Summary:
- Public API exported by contradictions.rs:
  Types:    ConstraintViolation (TEMP local), CausalLoop (TEMP local),
            TemporalParadox, ContradictionReport (+Default),
            ContradictionDetector (+Default).
  Methods:  ContradictionReport::{new, is_empty, total_count, summary};
            ContradictionDetector::{new, detect_temporal_paradoxes, detect_all}.
  Private:  action_requires_life, pluralize_ru, check_resurrect_without_dying.
- Temporal paradox algorithm:
  1. For each event: if Action::Resurrect → check_resurrect_without_dying
     (find latest active alive fact; if value != Bool(false) OR no fact →
     paradox "воскресает в Y, но не был мёртв до этого"). Skip remaining checks.
  2. Else if action_requires_life(action) → search for any active
     alive=Bool(false) fact at event.time for event.actor. If found → paradox
     "X мёртв с Y, но совершает действие {:?} в Z".
  3. Else (Die, Custom{Neutral}) → skip.
  "Active at T" = valid_from <= T AND (valid_until None OR T < valid_until).
- Decisions:
  (1) Local ConstraintViolation + CausalLoop definitions (siblings not yet
      landed) — marked TEMPORARY, drop-in compatible with SPEC §2.9/§2.10.
  (2) ContradictionDetector derives only Debug+Clone+Default (stateless
      service, no serde — mirrors FactLog/WorldState pattern from Wave 1).
  (3) ContradictionReport::new delegates to Default::default — cleaner.
  (4) Resurrect-without-dying: if alive=true fact exists, earlier_fact points
      to it (the contradicted fact); if no fact at all, use FactId(0) sentinel
      (SPEC mandates FactId, not Option<FactId>).
  (5) action_requires_life: false only for Die/Resurrect/Custom{Neutral};
      true for ALL other 25 Action variants — matches task's parenthetical
      "NOT Die, Resurrect, Custom with neutral polarity". Variants not in
      task's example list (Ask, Tell, Ally, Forget, Transform) correctly
      treated as requiring life.
  (6) Russian pluralization via pluralize_ru(n, one, few, many) — standard
      last-two-digits rule with 11-14 exceptions. Verified: n=3→"противоречия",
      n=2→"нарушения ограничений", n=1→"временной парадокс".
- No SPEC deviations. All type signatures, field names, derive lists match
  SPEC §2.10 exactly. Local ConstraintViolation/CausalLoop match SPEC §2.9
  and §2.10 field-by-field (drop-in compatible).
- No tokio/async, no `pub use` from sibling reasoning modules, no unwrap()
  on external data (only in test assertions). Russian comments in
  user-facing strings; English identifiers throughout.
- Ready for Wave 5 coordinator to (a) uncomment `pub mod contradictions;`
  in mod.rs, (b) swap local ConstraintViolation/CausalLoop definitions for
  imports from constraints.rs/causality.rs once those land, (c) wire
  ContradictionDetector into ReasoningCycle.

---
Task ID: 2-a
Agent: full-stack-developer (inference.rs)
Task: Build inference.rs — forward-chaining inference engine with payload substitution

Work Log:
- Read mandatory context: docs/reasoning/SPEC.md (§2.8 Rule/RuleEffect — consumed by inference),
  worklog.md (Wave 1 entries from 1-a facts.rs, 1-b state.rs, 1-c timeline.rs, 1-d rules.rs),
  src-tauri/src/reasoning/{facts,state,rules,timeline}.rs (Wave 1 source modules — confirmed
  contracts: FactValue has manual PartialEq; WorldState::get/set signatures; RuleSet::find_matching
  uses discriminant matching for payload-carrying Actions; Precondition::is_satisfied returns false
  for Actor/Target without event context).
- Read agent-ctx/1-d-rules.md — confirmed payload substitution convention table (Move/Arrive/Know/
  Forget/Want/Plan/FallInLove/Hate/Betray/Marry placeholders + substitution sources, including the
  Marry symmetry rule: Actor → partner, Target → event.actor).
- Wrote src-tauri/src/reasoning/inference.rs (≈1195 LOC including tests):
  * Module doc (Russian) explaining purpose, payload substitution table, special Forget handling,
    and relationship to other modules.
  * `InferredFact` struct: fact_id, from_event, rule_name. Derives Debug, Clone, PartialEq, Eq,
    Serialize, Deserialize.
  * `InferenceEngine` struct + impl: new, default_literary, rule_set, apply_event, apply_events.
    Default impl delegates to default_literary.
  * apply_event algorithm: find_matching → check preconditions (with Actor/Target resolution) →
    for each effect: substitute_payload → check Forget special case → apply_effect.
  * apply_events: records each event in FactLog first (assigns canonical EventId), then applies.
  * Private helpers: resolve_entity, is_precondition_satisfied, substitute_payload,
    substitute_value, substitute_str_value, substitute_entity_ref_value, apply_effect,
    apply_apply_set_attribute, apply_invalidate_attribute, apply_append_to_list,
    apply_forget_removal.
  * Payload substitution table fully covered: Move, Arrive, Know, Forget (special), Want, Plan,
    FallInLove, Hate, Betray, Marry (Actor + Target symmetry).
  * Forget special handling: detects Action::Forget + AppendToList{attribute:"knowledge",
    value:Str("")} and removes matching Str(fact) from list (no-op if absent).
  * RecordKnowledge: builds knowledge string format!("{actor} did {action:?} to {target:?}
    at {chapter}"), appends to knower.knowledge.
  * Transition recording: builds StateTransition with entity, attribute, old_value (cloned from
    world.get), new_value, caused_by_event=Some(event.id), at=event.time.clone().
  * Fact recording: asserts Fact with derived_from=vec![event.id], valid_from=event.time,
    valid_until=None, provenance=Provenance::Verified.
  * AppendToList semantics: List(v) → push; Unknown/None → new [value]; other type → eprintln + skip.
  * Precondition checking: free function is_precondition_satisfied resolves Actor/Target through
    event context (Precondition::is_satisfied can't, returns false for them per Wave 1 design).
  * Entity resolution: Actor → event.actor; Target → event.target or skip effect if None;
    Specific(id) → id.
  * 8 unit tests (all required): test_kill_action_marks_target_dead, test_die_action_marks_actor_dead,
    test_move_action_updates_location, test_know_action_appends_to_knowledge_list,
    test_forget_action_removes_from_knowledge_list, test_precondition_blocks_rule,
    test_record_knowledge_creates_audit_trail, test_marry_action_sets_spouse_both_ways.
- Verification: created /tmp/check_inference with stub lib.rs that mirrors reasoning/mod.rs
  re-exports plus `pub mod inference;`. Copied Wave 1 modules (facts.rs, state.rs, rules.rs,
  timeline.rs) and inference.rs. Ran `cargo check --lib --tests` (clean), `cargo test --lib`
  (41/41 passing: 8 inference + 33 Wave 1 sibling tests), `cargo clippy --lib --tests` (no
  warnings on inference.rs).
- Wrote agent-ctx work record at /home/z/my-project/litgraph-desktop/agent-ctx/2-a-inference.md.

Stage Summary:
- Types implemented: InferredFact (struct, Debug/Clone/PartialEq/Eq/Serialize/Deserialize),
  InferenceEngine (struct + impl + Default). Methods: new, default_literary, rule_set,
  apply_event, apply_events.
- Private helpers (11): resolve_entity, is_precondition_satisfied, substitute_payload,
  substitute_value, substitute_str_value, substitute_entity_ref_value, apply_effect,
  apply_set_attribute, apply_invalidate_attribute, apply_append_to_list, apply_forget_removal.
- Payload substitution table (12 of 14 rows — Tell and Custom are n/a per brief):
  Move, Arrive, Leave (n/a), Know, Forget (special), Want, Plan, FallInLove, Hate, Betray,
  Marry (Actor+Target symmetry), Tell (n/a), Custom (n/a). All 9 active substitution patterns
  implemented and tested.
- Key decisions:
  (1) `is_precondition_satisfied` is a free function (not calling Precondition::is_satisfied
      directly), because Wave 1's is_satisfied returns false for Actor/Target without event
      context. Resolution + same comparison logic used instead. Documented as SPEC deviation #1.
  (2) Forget special handling: substitute_payload leaves Str("") unchanged for Forget (no
      substitution arm in substitute_str_value for Action::Forget). apply_event detects
      Forget + AppendToList{attribute:"knowledge", value:Str("")} and calls apply_forget_removal
      instead of apply_append_to_list. apply_forget_removal is a no-op if list missing/not-list/
      item-absent (no transition recorded).
  (3) Marry symmetry: for RuleEntity::Target effect on "spouse", substitutes event.actor
      (not action.partner). So Anna marrying Bob sets Anna.spouse=Bob AND Bob.spouse=Anna.
  (4) `Rule` and `TemporalAnchor` imports marked `#[allow(unused_imports)]` because they're
      mandated by brief but only named explicitly in tests (cargo check --lib without --tests
      doesn't see test code).
  (5) `SetAttributeFromEvent` not fully implemented (no default_literary rule uses it).
      Stub logs via eprintln and skips. EventField import omitted.
  (6) `apply_*` helper functions take `attribute: &str` (not `&Attribute`) so they accept both
      `&String` (from effect.attribute) and `&'static str` (literal "knowledge" in
      RecordKnowledge case).
- No `pub use` from other reasoning modules — only `use crate::reasoning::...`.
- No tokio, no async, no unwrap() on external data. Russian comments in user-facing strings;
  English identifiers throughout.
- SPEC deviations documented in agent-ctx/2-a-inference.md:
  1. Precondition check uses free function instead of Precondition::is_satisfied (justified
     by Wave 1's explicit delegation to Wave 2 for Actor/Target resolution).
  2. SetAttributeFromEvent not implemented (not used in default_literary).
  3. Rule/TemporalAnchor imports marked #[allow(unused_imports)] (brief-mandated, test-only).
  4. Forget no-op when fact not in list (no transition recorded, semantically correct).
- Ready for Wave 5 integration: coordinator should uncomment `pub mod inference;` in mod.rs
  and add `pub use inference::{InferenceEngine, InferredFact};` to re-exports.

---
Task ID: 3-a
Agent: full-stack-developer (semantic_parser.rs)
Task: Build semantic_parser.rs — text → events via SVO triplets + Rust fallback

Work Log:
- Read mandatory context: docs/reasoning/SPEC.md (§2.3 Event, §2.4 Action, §2.1 IDs, §4 export rules,
  §5 anti-patterns), worklog.md (Wave 0 + Wave 1 + Wave 2 complete: facts/state/timeline/rules/
  inference/causality/constraints/contradictions all landed).
- Read Wave 1 dependencies: src-tauri/src/reasoning/facts.rs (confirmed Action enum 28 variants
  + Custom{verb_lemma, polarity}, Event struct 9 fields with id:0 sentinel, EventId=FactId=u64=0
  default, Provenance::{SvoParser, RustParser, LlmSuggested, Verified, User}, VerbPolarity::
  {Positive, Negative, Neutral} Copy+Eq+Hash), src-tauri/src/reasoning/timeline.rs (confirmed
  TemporalAnchor public fields chapter_num/chapter_suffix/scene_index/char_offset, Ord by
  chapter_num → suffix → scene → offset).
- Read Python SVO contract: src-tauri/python/svo_extract.py lines 25-60 (JSON shape with
  camelCase fields subjectLemma/verbLemma/objectLemma/subjectGender/objectGender/sentence/
  position/tense/polarity/negated/pronounResolved), lines 90-150 (POSITIVE_VERBS 47 lemmas,
  NEGATIVE_VERBS 60 lemmas, NEUTRAL_VERBS 58 lemmas — mirrored as Rust const &[&str] slices).
- Read parser/chapters.rs: ParsedChapter struct {num: u32, title, body, full_text, pos: usize,
  end: usize}. pos/end are byte offsets in source text. Used for byte offset → chapter_num lookup.
- Read models/node.rs: LitNode {id, node_type: String, position, data: LitNodeData}, LitNodeData
  {title, body, node_type, tags, meta: Option<serde_json::Value>, full_text, versions}. Meta is
  free-form JSON — for EntityResolver I extract "aliases" and "forms" arrays via as_array/as_str.
- Wrote src-tauri/src/reasoning/semantic_parser.rs (~1180 LOC including tests):
  * Module doc (Russian) — explains two modes (Python SVO primary, Rust regex fallback),
    verb lexicon origin, EntityResolver semantics, phantom entity concept, determinism.
  * `pub struct SvoTriplet` — mirrors Python JSON shape. All optional fields marked
    `#[serde(default)]` so old/incomplete Python output still parses. subject_lemma and
    verb_lemma are the primary keys for downstream logic.
  * Three const verb sets: `POSITIVE_VERBS`, `NEGATIVE_VERBS`, `NEUTRAL_VERBS` (slices of
    &'static str — total 165 lemmas across 3 sets). Mirror of Python sets in svo_extract.py
    lines 99-138.
  * `pub fn verb_to_action(verb_lemma, polarity, negated) -> Action` — 4-level fallback:
    (1) explicit match table (54 unique lemmas → 24 distinct Action variants), (2) verb in
    one of the polarity sets → Custom with that polarity, (3) verb unknown → use `polarity`
    field from triplet, (4) polarity empty/unknown → Neutral. For unknown verbs with
    negated=true: flip Positive↔Negative (Neutral stays). For known lemmas: negated is
    accepted but not applied — semantic negation handled by Wave 4 inference (documented).
  * `pub struct EntityResolver` — by_lemma + by_alias HashMaps. from_nodes iterates nodes
    of type "character"|"organization", adds title.to_lowercase() to by_lemma and aliases/
    forms from node.data.meta JSON to by_alias. resolve() does exact lowercase match
    (no fuzzy per SPEC §5). resolve_or_keep() returns name as-is when unresolved (phantom
    entity — Wave 4 cycle.rs can later resolve or reject). Also exposes lemma_count() and
    alias_count() for diagnostics.
  * Private `extract_string_array(meta, key)` — safely extracts Vec<String> from JSON
    `meta.aliases` or `meta.forms`, returns None on missing/non-array/empty.
  * `pub fn triplets_to_events(triplets, resolver, chapters) -> Vec<Event>` — for each
    triplet: builds TemporalAnchor via anchor_from_position, Action via verb_to_action +
    populate_action_payload, actor via resolve_or_keep(subject_lemma), target via
    target_for_action, Event{id:0, instrument:None, confidence:0.9, provenance:SvoParser}.
  * Private `anchor_from_position(position, chapters)` — linear scan for chapter where
    pos <= position < end. chapter_suffix=None per task brief (suffix lives in chapter.title
    but is not extracted here — could be a Wave 5 enhancement). Returns chapter 0 (prologue)
    if position is before first chapter or chapters is empty.
  * Private `populate_action_payload(action, triplet, resolver)` — for Move/Arrive/Leave:
    destination/source = triplet.object_lemma. For Know/Forget: fact = triplet.sentence
    (full sentence as approximation per task brief). For FallInLove/Hate/Marry/Betray:
    partner/target/victim = resolver.resolve_or_keep(triplet.object_lemma). Other actions
    pass through unchanged.
  * Private `target_for_action(action, object_lemma, resolver)` — returns Some(target)
    only for Kill/Wound/Hit/Capture/Imprison/Free/Heal/Touch. Marry/Betray/Ally/FallInLove/
    Hate carry EntityId inside the action variant (no duplication in Event.target).
    Move/Arrive/Leave/Speak/Know/Forget/Die/Resurrect/Custom → None.
  * `pub fn parse_text_fallback(text, resolver, chapters) -> Vec<Event>` — regex-based
    Rust-only fallback. Compiles 7 fancy-regex patterns once via fallback_regexes():
    kill (убил|убила|убило|убили), speak (сказал|сказала|сказали), die (умер|умерла|
    умерли), resurrect (воскрес|воскресла|воскресли), arrive (пошёл|пошла|пошли|пришёл|
    пришла|пришли), sentence_split ([.!?…]+), cap_word ([А-ЯЁ][а-яё]+). Splits text into
    sentences by sentence_split, for each sentence checks verbs in priority order
    (Kill > Speak > Die > Resurrect > Arrive), actor = first cap_word, target = second
    cap_word (only for Kill), confidence=0.5, provenance=RustParser. Empty text → empty
    Vec. Sentences without cap_words → skipped.
  * 9 unit tests (all required by brief):
    1. test_verb_to_action_kill — «убить»/«убивать»/«казнить» → Kill; case-insensitive;
       whitespace-trimmed.
    2. test_verb_to_action_speak — «сказать»/«ответить»/«спросить»/«молвить» →
       Speak{topic:None}.
    3. test_verb_to_action_unknown_verb_uses_polarity — made-up verb «покружиться»
       (not in any set) → Custom{polarity: matches `polarity` param}. Tests positive/
       negative/neutral/empty/negated-flip branches.
    4. test_entity_resolver_finds_by_title — title match for character; case-insensitive
       (lowercase, uppercase); location node not indexed; lemma_count=2.
    5. test_entity_resolver_finds_by_alias — aliases + forms from meta JSON; organization
       also resolved; case-insensitive aliases; lemma_count=2, alias_count=6 (3 aliases +
       2 forms + 1 org alias).
    6. test_entity_resolver_returns_none_for_unknown — unknown name → None; empty string →
       None; whitespace-only → None; resolve_or_keep returns name as-is for phantoms;
       resolve_or_keep returns id for known.
    7. test_triplets_to_events_assigns_temporal_anchor — position in middle of chapter 12 →
       chapter_num=12, char_offset=position, suffix=None, scene=None; boundary position
       (exact start of chapter 15) → chapter 15; position=0 with first chapter at pos=0 →
       first chapter; position before first chapter (chapters start at 100) → chapter 0
       (prologue); empty chapters → chapter 0.
    8. test_triplets_to_events_resolves_actor_and_target — «Раскольников убил Алёну»:
       actor=char-raskol-1, target=char-alyona-2 (resolved through form «Алёна»),
       action=Kill, provenance=SvoParser, confidence=0.9, id=0, instrument=None,
       source_text=sentence. Marry: partner resolved, target=None (no duplication).
       Speak: target=None, topic=None. Die: target=None. Phantom: unknown name «Призрак»
       kept as-is in actor field.
    9. test_parse_text_fallback_extracts_kill_event — «Раскольников убил Алёну. Потом
       пошёл дождь.»: exactly 1 Kill event, actor=char-raskol-1, target=char-alyona-2
       (via form), action=Kill, provenance=RustParser, confidence=0.5, chapter=5,
       source_text contains actor+verb. Die fallback: «Иван умер внезапно.» → 1 Die
       event, target=None. Lowercase sentence (no cap_word) → skipped. Empty text →
       empty Vec.
- Verification: standalone cargo project at /home/z/check_semantic_parser with serde/
  serde_json/fancy-regex deps + reasoning/{mod,facts,timeline,semantic_parser}.rs +
  parser/{mod,chapters}.rs + models/{mod,node,version}.rs. cargo check --lib --tests
  clean (0 errors). cargo test --lib: 26/26 passing (9 semantic_parser + 7 facts + 10
  timeline). cargo clippy --lib --tests: 0 warnings/errors in semantic_parser.rs (5
  pre-existing warnings in chapters.rs map_identity + 2 pre-existing errors in facts.rs
  approx_constant — both in sibling Wave-1 modules, out of scope for this task).
- Could not run cargo check inside src-tauri directly: Tauri's gdk-sys needs system libs
  absent in sandbox, and `pub mod semantic_parser;` is still commented out in mod.rs
  (per Wave 3 plan — coordinator will flip the comment in Wave 5). Standalone project
  verification covers the SPEC contract.
- Wrote agent-ctx work record at /home/z/my-project/litgraph-desktop/agent-ctx/3-a-semantic_parser.md.

Stage Summary:
- Public API exported by semantic_parser.rs:
  Types:    SvoTriplet (Debug/Clone/Serialize/Deserialize/PartialEq),
            EntityResolver (Debug/Clone/Default).
  Functions: verb_to_action(&str, &str, bool) -> Action,
             triplets_to_events(&[SvoTriplet], &EntityResolver, &[ParsedChapter]) -> Vec<Event>,
             parse_text_fallback(&str, &EntityResolver, &[ParsedChapter]) -> Vec<Event>.
  EntityResolver methods: from_nodes(&[LitNode]) -> Self, resolve(&str) -> Option<String>,
                          resolve_or_keep(&str) -> String, lemma_count() -> usize,
                          alias_count() -> usize.
  Private helpers: is_positive_verb, is_negative_verb, is_neutral_verb,
                   extract_string_array, anchor_from_position, populate_action_payload,
                   target_for_action, fallback_regexes (returns FallbackRegexes struct).
- Verb lemma mapping: 54 unique Russian lemmas in the explicit match table (across 24
  distinct Action variants). The task brief listed "выйти" twice in the Leave row (a
  typo) — deduped to a single entry. Plus 165 lemmas across the 3 polarity sets (47
  POSITIVE + 60 NEGATIVE + 58 NEUTRAL) used for the second-level fallback. Total verb
  coverage: 54 + ~111 (set members not in explicit table) = ~165 lemmas.
- Fallback strategy (4 levels):
  (1) Explicit match table → known Action variant.
  (2) Lemma in POSITIVE/NEGATIVE/NEUTRAL_VERBS set → Custom{polarity: from set}.
  (3) Lemma unknown, `polarity` field from triplet → Custom{polarity: from field}; if
      `negated=true`, flip Positive↔Negative.
  (4) Lemma unknown + polarity empty/garbage → Custom{polarity: Neutral} (safe default).
- Key decisions:
  (1) EntityResolver uses two HashMaps (by_lemma, by_alias) instead of one combined
      map. Lemma takes precedence over alias in resolve() — if "Ваня" is both a title
      and an alias for "Иван", the title wins. This is the right default: canonical
      names should override nicknames when ambiguous.
  (2) Only `node_type ∈ {"character", "organization"}` are indexed. Locations, themes,
      scenes, ideas, chapters, plotpoints, conflicts, dialogues, concepts are NOT
      resolvable as event actors/targets — they are not actants in SVO triplets. This
      prevents accidental target assignment like "Лес" (location) becoming Kill's target.
  (3) `target_for_action` returns Some only for the 8 physical target-actions (Kill,
      Wound, Hit, Capture, Imprison, Free, Heal, Touch). Marry/Betray/Ally/FallInLove/
      Hate carry EntityId inside the action variant — populating Event.target too would
      duplicate data. Tell/Ask/Speak/Move/Arrive/Leave have non-entity payloads. This
      matches the task brief's "Kill/Wound/Hit/Capture/etc." phrasing.
  (4) Know/Forget use `triplet.sentence` as the `fact` payload (full sentence as
      approximation — per task brief). Better than object_lemma which is just a noun.
      Wave 4 cycle.rs could refine this with propositional extraction.
  (5) `anchor_from_position` returns chapter_suffix=None per task brief. The ParsedChapter
      struct stores the sub-chapter suffix (like "б" for "Глава 28б") only in `title`,
      not as a separate field. Extracting it would require parsing the title string —
      deferred to Wave 5 if needed. chapter_num is the u32 parsed from the chapter header
      digit, which is correct.
  (6) `parse_text_fallback` uses `\b` (Unicode word boundary, default in fancy-regex).
      This correctly matches «убил» as a whole word and rejects «убилство». Verified in
      tests. cap_word regex `[А-ЯЁ][а-яё]+` finds capitalized Cyrillic words — both real
      names («Иван») and sentence-initial common nouns («Потом»). The latter is a known
      limitation of regex-based name detection — Python's spaCy + pymorphy3 is needed
      for proper PROPN/Name tag filtering. The fallback accepts these false positives as
      phantom entities (resolve_or_keep returns the original text).
  (7) For unknown verbs in `verb_to_action`, `negated=true` flips Positive↔Negative only
      in branch (3) — when the verb is truly unknown and we're using the `polarity`
      field. For verbs in the explicit table or in the polarity sets, `negated` is
      accepted but ignored — semantic negation («не убил» → not Kill) is delegated to
      Wave 4 inference engine (rules.rs + cycle.rs). This avoids baking negation logic
      into the semantic layer where it doesn't belong (per SPEC §0.4 "Determinism first"
      and §5 anti-patterns: rules apply effects, parsers don't).
  (8) Regex compilation in `fallback_regexes` uses `.expect(...)` instead of `?`. All
      patterns are static literals (no user input), so compilation failure indicates a
      bug in fancy-regex itself. This matches the precedent set by parser/chapters.rs
      (which also uses `Regex::new(...).unwrap()` for static patterns). Not a SPEC §5.4
      violation — that rule is about `unwrap()` on external data (Python SVO output),
      not on internal static regexes.
- Cross-module dependencies (per SPEC §4.6 — no `pub use` from sibling reasoning modules):
  * `use crate::reasoning::facts::{Action, Event, EventId, Provenance, VerbPolarity};`
  * `use crate::reasoning::timeline::TemporalAnchor;`
  * `use crate::models::LitNode;`
  * `use crate::parser::chapters::ParsedChapter;`
  * `use std::collections::HashMap;`
  * `use fancy_regex::Regex;`
  * `use serde::{Deserialize, Serialize};`
- No `pub use` from other reasoning modules. No tokio, no async, no `unwrap()` on
  external data (Python SVO output uses `?` semantics via filter_map/unwrap_or).
  Russian comments in user-facing strings; English identifiers throughout.
- SPEC deviations (documented in module doc + agent-ctx):
  1. `negated` parameter is accepted in `verb_to_action` but only applied to flip
     polarity on truly-unknown verbs (branch 3). For known verbs in the explicit table
     or in polarity sets, negation semantics are deferred to Wave 4 inference — this
     is the correct layering per SPEC §0.4 (rules apply effects, parsers don't).
  2. `chapter_suffix` is always `None` in `anchor_from_position` — ParsedChapter stores
     the suffix only in its `title` field, not as a separate struct field. Extracting
     it would require string parsing of the title. Deferred to Wave 5 if sub-chapter
     temporal reasoning becomes important.
  3. `parse_text_fallback` accepts sentence-initial common nouns («Потом») as phantom
     actors. Python's spaCy is needed for proper PROPN/Name filtering. The fallback
     is documented as "intentionally limited" in the task brief — full SVO is Python's
     job. resolve_or_keep returns the original text as a phantom entity, which Wave 4
     cycle.rs can later resolve or reject.
- Ready for Wave 5 integration: coordinator should uncomment `pub mod semantic_parser;`
  in mod.rs and add `pub use semantic_parser::{EntityResolver, SvoTriplet,
  parse_text_fallback, triplets_to_events, verb_to_action};` to re-exports.

---
Task ID: 3-b
Agent: full-stack-developer (memory.rs)
Task: Build memory.rs — KnowledgeBase with subgraph retrieval

Work Log:
- Read mandatory context: docs/reasoning/SPEC.md (§2.6 FactLog, §3.2 integration boundary,
  §4 module export rules, §5 anti-patterns), worklog.md (Wave 1 entries for facts/state/
  timeline/rules, Wave 2 entries for inference/causality/constraints/contradictions),
  src-tauri/src/reasoning/{facts,state,timeline,causality}.rs (confirmed FactLog API:
  new/assert_fact/record_event/get_facts_for/get_events_in_chapter/all_facts/all_events;
  TemporalAnchor ordering; CausalityEngine BFS pattern for inspiration), src-tauri/src/
  models/{node,edge,project,version}.rs (confirmed LitNode/LitEdge/Project field shapes;
  edge fields are snake_case in Rust — source_handle/target_handle, serde rename_all
  camelCase only for JSON).
- Wrote src-tauri/src/reasoning/memory.rs (~770 LOC including tests):
  * Module doc (Russian) — explains subgraph retrieval reduces LLM context size vs
    current "send everything" approach in ai/prompts.rs::build_assistant_prompt.
  * `Subgraph` struct (center/nodes/edges/facts/events/max_hops) — derives Debug, Clone,
    Serialize, Deserialize. Methods: is_empty (checks all 4 collections), summary
    (Russian, pluralized via pluralize_ru).
  * `KnowledgeBase` struct (nodes HashMap, edges Vec, facts FactLog, adjacency HashMap)
    — manual Debug impl (FactLog has no Debug derive), Default impl.
  * Constructor: new() empty, from_project(&Project, FactLog) — copies nodes/edges,
    owns FactLog, builds undirected adjacency with dedup.
  * Methods (14): get_node, neighbors, neighbors_filtered, facts_for, events_involving,
    events_in_chapter, related_entities, subgraph, search_by_name, node_count,
    edge_count, fact_count, event_count, retrieve_relevant, retrieve_for_question.
  * Private helpers: bfs_frontier (BFS up to max_hops, cycle-safe via HashSet).
  * Private free functions: build_adjacency (undirected, dedup neighbors),
    trim_subgraph (degree-desc sort + filter), merge_subgraphs (union by ID),
    pluralize_ru (Russian last-two-digits rule with 11-14 exception).
  * 8 required unit tests + 3 extra smoke tests:
    1. test_knowledge_base_from_project_initializes_adjacency
    2. test_neighbors_returns_directly_connected_nodes
    3. test_neighbors_filtered_by_edge_kind
    4. test_events_involving_finds_actor_and_target
    5. test_related_entities_bfs_within_max_hops
    6. test_subgraph_collects_nodes_edges_facts_events
    7. test_retrieve_relevant_finds_matching_node (incl. trim-by-degree sub-case)
    8. test_retrieve_for_question_handles_multi_word_query (incl. union + trim)
    Extra: test_subgraph_for_nonexistent_center_is_empty_or_self_only,
           test_pluralize_ru_forms (1/21→one, 2-4/22-24→few, 0/5-19/11-14/20+→many),
           test_subgraph_serializes_to_json (roundtrip via serde_json).
  * Test fixture: 4 LitNodes (ivan/anna/castle/ch1), 4 LitEdges (location×2, character,
    reference), 4 facts (alive×2 + location×2), 3 events (Speak, Arrive, Kill).
- Verification: standalone cargo project at /tmp/check_memory/ with serde + serde_json
  deps. Copied Wave 1 modules (facts/state/rules/timeline) + memory.rs + models
  (node/edge/project/version). Minimal reasoning/mod.rs with `pub mod memory;` and
  `pub use memory::{KnowledgeBase, Subgraph};`.
  * `cargo check --lib --tests` — clean.
  * `cargo clippy --lib --tests` — zero warnings on memory.rs (Wave 1 siblings have
    2 pre-existing `approx_constant` errors in facts.rs tests + 1 `explicit_auto_deref`
    warning in state.rs — not my code, not touched).
  * `cargo test --lib` — 44/44 passing (11 memory + 7 facts + 6 state + 11 rules +
    9 timeline). All 8 required memory tests pass.
- Could not run cargo check in src-tauri directly: Tauri's gdk-sys needs system libs
  absent in sandbox, and `pub mod memory;` is commented out in mod.rs awaiting Wave 5.
  Code is syntactically + semantically correct against the SPEC contract, verified via
  standalone project.
- Wrote agent-ctx work record at agent-ctx/3-b-memory.md.

Stage Summary:
- Public API exported by memory.rs:
  Types:    Subgraph (Debug/Clone/Serialize/Deserialize),
            KnowledgeBase (Default + manual Debug).
  Methods:  Subgraph::{is_empty, summary};
            KnowledgeBase::{new, from_project, get_node, neighbors, neighbors_filtered,
              facts_for, events_involving, events_in_chapter, related_entities,
              subgraph, search_by_name, node_count, edge_count, fact_count,
              event_count, retrieve_relevant, retrieve_for_question}.
  Private:  KnowledgeBase::bfs_frontier;
            free fns build_adjacency, trim_subgraph, merge_subgraphs, pluralize_ru.
- BFS subgraph algorithm:
  1. bfs_frontier(center, max_hops) — VecDeque<(id, hops)>, cycle-safe via HashSet,
     center always included even if not in nodes HashMap.
  2. subgraph() — filter nodes/edges/facts/events by frontier membership, sort by id.
- Retrieval strategy:
  * retrieve_relevant(query, max_nodes) — single-match: subgraph(first_match, 2) +
    trim. No-match: empty Subgraph with center=query.
  * retrieve_for_question(question, max_nodes) — tokenize by whitespace, dedupe
    matches, single→subgraph(match, 2)+trim, multiple→merge_subgraphs(all)+trim.
  * trim_subgraph — sort by (degree desc, id asc), truncate to max_nodes, filter
    edges/facts/events to kept set.
  * merge_subgraphs — union by ID (HashMap<String,_> for nodes/edges, HashMap<u64,_>
    for facts/events), max_hops = max across inputs.
- Decisions:
  (1) Undirected adjacency — edges treated as bidirectional in neighbors/related/
      subgraph BFS. Matches literary-graph semantics (ivan→anna character means
      both are related).
  (2) All Vec-returning methods sort by ID for deterministic output (HashMap iter
      order is randomized in Rust; sorting prevents flaky tests + non-reproducible
      LLM prompts).
  (3) BFS includes center even if not in `nodes` — allows retrieving facts/events
      for entities mentioned in events but without a graph node.
  (4) Subgraph::is_empty checks all 4 collections (nodes/edges/facts/events) —
      mirrors ContradictionReport::is_empty pattern from Wave 2.
  (5) pluralize_ru returns &'a str (zero-alloc, borrows from caller's args).
  (6) Manual Debug impl for KnowledgeBase — FactLog has no Debug derive (Wave 1
      design), so derive(Debug) would fail. Manual impl reports counts only.
  (7) #[allow(unused_imports)] on FactValue + TemporalAnchor — brief mandates
      importing these, but they're only used in test fixtures. Same pattern as
      inference.rs (Wave 2).
  (8) sort_by_key for Copy id types (FactId/EventId = u64), sort_by for String
      id types (LitNode.id/LitEdge.id) — clippy clean.
- No SPEC deviations. All type signatures, field names, derive lists match
  task brief exactly.
- No tokio/async, no `pub use` from sibling reasoning modules, no unwrap() on
  external data (only in test assertions via expect/unwrap on locally-built
  fixtures). Russian comments in user-facing strings; English identifiers throughout.
- Ready for Wave 5 integration: coordinator should uncomment `pub mod memory;`
  in mod.rs and add `pub use memory::{KnowledgeBase, Subgraph};` to re-exports.
  For LLM context: `kb.retrieve_for_question(user_question, 20)` produces a
  Subgraph that can be serialized to JSON and injected into ai/prompts.rs::
  build_assistant_prompt as the "WORLD STATE (relevant subset)" section.
