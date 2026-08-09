# Task 3-b — memory.rs (Wave 3 / Semantic Layer)

**Agent:** full-stack-developer
**Module:** `src-tauri/src/reasoning/memory.rs`
**Status:** ✅ Complete — 11/11 unit tests passing in standalone sandbox
(8 required + 3 extra smoke tests). Zero clippy warnings on memory.rs.

## What was built

The KnowledgeBase — long-term memory for the reasoning engine. Stores
the project graph (`LitNode`/`LitEdge`) plus an owned `FactLog`, and
provides subgraph retrieval API so LLM gets only relevant context
(replacing the current "send everything" approach in `ai/prompts.rs`).

## Public API

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subgraph {
    pub center: String,
    pub nodes: Vec<LitNode>,
    pub edges: Vec<LitEdge>,
    pub facts: Vec<Fact>,
    pub events: Vec<Event>,
    pub max_hops: usize,
}

impl Subgraph {
    pub fn is_empty(&self) -> bool;
    pub fn summary(&self) -> String;  // Russian, pluralized
}

pub struct KnowledgeBase { /* private fields */ }

impl KnowledgeBase {
    pub fn new() -> Self;
    pub fn from_project(project: &Project, facts: FactLog) -> Self;
    pub fn get_node(&self, id: &str) -> Option<&LitNode>;
    pub fn neighbors(&self, id: &str) -> Vec<&LitNode>;
    pub fn neighbors_filtered(&self, id: &str, edge_kind: &str) -> Vec<&LitNode>;
    pub fn facts_for(&self, entity: &str) -> Vec<&Fact>;
    pub fn events_involving(&self, entity: &str) -> Vec<&Event>;
    pub fn events_in_chapter(&self, chapter: u32) -> Vec<&Event>;
    pub fn related_entities(&self, entity: &str, max_hops: usize) -> Vec<String>;
    pub fn subgraph(&self, center: &str, max_hops: usize) -> Subgraph;
    pub fn search_by_name(&self, query: &str) -> Vec<&LitNode>;
    pub fn node_count(&self) -> usize;
    pub fn edge_count(&self) -> usize;
    pub fn fact_count(&self) -> usize;
    pub fn event_count(&self) -> usize;
    pub fn retrieve_relevant(&self, query: &str, max_nodes: usize) -> Subgraph;
    pub fn retrieve_for_question(&self, question: &str, max_nodes: usize) -> Subgraph;
}

impl Default for KnowledgeBase;
impl std::fmt::Debug for KnowledgeBase;  // manual, FactLog not Debug
```

## Algorithms

### BFS frontier (private `bfs_frontier`)
- Start: `{center}` always (even if not in `nodes` — allows retrieving
  facts/events for IDs without a graph node).
- Queue: `VecDeque<(String, usize)>` — node ID + hop count.
- Stop expanding when `hops >= max_hops`.
- Cycle-safe via `visited: HashSet<String>`.

### `subgraph(center, max_hops)`
1. BFS → frontier (HashSet<String>).
2. Nodes: frontier ∩ self.nodes, sorted by id.
3. Edges: both source and target in frontier, sorted by id.
4. Facts: entity in frontier, sorted by id.
5. Events: actor in frontier OR target in frontier, sorted by id.

### `retrieve_relevant(query, max_nodes)`
1. `search_by_name(query)` — case-insensitive substring on title.
2. Empty → return Subgraph{center=query, empty, max_hops=0}.
3. Else take first match (sorted by id for determinism), call
   `subgraph(center, 2)`.
4. If nodes.len() > max_nodes → `trim_subgraph`.

### `retrieve_for_question(question, max_nodes)`
1. Tokenize by whitespace.
2. For each token: search_by_name → collect match IDs (deduped).
3. Empty → empty Subgraph.
4. Single match → subgraph(match, 2) + trim.
5. Multiple matches → for each: subgraph(match, 2) → `merge_subgraphs`
   (union by ID for nodes/edges/facts/events) + trim.

### `trim_subgraph(sg, max_nodes)` (private free fn)
1. If nodes.len() <= max_nodes → return as-is.
2. Compute degree per node from subgraph's edges.
3. Sort by (degree desc, id asc) — stable.
4. Truncate to max_nodes.
5. Filter edges: both endpoints in kept set.
6. Filter facts: entity in kept set.
7. Filter events: actor OR target in kept set.

### `merge_subgraphs(subgraphs, center)` (private free fn)
- Union by ID via HashMap<String, _> for nodes/edges; HashMap<u64, _>
  for facts/events (id is u64 FactId/EventId).
- max_hops = max across inputs.
- Output sorted by id for determinism.

### `pluralize_ru(n, one, few, many)` (private free fn)
- Last-two-digits rule with 11-14 exception.
- Returns `&'a str` (borrows from caller's args).
- Verified: 1→"прыжок", 2-4→"прыжка", 0/5-19/20+→"прыжков".

