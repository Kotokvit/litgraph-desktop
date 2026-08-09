# Task 3-a — semantic_parser.rs (Wave 3 / Semantic Layer)

Agent: full-stack-developer (semantic_parser.rs)
File: `src-tauri/src/reasoning/semantic_parser.rs` (~1180 LOC including tests)
Date: 2024 build

## What was built

The **semantic compiler** for the Reasoning Engine: converts Python SVO triplets
(JSON) and raw Russian text into typed `Event` structs that the inference engine
can consume. Two modes:

1. **Primary (Python SVO)** — `triplets_to_events(&[SvoTriplet], &EntityResolver,
   &[ParsedChapter]) -> Vec<Event>`. High-confidence (0.9), `Provenance::SvoParser`.
2. **Fallback (Rust regex)** — `parse_text_fallback(&str, &EntityResolver,
   &[ParsedChapter]) -> Vec<Event>`. Low-confidence (0.5), `Provenance::RustParser`.
   Used when Python is unavailable (no spaCy / pymorphy3 / interpreter).

## Public API

### Types
- `pub struct SvoTriplet` — mirrors Python JSON shape, `#[serde(rename = "...")]`
  for camelCase round-trip. All optional fields marked `#[serde(default)]`.
  Derives `Debug, Clone, Serialize, Deserialize, PartialEq`.
- `pub struct EntityResolver` — `by_lemma` + `by_alias` HashMaps.
  Derives `Debug, Clone, Default`.

### Functions
- `pub fn verb_to_action(verb_lemma: &str, polarity: &str, negated: bool) -> Action`
  — 4-level fallback (explicit table → polarity set → polarity field → Neutral).
- `pub fn triplets_to_events(triplets, resolver, chapters) -> Vec<Event>`
- `pub fn parse_text_fallback(text, resolver, chapters) -> Vec<Event>`

### EntityResolver methods
- `from_nodes(&[LitNode]) -> Self`
- `resolve(&str) -> Option<String>` — exact lowercase match, no fuzzy.
- `resolve_or_keep(&str) -> String` — fallback to original name (phantom entity).
- `lemma_count() -> usize` / `alias_count() -> usize` — diagnostics.

## Verb lemma mapping

- **54 unique Russian verb lemmas** in the explicit match table → 24 distinct
  `Action` variants. The task brief listed `"выйти"` twice in the Leave row
  (a typo) — deduped to a single entry.
- **165 lemmas across 3 polarity sets** (47 POSITIVE + 60 NEGATIVE + 58 NEUTRAL)
  used for the second-level fallback. Mirrored from `svo_extract.py` lines 99-138.

## Tests

All 9 unit tests required by brief pass (verified in standalone cargo project
at `/home/z/check_semantic_parser`):

```
test reasoning::semantic_parser::tests::test_entity_resolver_finds_by_title ... ok
test reasoning::semantic_parser::tests::test_entity_resolver_finds_by_alias ... ok
test reasoning::semantic_parser::tests::test_entity_resolver_returns_none_for_unknown ... ok
test reasoning::semantic_parser::tests::test_triplets_to_events_assigns_temporal_anchor ... ok
test reasoning::semantic_parser::tests::test_triplets_to_events_resolves_actor_and_target ... ok
test reasoning::semantic_parser::tests::test_verb_to_action_kill ... ok
test reasoning::semantic_parser::tests::test_verb_to_action_speak ... ok
test reasoning::semantic_parser::tests::test_verb_to_action_unknown_verb_uses_polarity ... ok
test reasoning::semantic_parser::tests::test_parse_text_fallback_extracts_kill_event ... ok
```

Plus 17 sibling tests from Wave 1 (facts + timeline) all pass = 26/26 total.

`cargo clippy --lib --tests`: 0 warnings/errors in `semantic_parser.rs`
(pre-existing warnings in chapters.rs `map_identity` and errors in facts.rs
`approx_constant` are in sibling modules, out of scope).

## Key decisions

1. **EntityResolver two HashMaps** (by_lemma + by_alias). Lemma wins over alias
   when ambiguous — canonical names override nicknames.
2. **Only `character` + `organization` nodes indexed** — prevents locations
   like "Лес" from accidentally becoming Kill's target.
3. **target_for_action**: Some only for Kill/Wound/Hit/Capture/Imprison/Free/
   Heal/Touch. Marry/Betray/Ally/FallInLove/Hate carry EntityId inside the
   action variant — populating Event.target would duplicate.
4. **Know/Forget use `triplet.sentence` as `fact`** (full sentence as
   approximation per task brief). Wave 4 cycle.rs can refine with
   propositional extraction.
5. **chapter_suffix=None in anchor_from_position** — ParsedChapter stores
   suffix only in `title` field, not as separate field. Extracting would
   require string parsing. Deferred to Wave 5.
6. **`\b` Unicode word boundary** in fallback regexes — fancy-regex supports
   this by default. Correctly matches «убил» as whole word, rejects «убилство».
7. **`negated` parameter**: only applied to flip Positive↔Negative on truly
   unknown verbs (branch 3). For known verbs, semantic negation deferred to
   Wave 4 inference (rules.rs + cycle.rs) per SPEC §0.4 (rules apply effects,
   parsers don't).
8. **Regex compilation uses `.expect(...)`** — all patterns are static
   literals, failure indicates fancy-regex bug. Matches chapters.rs precedent.
   Not a SPEC §5.4 violation (that rule is about `unwrap()` on external data,
   not internal static regexes).

## SPEC deviations

1. `negated` accepted but only applied to flip polarity on truly-unknown verbs.
   For known verbs, negation semantics deferred to Wave 4 inference. Justified
   by SPEC §0.4 "Determinism first" + §5 anti-patterns (rules apply effects,
   parsers don't).
2. `chapter_suffix` always `None` in `anchor_from_position`. ParsedChapter
   stores suffix only in `title` field. Deferred to Wave 5.
3. `parse_text_fallback` accepts sentence-initial common nouns («Потом») as
   phantom actors. Python's spaCy needed for proper PROPN/Name filtering.
   Fallback documented as "intentionally limited" in task brief.

## Cross-module dependencies (no `pub use` from siblings)

- `use crate::reasoning::facts::{Action, Event, EventId, Provenance, VerbPolarity};`
- `use crate::reasoning::timeline::TemporalAnchor;`
- `use crate::models::LitNode;`
- `use crate::parser::chapters::ParsedChapter;`
- `use std::collections::HashMap;`
- `use fancy_regex::Regex;`
- `use serde::{Deserialize, Serialize};`

## Wave 5 integration TODO for coordinator

1. Uncomment `pub mod semantic_parser;` in `src-tauri/src/reasoning/mod.rs`
   (currently under "Wave 3: semantic layer (pending)").
2. Add to re-exports:
   ```rust
   pub use semantic_parser::{
       EntityResolver, SvoTriplet, parse_text_fallback,
       triplets_to_events, verb_to_action,
   };
   ```
3. Wire into ReasoningEngine (Wave 5): build EntityResolver from ParseResult
   nodes, call triplets_to_events after SVO Python invocation, pass events to
   ReasoningCycle::observe.
