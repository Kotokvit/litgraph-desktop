# Task 2-b — causality.rs (Wave 2 / Logic Layer)

**Agent:** full-stack-developer
**Module:** `src-tauri/src/reasoning/causality.rs`
**Status:** ✅ Complete — 13/13 unit tests passing in sandbox

## What was built

The causal propagation engine for LitGraph's reasoning layer. Converts
`cause`-kind edges from the LitGraph graph into typed `CausalLink`s (mapping
node IDs → EventIds via FactLog), then provides:

- Direct cause/effect queries (O(L) scan)
- Transitive closure upstream/downstream (recursive DFS with cycle guard)
- Cycle detection (DFS with `visiting`/`visited` marker sets)
- Shortest-path finding (BFS with parent pointers, max 20 hops)

## Public API

```rust
pub struct CausalLink {
    pub cause_event_id: EventId,
    pub effect_event_id: EventId,
    pub description: String,
}

pub struct CausalLoop {
    pub chain: Vec<EventId>,        // A→B→C→A stored as [A, B, C, A]
    pub description: String,
}

pub struct CausalityEngine { /* private links: Vec<CausalLink> */ }

impl CausalityEngine {
    pub fn new() -> Self;
    pub fn from_edges(edges: &[LitEdge], facts: &FactLog) -> Self;
    pub fn add_link(&mut self, link: CausalLink);
    pub fn links(&self) -> &[CausalLink];
    pub fn direct_causes_of(&self, event_id: EventId) -> Vec<&CausalLink>;
    pub fn direct_effects_of(&self, event_id: EventId) -> Vec<&CausalLink>;
    pub fn transitive_causes(&self, event_id: EventId) -> Vec<EventId>;
    pub fn transitive_effects(&self, event_id: EventId) -> Vec<EventId>;
    pub fn detect_causal_loops(&self) -> Vec<CausalLoop>;
    pub fn explain_chain(&self, from: EventId, to: EventId) -> Option<Vec<EventId>>;
}

impl Default for CausalityEngine;
```

`CausalLink` and `CausalLoop` derive `Debug, Clone, Serialize, Deserialize`.
`CausalityEngine` derives `Debug, Clone` (no serde — internal container).

## Algorithm notes

- **Cycle detection** uses dual marker sets: `visiting` for current DFS path
  (back-edge ⇒ cycle), `visited` for fully-processed nodes (skip on re-entry).
  Nodes are sorted before iteration so output is deterministic.
- **Transitive walks** seed `visited` with the query node itself, so cycles
  can't bring the start node back into its own result set.
- **explain_chain** uses BFS over directed edges (cause→effect direction).
  Depth cap = 20 hops; returns `None` on `from == to`, no-path, or >MAX_HOPS.
- **from_edges** maps source/target node IDs to EventIds by scanning
  `facts.all_events()` and picking the event with the earliest `time`
  whose `actor` or `target` matches the node.

## Verification

- Created `/home/z/check_causality` sandbox cargo project mirroring SPEC
  contracts — copied Wave 1 `facts.rs` + `timeline.rs` from real project,
  stubbed `models::edge::LitEdge` matching real `EdgeData` struct.
- `cargo check --lib` — clean.
- `cargo test --lib` — 30/30 passing (13 causality + 7 facts + 10 timeline).
- `cargo clippy --lib` — clean for causality.rs. (Pre-existing
  `approx_constant` clippy lint in facts.rs Float(3.14) test is out of scope.)
- Could not run `cargo check` in src-tauri directly because `pub mod causality;`
  is still commented out in mod.rs (Wave 2 — coordinator will flip in Wave 5)
  and because Tauri's gdk-sys build needs system libs not present in sandbox.

## Decisions worth flagging

1. **Cycle chain is closed**: A→B→C→A is stored as `[A, B, C, A]` (last ==
   first). Matches SPEC §2.10's example notation.
2. **transitive_causes(A) excludes A itself**, even when A is in a cycle.
   For A→B→A, result = `[B]`, not `[A, B]`.
3. **`explain_chain` rejects `from == to`** per brief — returns `None`.
4. **Self-loops allowed** in CausalLink and detected as cycles of length 1
   (chain `[X, X]`). Covered by `test_self_loop_is_detected`.
5. **`from_edges` silently skips** edges whose source/target has no matching
   event in FactLog (cause edges may reference non-event nodes).
6. **mod.rs NOT touched** per task constraint. `pub mod causality;` remains
   commented out under Wave 2 — coordinator flips it during Wave 5 integration.

## Files touched

- `src-tauri/src/reasoning/causality.rs` — created (~730 LOC with tests).
- `worklog.md` — appended Task 2-b section.
- This file (`agent-ctx/2-b-causality.md`).

## Downstream consumers

- `contradictions.rs` (Wave 2) — will `use crate::reasoning::causality::CausalLoop`
  to populate `ContradictionReport.causal_loops`. The struct is already
  Serialize/Deserialize-ready.
- `cycle.rs` (Wave 4) — will call `detect_causal_loops()` during `reason()`
  step to populate the CycleReport.
- `hypotheses.rs` (Wave 4) — may use `explain_chain()` to generate natural-
  language explanations of why an event implies a state change.
