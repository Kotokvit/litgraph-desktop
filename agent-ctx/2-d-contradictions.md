# Task 2-d — contradictions.rs Work Record

**Agent:** full-stack-developer (contradictions.rs)
**Task ID:** 2-d
**Module:** `src-tauri/src/reasoning/contradictions.rs`
**Wave:** 2 (logic layer)

## Work Log

1. Read mandatory inputs:
   - `docs/reasoning/SPEC.md` §2.9 (Constraint), §2.10 (ContradictionReport, TemporalParadox, CausalLoop)
   - `worklog.md` (Tasks 0, 1-a, 1-b, 1-c, 1-d — Wave 1 complete)
   - `src-tauri/src/reasoning/facts.rs` — confirmed Fact/Event/FactLog/FactValue/Action/VerbPolarity/Provenance API
   - `src-tauri/src/reasoning/timeline.rs` — confirmed TemporalAnchor API (Ord, before/after, display_chapter)
   - `src-tauri/src/reasoning/mod.rs` — confirmed Wave 2 modules (`pub mod contradictions;`) are commented out, will be uncommented by Wave 5 coordinator
   - Listed `src-tauri/src/reasoning/` directory — confirmed `constraints.rs` and `causality.rs` do NOT yet exist (parallel Wave 2 agents in flight)

2. Wrote `src-tauri/src/reasoning/contradictions.rs` (~520 LOC including tests):
   - Module doc comment (Russian) explaining 3 contradiction categories, temporal paradox algorithm, and time-interval semantics `[valid_from, valid_until)`.
   - Sibling-module forward declarations:
     * Local `pub struct ConstraintViolation` (7 fields, per SPEC §2.9) — TEMPORARY, marked for replacement with `use crate::reasoning::constraints::ConstraintViolation;` once sibling lands.
     * Local `pub struct CausalLoop` (description, chain: Vec<EventId>, per SPEC §2.10) — TEMPORARY, marked for replacement with `use crate::reasoning::causality::CausalLoop;` once sibling lands.
   - Own types per SPEC §2.10:
     * `pub struct TemporalParadox` (5 fields: description, earlier_fact, later_event, earlier_at, later_at).
     * `pub struct ContradictionReport` (3 fields: violations, temporal_paradoxes, causal_loops) + `Default` derive.
     * Methods on `ContradictionReport`: `new`, `is_empty`, `total_count`, `summary` (Russian pluralization via `pluralize_ru`).
   - `pub struct ContradictionDetector` (stateless, `Debug, Clone, Default`):
     * `new`
     * `detect_temporal_paradoxes(&FactLog, &[Event]) -> Vec<TemporalParadox>`
     * `detect_all(constraint_violations, &FactLog, &[Event], causal_loops) -> ContradictionReport`
     * Private `check_resurrect_without_dying` helper (associated function).
   - Private helpers:
     * `action_requires_life(&Action) -> bool` — false for Die, Resurrect, Custom{Neutral}; true for everything else.
     * `pluralize_ru(n, one, few, many) -> &'static str` — Russian noun pluralization by last two digits.
   - 6 unit tests (all required by task brief):
     * test_detect_peter_dead_in_ch12_speaks_in_ch15
     * test_no_paradox_for_alive_character_speaking
     * test_detect_resurrect_without_dying (3 sub-cases: alive+resurrect=paradox, dead+resurrect=OK, no-fact+resurrect=paradox)
     * test_contradiction_report_summary (2 violations + 1 paradox → "Найдено 3 противоречия: 2 нарушения ограничений, 1 временной парадокс")
     * test_contradiction_report_is_empty
     * test_detect_all_combines_violations_and_paradoxes