## Test fixture

4 LitNodes (ivan:Character, anna:Character, castle:Location, ch1:Chapter),
4 LitEdges (e1 ivan→castle location, e2 anna→castle location,
e3 ivan→anna character, e4 ch1→ivan reference), 4 facts (alive×2 +
location×2), 3 events (ivan Speak→anna ch1, anna Arrive ch1,
ivan Kill→anna ch2).

## Verification

- Standalone cargo project at `/tmp/check_memory/` with serde + serde_json deps.
- Copied Wave 1 modules (facts/state/rules/timeline) + memory.rs +
  models (node/edge/project/version).
- `cargo check --lib --tests` — clean.
- `cargo clippy --lib --tests` — zero warnings on memory.rs (Wave 1
  siblings have 2 pre-existing `approx_constant` errors in facts.rs
  tests + 1 `explicit_auto_deref` warning in state.rs — not my code).
- `cargo test --lib` — 44/44 passing (11 memory + 7 facts + 6 state +
  11 rules + 9 timeline). All 8 required memory tests pass + 3 extra
  smoke tests (pluralize_ru forms, nonexistent center, JSON roundtrip).

## SPEC deviations

None. All public types and method signatures match the task brief
exactly. KnowledgeBase derives only `Default` (FactLog doesn't derive
Clone/Serialize/Deserialize, so KnowledgeBase can't either — manual
`Debug` impl added for diagnostics).

## Decisions

1. **Undirected adjacency**: edges treated as bidirectional in
   `neighbors`, `neighbors_filtered`, `related_entities`, `bfs_frontier`.
   This matches the literary-graph semantics where `ivan → anna` (character)
   means both are related.
2. **Deterministic ordering**: all `Vec`-returning methods sort by ID
   alphabetically. HashMap iteration order is randomized in Rust, so
   without sorting, tests would be flaky and LLM prompts non-reproducible.
3. **BFS includes center even if not in `nodes`**: allows retrieving
   facts/events for entities mentioned in events but without a graph
   node (orphan entities). Edge case but useful.
4. **`Subgraph::is_empty` checks all 4 collections**: stricter than
   just `nodes.is_empty()`. Mirrors `ContradictionReport::is_empty`
   pattern from Wave 2.
5. **`pluralize_ru` returns `&'a str`**: zero-allocation, borrows from
   caller's `&'a str` arguments. Cleaner than returning `String`.
6. **`merge_subgraphs` uses `HashMap<u64, _>` for facts/events**: id
   fields are `FactId`/`EventId` (u64 aliases). Cheaper than String keys.
7. **Manual `Debug` for KnowledgeBase**: FactLog has no Debug impl
   (Wave 1 design). Manual impl reports counts only, not internal state.
8. **`#[allow(unused_imports)]` on FactValue + TemporalAnchor**: brief
   mandates importing these, but they're only used in test fixtures.
   Same pattern as `inference.rs` (Wave 2).

## Integration notes (for Wave 5 coordinator)

- Uncomment `pub mod memory;` in `src-tauri/src/reasoning/mod.rs`.
- Add `pub use memory::{KnowledgeBase, Subgraph};` to re-exports.
- `KnowledgeBase` is constructed once per project lifecycle (after
  parse_md_full + SVO extraction populates FactLog). The resulting KB
  is read-only — mutations go through WorldState + FactLog, then a new
  KB is built if needed.
- For LLM context: `kb.retrieve_for_question(user_question, 20)`
  produces a Subgraph that can be serialized to JSON and injected
  into `ai/prompts.rs::build_assistant_prompt` as the "WORLD STATE
  (relevant subset)" section (replacing the current "send everything"
  approach).
