# Task 1-b — state.rs (WorldState)

**Agent:** full-stack-developer (state.rs)
**Wave:** 1 (Data Layer)
**File:** `/home/z/my-project/litgraph-desktop/src-tauri/src/reasoning/state.rs`
**Status:** ✅ Complete — compiles + 6/6 tests pass (verified via stub-facts/timeline in /tmp/check_proj)

## Public API Exported

```rust
pub type EntityId = String;
pub type Attribute = String;

pub struct StateTransition {
    pub entity: EntityId,
    pub attribute: Attribute,
    pub old_value: Option<FactValue>,
    pub new_value: FactValue,
    pub caused_by_event: Option<EventId>,
    pub at: TemporalAnchor,
}

pub struct WorldSnapshot {
    pub current: HashMap<EntityId, HashMap<Attribute, FactValue>>,
    pub now: TemporalAnchor,
}

pub struct WorldState { /* private fields */ }

impl WorldState {
    pub fn new() -> Self;
    pub fn get(&self, entity: &str, attr: &str) -> Option<&FactValue>;
    pub fn set(&mut self, entity: &str, attr: Attribute, value: FactValue, transition: StateTransition);
    pub fn has_attribute(&self, entity: &str, attr: &str) -> bool;
    pub fn entities_with(&self, attr: &str, value: &FactValue) -> Vec<EntityId>;
    pub fn advance_to(&mut self, anchor: &TemporalAnchor);   // panics on backward
    pub fn now(&self) -> &TemporalAnchor;
    pub fn history(&self) -> &[StateTransition];
    pub fn snapshot(&self) -> WorldSnapshot;
    pub fn restore(&mut self, snap: WorldSnapshot);
    pub fn invalidate(&mut self, entity: &str, attr: &str, caused_by_event: Option<EventId>);
}

impl Default for WorldState;
```

## Dependencies consumed (NOT defined here)

- `crate::reasoning::facts::{FactValue, EventId}` — per SPEC §2.5 / §2.1.
- `crate::reasoning::timeline::TemporalAnchor` — per SPEC §2.2.

## Key design decisions

1. **`&str` for entity/attr parameters instead of `&EntityId`/`&Attribute`** —
   SPEC §2.7 uses `&EntityId`/`&Attribute`, but task brief specified `&str`.
   Functionally compatible via deref coercion; `&str` is more ergonomic at
   call sites (`state.get("Petr", "alive")` works without `.as_str()`).

2. **Private `fact_value_eq` helper** — SPEC §2.5 does NOT derive `PartialEq`
   for `FactValue`, but `entities_with` needs equality. Implemented as a
   private free function (recursive for `List`, NaN-aware for `Float`).
   If `facts.rs` later adds `PartialEq`, this helper remains correct (just
   redundant).

3. **`set` trusts caller-provided transition** — the function does NOT
   re-derive `old_value` from current state. The transition is the audit
   record; rules/inference are responsible for consistency. Doc comment
   explains the invariants.

4. **`restore` appends synthetic `__restore__` transition** — preserves
   prior history (audit trail) while marking the rollback point. Uses
   empty `entity` + `attribute = "__restore__"` + Russian message in
   `new_value` (`FactValue::Str`).

5. **`advance_to` panics on backward** — per task brief. Panic message is
   in Russian, includes both `now` and the offending `anchor` for debug.

6. **`has_attribute` returns true for `Unknown` values** — `invalidate`
   sets value to `Unknown` but the attribute is still "set". `get` returns
   `Some(&Unknown)` in this case. Tests verify this distinction.

## Tests (all passing)

- `test_set_and_get_attribute` ✓
- `test_history_records_transitions` ✓
- `test_invalidate_sets_unknown` ✓
- `test_snapshot_and_restore` ✓
- `test_advance_to_updates_now` ✓
- `test_entities_with_finds_matching` ✓ (also covers Unknown-matching after invalidate)

## SPEC deviations

- `get`/`set`/`has_attribute`/`entities_with`/`invalidate` use `&str` for
  entity/attr instead of `&EntityId`/`&Attribute`. Justified by task brief
  (explicit signatures) and ergonomic improvement (deref-compatible).
- `WorldState` derives `Debug, Clone, Serialize, Deserialize` (SPEC §2.7
  shows no derives for `WorldState`, but task brief says "all public types"
  must have these). All field types support these traits.