3. Verification:
   - Set up standalone cargo project at `/tmp/check_contradictions/` with:
     * `Cargo.toml` (serde dep)
     * `src/lib.rs` → `pub mod reasoning;`
     * `src/reasoning/mod.rs` → declares `pub mod facts; pub mod timeline; pub mod contradictions;`
     * Copied `facts.rs`, `timeline.rs`, `contradictions.rs` from src-tauri
   - `cargo check --lib` — clean (after removing unused `Fact` import).
   - `cargo clippy --lib` — clean (after fixing `op_ref` warning: changed `&f.valid_from <= &event.time` to `f.valid_from <= event.time` — Rust's `<=` auto-refs operands, so references are redundant).
   - `cargo test --lib` — 23 tests pass (6 contradictions + 7 facts + 10 timeline). My 6 tests:
     ```
     test reasoning::contradictions::tests::test_contradiction_report_is_empty ... ok
     test reasoning::contradictions::tests::test_contradiction_report_summary ... ok
     test reasoning::contradictions::tests::test_detect_all_combines_violations_and_paradoxes ... ok
     test reasoning::contradictions::tests::test_detect_peter_dead_in_ch12_speaks_in_ch15 ... ok
     test reasoning::contradictions::tests::test_detect_resurrect_without_dying ... ok
     test reasoning::contradictions::tests::test_no_paradox_for_alive_character_speaking ... ok
     ```
   - Could not run `cargo check` in src-tauri directly (Tauri's gdk-sys needs system libs absent in sandbox, and `pub mod contradictions;` is commented out in mod.rs awaiting Wave 5). Verified correctness via standalone project mirroring SPEC contracts.

## Stage Summary

### Public API exported by contradictions.rs:

**Types:**
- `ConstraintViolation` (TEMPORARY local; will be replaced by import from constraints.rs)
- `CausalLoop` (TEMPORARY local; will be replaced by import from causality.rs)
- `TemporalParadox` (5 fields: description, earlier_fact, later_event, earlier_at, later_at)
- `ContradictionReport` (3 fields: violations, temporal_paradoxes, causal_loops; + Default)
- `ContradictionDetector` (stateless service; + Default)

**Methods:**
- `ContradictionReport::new`, `is_empty`, `total_count`, `summary`
- `ContradictionDetector::new`, `detect_temporal_paradoxes`, `detect_all`
- Private: `check_resurrect_without_dying`, `action_requires_life`, `pluralize_ru`

### Temporal paradox detection algorithm:

For each event in `events`:
1. If `action == Resurrect` → delegate to `check_resurrect_without_dying`:
   - Find latest active `alive` fact at event.time.
   - If that fact's value is `Bool(false)` → no paradox (legitimate resurrection).
   - Else (`Bool(true)` or no fact) → emit paradox with description "X воскресает в Y, но не был мёртв до этого".
2. Else if `action_requires_life(action)`:
   - Find any active `alive = Bool(false)` fact at event.time for `event.actor`.
   - If found → emit paradox with description "X мёртв с Y, но совершает действие {:?} в Z".
3. Else (Die, Custom{Neutral}) → skip.

"Active at time T" = `valid_from <= T` AND (`valid_until is None` OR `T < valid_until`) — standard semi-open interval semantics.

### Decisions made:

1. **Local definitions for ConstraintViolation & CausalLoop.** Sibling modules `constraints.rs` and `causality.rs` don't exist yet (confirmed via `ls`). Defined minimal local versions matching SPEC §2.9 and §2.10 field-by-field, with `// TEMPORARY local definition — replace with use ... once that module lands` comments. Coordinator (Wave 5) will swap to imports.

2. **No `pub use` from sibling modules.** Per SPEC §4.6 and task rules. All cross-module access via `use crate::reasoning::facts::{...}` and `use crate::reasoning::timeline::TemporalAnchor;`.

3. **`ContradictionDetector` derives only `Debug, Clone, Default`.** Not `Serialize, Deserialize` — it's a stateless service, no data to serialize. Matches pattern of `FactLog` and `WorldState` in Wave 1 (stateful services don't derive serde).

4. **`ContradictionReport` derives `Default`.** `new()` delegates to `default()` — cleaner than initializing empty Vecs manually.

5. **Resurrect-without-dying paradox: `earlier_fact` field handling.** When `alive = true` fact exists, point `earlier_fact` to it (it's the fact being contradicted). When no `alive` fact exists at all, use `FactId(0)` as sentinel (no Option<FactId> per SPEC). `earlier_at` mirrors this: live fact's `valid_from` if exists, else `event.time` (best available anchor).

6. **Russian pluralization via `pluralize_ru`.** Standard rule based on last two digits: 1/21/31→one, 2-4/22-24→few, 0/5-20/25-30→many, with exceptions for 11-14. Verified against task example: n=3 → "противоречия", n=2 → "нарушения ограничений", n=1 → "временной парадокс".

7. **`action_requires_life` criterion.** Per task brief §5 parenthetical: "any action that requires being alive — i.e. NOT `Die`, `Resurrect`, `Custom` with neutral polarity". Implemented as exhaustive match: `Die | Resurrect → false`, `Custom{Neutral} → false`, `Custom{Positive|Negative} → true`, all other Action variants → `true`. This means `Ask`, `Tell`, `Ally`, `Forget`, `Transform` (not in the task's example list of 20) are correctly treated as requiring life — consistent with the parenthetical rule.

8. **Time comparison via `<=` operator.** Rust's `PartialOrd` comparison operators auto-reference their operands, so `f.valid_from <= event.time` works even for non-Copy `TemporalAnchor`. Initial code used `&f.valid_from <= &event.time` (explicit references) — clippy flagged this as `op_ref`; fixed.

### SPEC deviations:

None. All type signatures, field names, and derive lists match SPEC §2.10 exactly. The `ConstraintViolation` and `CausalLoop` local definitions match SPEC §2.9 and §2.10 field-by-field (drop-in compatible).

### Dependencies:

- `use crate::reasoning::facts::{Action, Event, EventId, FactId, FactLog, FactValue, VerbPolarity};` — Wave 1 (ready).
- `use crate::reasoning::timeline::TemporalAnchor;` — Wave 1 (ready).
- Local definitions of `ConstraintViolation` and `CausalLoop` — to be replaced by sibling-module imports once those modules land.

No tokio, no async, no `unwrap()` on external data (only in test assertions on `Option::unwrap` for clarity). Russian comments in user-facing strings; English identifiers throughout.
