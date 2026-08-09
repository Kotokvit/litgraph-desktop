# Task 2-c — constraints.rs (Wave 2 / Logic Layer)

## Agent
full-stack-developer (constraints.rs)

## File written
`/home/z/my-project/litgraph-desktop/src-tauri/src/reasoning/constraints.rs`

## Public types/APIs exported

| Type | Kind | Notes |
|------|------|-------|
| `ConstraintCondition` | struct + impl | Fields: `attribute: Attribute`, `equals: FactValue`. Method: `is_met_by(&WorldState, &str) -> bool`. Derives `Debug, Clone`. |
| `Constraint` | struct | Fields: `name: &'static str`, `when: ConstraintCondition`, `forbids: Action`, `reason: String`. Derives `Debug, Clone`. |
| `ConstraintViolation` | struct | Fields: `constraint_name: String`, `event_id: EventId`, `actor: String`, `attempted_action: Action`, `reason: String`, `conflicting_fact: Option<FactId>`, `at: TemporalAnchor`. Derives `Debug, Clone, Serialize, Deserialize`. |
| `ConstraintEngine` | struct + impl + Default | Private field `constraints: Vec<Constraint>`. Methods: `new()`, `default_literary()`, `add(c)`, `check(state, event) -> Vec<ConstraintViolation>`, `check_all(state, events) -> Vec<...>`, `len()`, `is_empty()`. `Default::default()` delegates to `default_literary()`. |

Helper (private, not pub): `action_forbidden(forbids: &Action, attempted: &Action) -> bool`.

## default_literary() constraint count
**16 constraints** (>= 9 as required):

| # | name | when | forbids | reason |
|---|------|------|---------|--------|
| 1 | `dead_cannot_speak` | `alive = Bool(false)` | `Speak { topic: None }` | "Невозможно: персонаж мёртв, но пытается говорить" |
| 2 | `dead_cannot_move` | `alive = Bool(false)` | `Move { destination: "" }` | "Невозможно: персонаж мёртв, но перемещается" |
| 3 | `dead_cannot_act_physically_hit` | `alive = Bool(false)` | `Hit` | "Невозможно: мёртвый персонаж не может физически действовать" |
| 4 | `dead_cannot_act_physically_kill` | `alive = Bool(false)` | `Kill` | (same) |
| 5 | `dead_cannot_act_physically_wound` | `alive = Bool(false)` | `Wound` | (same) |
| 6 | `dead_cannot_act_physically_capture` | `alive = Bool(false)` | `Capture` | (same) |
| 7 | `dead_cannot_act_physically_imprison` | `alive = Bool(false)` | `Imprison` | (same) |
| 8 | `dead_cannot_act_physically_free` | `alive = Bool(false)` | `Free` | (same) |
| 9 | `dead_cannot_act_physically_heal` | `alive = Bool(false)` | `Heal` | (same) |
| 10 | `dead_cannot_act_physically_touch` | `alive = Bool(false)` | `Touch` | (same) |
| 11 | `imprisoned_cannot_move` | `imprisoned = Bool(true)` | `Move { destination: "" }` | "Невозможно: персонаж в заточении, не может переместиться" |
| 12 | `imprisoned_cannot_speak_freely` | `imprisoned = Bool(true)` | `Tell { topic: "", to: "" }` | "Невозможно: заключённый не может свободно рассказывать" |
| 13 | `captured_cannot_betray` | `captured = Bool(true)` | `Betray { victim: "" }` | "Пленённый персонаж не может совершать предательства" |
| 14 | `dead_cannot_die_again` | `alive = Bool(false)` | `Die` | "Персонаж уже мёртв" |
| 15 | `dead_cannot_marry` | `alive = Bool(false)` | `Marry { partner: "" }` | "Мёртвый персонаж не может вступить в брак" |
| 16 | `dead_cannot_know_new_facts` | `alive = Bool(false)` | `Know { fact: "" }` | "Мёртвый персонаж не может узнавать новые факты" |

## action_forbidden logic (matching semantics)

```rust
fn action_forbidden(forbids: &Action, attempted: &Action) -> bool {
    use std::mem::discriminant;
    match (forbids, attempted) {
        (Action::Custom { polarity: p1, .. },
         Action::Custom { polarity: p2, .. }) => p1 == p2,
        (a, b) => discriminant(a) == discriminant(b),
    }
}
```

Semantics:
- `Action::Custom` is matched by **polarity only** (verb_lemma is wildcard).
  A `forbids: Custom{Positive, ..}` matches any `Custom{Positive, ..}` event.
- All other Action variants are matched by **discriminant** (payload is
  wildcard). A `forbids: Speak{topic:None}` matches any `Speak{topic:_}`
  event; a `forbids: Move{destination:""}` matches any
  `Move{destination:<anything>}`.
- Cross-variant pairs never match (`Hit` doesn't forbid `Kill`,
  `Move` doesn't forbid `Arrive`, `Custom` doesn't forbid `Hit`).

This matches the convention used by `rules.rs::action_matches` (Wave 1, Task 1-d).

## SPEC deviations (with justification)

1. **`dead_cannot_act_physically` constraint group (8 constraints, not 1)**:
   SPEC §2.9 specifies `Constraint.forbids: Action` (single Action per
   Constraint). To forbid all 8 physical actions (Hit/Kill/Wound/Capture/
   Imprison/Free/Heal/Touch) for dead characters, the brief offered two
   options: (a) change `forbids: Action` to `forbids: Vec<Action>` (SPEC
   deviation), or (b) write multiple constraints with the same `when`.
   **Chose option (b)**: 8 separate `Constraint` instances with identical
   `when` (alive=false) and different `forbids`, with unique names
   `dead_cannot_act_physically_<variant>`.
   - **Justification**: keeps SPEC §2.9 intact (no field type change), enables
     precise violation diagnostics (which physical action was attempted), and
     matches the SPEC-recommended approach in the brief.
   - **Impact on test count**: `default_literary()` has 16 constraints, well
     above the `>= 9` assertion in `test_default_literary_has_expected_constraints`.

2. **`Default` delegates to `default_literary()`**: SPEC §2.9 doesn't specify
   which constructor `Default::default()` should delegate to. Following the
   project precedent set by `rules.rs::RuleSet::default()` (Wave 1, Task 1-d),
   `ConstraintEngine::default()` returns the literary constraint set.
   - **Justification**: consistent with sibling module; users wanting an empty
     engine use `ConstraintEngine::new()` explicitly.

## Dependencies on other modules

- `crate::reasoning::facts::{Action, Event, EventId, FactId, FactValue}` — Wave 1 (Task 1-a). Verified all types and field signatures match.
- `crate::reasoning::state::{Attribute, WorldState}` — Wave 1 (Task 1-b). Verified `WorldState::get(&self, &str, &str) -> Option<&FactValue>` signature; `&self.attribute` (`&String`) coerces to `&str`.
- `crate::reasoning::timeline::TemporalAnchor` — Wave 1 (Task 1-c). Used only as a value carried through `Event::time` to `ConstraintViolation::at`.

## Verification

Standalone cargo project at `/home/z/constraints_check/`:
- Structure: `src/lib.rs` → `pub mod reasoning` → `src/reasoning/mod.rs` → `{timeline, facts, state, constraints}`.
- Copied Wave 1 modules (timeline.rs, facts.rs, state.rs) verbatim from the real project + new constraints.rs.

Test results:
```
$ cargo test --lib
running 31 tests
test reasoning::constraints::tests::test_action_forbidden_uses_discriminant ... ok
test reasoning::constraints::tests::test_action_forbidden_custom_matches_by_polarity ... ok
test reasoning::constraints::tests::test_check_all_returns_violations_for_multiple_events ... ok
test reasoning::constraints::tests::test_dead_character_cannot_move ... ok
test reasoning::constraints::tests::test_dead_character_cannot_speak ... ok
test reasoning::constraints::tests::test_alive_character_can_speak ... ok
test reasoning::constraints::tests::test_default_literary_has_expected_constraints ... ok
test reasoning::constraints::tests::test_imprisoned_character_cannot_move ... ok
[... 23 tests from Wave 1 modules all pass ...]

test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured
```

Clippy:
```
$ cargo clippy --lib -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.20s
```
Clean, zero warnings.

## 8 mandatory unit tests (all pass)

1. `test_dead_character_cannot_speak` — dead Petr speaks → exactly 1 violation (`dead_cannot_speak`), all fields populated correctly.
2. `test_alive_character_can_speak` — alive Ivan speaks → 0 violations.
3. `test_dead_character_cannot_move` — dead Petr moves → exactly 1 violation (`dead_cannot_move`), `destination: "Замок"` payload preserved in `attempted_action`.
4. `test_imprisoned_character_cannot_move` — imprisoned+alive Anna moves → exactly 1 violation (`imprisoned_cannot_move`), `dead_cannot_move` does NOT fire.
5. `test_action_forbidden_uses_discriminant` — Speak{None} matches Speak{Some(_)}, Move{""} matches Move{"Москва"}, Hit doesn't match Kill, Move doesn't match Arrive.
6. `test_action_forbidden_custom_matches_by_polarity` — Positive matches Positive (any verb_lemma), Positive doesn't match Negative, Neutral matches Neutral, Custom never matches non-Custom.
7. `test_check_all_returns_violations_for_multiple_events` — 4 events (Petr speaks, Anna moves, Ivan speaks, Petr dies) → violations for events 1, 2, 4; none for event 3; event 4 explicitly violates `dead_cannot_die_again`.
8. `test_default_literary_has_expected_constraints` — `len() >= 9` AND all 16 expected constraint names present by exact match.

## Worklog
Appended to `/home/z/my-project/litgraph-desktop/worklog.md` under section `Task ID: 2-c`.

## Status
✅ Complete. Ready for Wave 2 siblings (`inference.rs`, `causality.rs`, `contradictions.rs`) and Wave 4 consumer (`cycle.rs`).
