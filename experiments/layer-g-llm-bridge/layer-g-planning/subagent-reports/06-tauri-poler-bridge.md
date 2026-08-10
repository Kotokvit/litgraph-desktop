# Subagent 06 — Tauri-side POLER Bridge & Duplication Audit

**Task ID:** 06-tauri-poler-bridge
**Agent:** Explore (medium thoroughness)
**Scope:** `src-tauri/src/poler/mod.rs` + byte-for-byte duplication audit of `src-tauri/src/{parser,linguistic,dict,ai,models}/**` and 3 top-level files vs. their `litgraph-core/src/**` counterparts.

---

## 1. Executive Summary

`litgraph-core` is **already declared as a path Cargo dependency** of `src-tauri` (`src-tauri/Cargo.toml` line 18: `litgraph-core = { path = "../litgraph-core" }`). The `poler/mod.rs` module is a textbook re-export shim — 38 lines, zero custom logic, just `pub use litgraph_core::*` for Layer E (reasoning) + Layer D (`compute_epsilon_climax_with_analyzer`) + Layer C (`SvoParser`) + Layer B (`detect_characters`, `detect_chapters`).

Yet, of 25 paired files in scope, **14 are byte-identical duplicates** (40,794 LOC of pure copy-paste) and **6 have diverged** in conflicting directions (tauri-side has features core lacks AND vice-versa). Only **3 files use the clean re-export pattern** (`pub use litgraph_core::*`) that should be the universal model.

The duplication is **not** a structural necessity — it is unrefactored technical debt from before `litgraph-core` was split out. `poler/mod.rs` itself is the existence proof that the refactor is feasible: it lives in `src-tauri`, consumes `litgraph_core::*`, and works.

---

## 2. `poler/mod.rs` — What it actually does

**Path:** `/home/z/my-project/litgraph-desktop/src-tauri/src/poler/mod.rs` (39 LOC, 1837 bytes)

It is **pure re-exports, no custom logic**:

```rust
pub use litgraph_core::reasoning::{
    ConflictAnalyzer, ConflictReport, ManuscriptAnalysis, NarrativeGraph, ParadoxDetector,
    Paradox, ParadoxKind,
};
pub use litgraph_core::parser::epsilon::{compute_epsilon_climax_with_analyzer, EpsilonResult};
pub use litgraph_core::linguistic::svo_parser::{SvoParser, SvoTriplet};
pub use litgraph_core::parser::characters::{detect as detect_characters, ParsedCharacter};
pub use litgraph_core::parser::chapters::{detect as detect_chapters, ParsedChapter};
```

**Module docstring rationale** (lines 19–25): Layer E (symbolic narrative-graph analysis) lives in `litgraph-core` to keep it testable without Tauri; `src-tauri/src/reasoning/` is a *separate, legacy* Wave 1–5 reasoning engine (Facts/State/Inference/Cycle/Planner) that is Tauri-specific. The `poler` namespace exists to avoid name collisions between the two.

**Consumer:** `src-tauri/src/commands/poler.rs` (lines 38–41) imports everything from `crate::poler::{...}` and exposes 3 Tauri IPC commands: `cmd_compute_epsilon_climax`, `cmd_extract_svo`, `cmd_detect_paradoxes` — all pure/deterministic, no LLM calls.

**Verdict:** `poler/mod.rs` is exemplary. It is the exact pattern every other duplicated module should follow.

---

## 3. Duplication Audit

### 3.1 Method

```bash
cmp -s litgraph-core/$f src-tauri/$f   # byte-identity check
wc -l < ...                              # LOC for diverged pairs
md5sum ...                               # cross-check via hash grouping
```

### 3.2 Results matrix

| # | File | core LOC | tauri LOC | Status | Notes |
|---|------|---------:|----------:|--------|-------|
| 1 | `parser/chapters.rs` | 349 | 358 | **DIVERGED** | tauri added `suffix: Option<String>` field on `ParsedChapter` (P2.1) |
| 2 | `parser/characters.rs` | 897 | 897 | identical | — |
| 3 | `parser/locations.rs` | 103 | 103 | identical | — |
| 4 | `parser/themes.rs` | 209 | 209 | identical | — |
| 5 | `parser/epsilon.rs` | 1001 | 5 | **RE-EXPORT SHIM** | `pub use litgraph_core::parser::epsilon::*` — the good pattern |
| 6 | `parser/mod.rs` | 2112 | 2161 | **DIVERGED** | tauri added `ukrainian_semantic_categories::*` calls in token classification |
| 7 | `linguistic/mod.rs` | 29 | 8 | **RE-EXPORT SHIM** | `pub use litgraph_core::linguistic::{lemmatizer,pos_tagger,svo_parser}` |
| 8 | `linguistic/lemmatizer.rs` | — | — | **core-only** (tauri uses via shim) | Correct |
| 9 | `linguistic/pos_tagger.rs` | — | — | **core-only** | Correct |
| 10 | `linguistic/svo_parser.rs` | — | — | **core-only** | Correct |
| 11 | `dict/mod.rs` | 2 | 2 | identical | — |
| 12 | `dict/cognate.rs` | 67 | 85 | **DIVERGED** | tauri added RU/UK pronoun filter (`он/она/він/вона/...` returns `None`) |
| 13 | `dict/generated_cognates.rs` | 10220 | 10220 | identical | — |
| 14 | `ai/mod.rs` | 65 | 73 | **DIVERGED** | tauri renamed `Openaicompat` → `OpenAiCompat` (serde-tag breaking change) + dropped `AiResponse` re-export |
| 15 | `ai/prompts.rs` | 571 | 571 | identical | — |
| 16 | `ai/openai_compat.rs` | 32 | 32 | identical | — |
| 17 | `ai/ollama.rs` | 46 | 46 | identical | — |
| 18 | `ai/types.rs` | 16 | 16 | identical | — |
| 19 | `models/mod.rs` | 12 | 12 | **near-identical (shim)** | tauri drops `LitNodeType`/`EdgeKind`/`GraphData` from re-export list |
| 20 | `models/node.rs` | 45 | 51 | **DIVERGED** | tauri added `Concept` + `Organization` enum variants (v0.4.2) |
| 21 | `models/edge.rs` | 27 | 27 | identical | — |
| 22 | `models/project.rs` | 67 | 67 | identical | — |
| 23 | `models/version.rs` | 12 | 12 | identical | — |
| 24 | `linguistic_entities.rs` | 26642 | 26642 | identical | — |
| 25 | `languagetool_weights.rs` | 4917 | 3949 | **DIVERGED (tauri SHORTER)** | core has +968 LOC; rules rewritten/refactored |
| 26 | `ukrainian_semantic_categories.rs` | 1950 | 1950 | identical | — |

### 3.3 Aggregates

| Metric | Value |
|---|---:|
| Paired files in scope | 25 |
| Byte-identical duplicates | **14** |
| Total LOC of byte-identical duplication (tauri-side) | **40,794** |
| Intentional re-export shims (good pattern) | **3** (`parser/epsilon.rs`, `linguistic/mod.rs`, ~`models/mod.rs`) |
| Truly diverged files with conflicting drift | **6** |
| `core-only` files (already correctly delegated) | 3 (`lemmatizer/pos_tagger/svo_parser`) |
| Total `src-tauri/src` .rs LOC | 72,237 |
| Total `litgraph-core/src` .rs LOC | 52,800 |
| **Duplicated LOC as % of src-tauri .rs** | **≈ 56.5 %** |

### 3.4 Hash-grouping cross-check (`md5sum | uniq -c`)

Of 24 files in `parser/ ai/ models/ dict/` of both trees, **12 hashes appear exactly twice** (= byte-identical pairs). Combined with 2 top-level duplicates (`linguistic_entities.rs`, `ukrainian_semantic_categories.rs`) → **14 byte-identical pairs**, matches the matrix.

---

## 4. Why does `src-tauri` duplicate `litgraph-core`?

Three converging reasons, in order of historical plausibility:

1. **Original monolith.** All logic lived in `src-tauri/src/`. When `litgraph-core` was extracted (commit history shows crate added 2026-08-08) the source files were copied verbatim into `litgraph-core/src/` so the pure-Rust crate could be tested without Tauri. The originals in `src-tauri/src/` were never deleted.

2. **Cargo dep added late, refactor never finished.** `litgraph-core = { path = "../litgraph-core" }` was wired into `src-tauri/Cargo.toml`, and the `poler/` bridge module was written to use it — but the older modules (`parser`, `models`, `ai`, `dict`, `linguistic_entities`, `languagetool_weights`, `ukrainian_semantic_categories`) were left as in-tree `mod` declarations in `lib.rs` (lines 7–16) instead of being converted to `pub use litgraph_core::*` shims.

3. **Drift after split.** Several files were patched on the `src-tauri` side without backporting to `litgraph-core`:
   - `parser/chapters.rs`: `ParsedChapter::suffix` field added for v0.4.x TemporalAnchor sort (P2.1).
   - `models/node.rs`: `Concept` + `Organization` enum variants added (v0.4.2).
   - `dict/cognate.rs`: pronoun-blacklist filter added to prevent `вона → листуватися` mis-mapping.
   - `ai/mod.rs`: `Openaicompat` → `OpenAiCompat` serde rename + dropped `AiResponse` re-export.
   - `parser/mod.rs`: extra `ukrainian_semantic_categories::*` calls.
   - `languagetool_weights.rs`: **opposite direction** — core has +968 LOC over tauri, meaning the canonical copy in `litgraph-core` is now ahead, while the tauri in-tree copy is stale.

This drift is the **strongest argument for the refactor**: the project is already shipping two divergent implementations of the same business logic, and which one is "ahead" depends on the file.

---

## 5. Refactoring recommendation

### 5.1 Feasibility: **HIGH** — nothing structural blocks it

`litgraph-core` is already a Cargo dep. The `poler/mod.rs` bridge proves the pattern works end-to-end (Tauri command → `crate::poler::*` → `litgraph_core::*` → IPC DTO). No Tauri-specific glue exists inside the duplicated files — they are pure data structures, parsers, and rule tables.

### 5.2 The fix is a one-line-per-module change in `src-tauri/src/lib.rs`

Replace:

```rust
mod parser;
mod models;
mod ai;
pub mod dict;
pub mod languagetool_weights;
pub mod linguistic_entities;
pub mod ukrainian_semantic_categories;
pub mod linguistic;   // already a shim ✓
```

with:

```rust
pub use litgraph_core::{parser, models, ai, dict};
pub use litgraph_core::{languagetool_weights, linguistic_entities, ukrainian_semantic_categories};
pub mod linguistic;   // unchanged (already a shim)
mod poler;             // unchanged
```

…and delete the 14 duplicated files + 6 diverged files. **Net deletion: ~46,000 LOC of dead-equivalent code from `src-tauri/src/`.**

### 5.3 What's blocking the refactor — and the order to unblock it

The 6 diverged files need their tauri-side patches **upstreamed into `litgraph-core`** first, otherwise the refactor silently regresses features:

| Order | File | Tauri-only patch to upstream |
|------:|------|------------------------------|
| 1 | `models/node.rs` | Add `Concept`, `Organization` variants to `LitNodeType` |
| 2 | `parser/chapters.rs` | Add `suffix: Option<String>` to `ParsedChapter` + parsing logic |
| 3 | `dict/cognate.rs` | Add RU/UK pronoun blacklist in `normalize_token` |
| 4 | `ai/mod.rs` | Rename `Openaicompat` → `OpenAiCompat` (NOTE: serde-tag breaking change — frontend must agree) |
| 5 | `parser/mod.rs` | Backport the 7 `ukrainian_semantic_categories::*` callsites |
| 6 | `languagetool_weights.rs` | **Opposite direction**: port core's +968 LOC of newer rules *down* into tauri (or rather, just delete tauri's copy — core is the canonical newer version) |

After step 6, `src-tauri/src/linguistic_entities.rs` (26,642 LOC), `src-tauri/src/dict/generated_cognates.rs` (10,220 LOC), and `src-tauri/src/languagetool_weights.rs` (3,949 LOC) — the three heaviest duplicates — disappear from `src-tauri` entirely.

### 5.4 Risk callouts

- **`ai/mod.rs` serde rename**: `Openaicompat` → `OpenAiCompat` changes the JSON wire-format of `AiProvider` from `{"type":"openaicompat"}` to `{"type":"openaicompat"}` — wait, the `rename_all = "lowercase"` is on the enum tag, so `OpenAiCompat` serializes as `"openaicompat"` (lowercased) which is byte-identical to the old `Openaicompat` form. **No wire-break**, but the Rust-side identifier changes everywhere it's referenced. Just search/replace in `commands/ai.rs` etc.
- **`models/mod.rs` dropped re-exports**: tauri's `models/mod.rs` does NOT re-export `LitNodeType`, `EdgeKind`, `GraphData`. If any tauri module references these via `crate::models::LitNodeType`, it currently fails to compile — so they probably reference them via `crate::models::node::LitNodeType` directly. After the refactor, callers must use the fully-qualified path or the re-exports must be restored.
- **`languagetool_weights.rs`**: 968-line gap means tauri is missing rules that core has. Refactoring will *fix* a latent bug, not introduce one.
- **`commands/parse_md.rs` / `commands/parse_md_full.rs`**: heavy consumers of `crate::parser::*` and `crate::models::*` — must recompile against the re-exported `litgraph_core` versions. No semantic change expected since byte-identical duplicates.

---

## 6. Findings — bulleted

- **`poler/mod.rs` is a 39-line pure re-export shim** with zero custom logic. It exposes Layer E (`NarrativeGraph`, `ParadoxDetector`, `ConflictAnalyzer`, `ConflictReport`, `ManuscriptAnalysis`, `Paradox`, `ParadoxKind`) + Layer D (`compute_epsilon_climax_with_analyzer`, `EpsilonResult`) + Layer C (`SvoParser`, `SvoTriplet`) + Layer B (`detect_characters`, `detect_chapters`). Consumed by `commands/poler.rs` for 3 IPC commands.
- **`litgraph-core` is already a Cargo path-dep** of `src-tauri` — `litgraph-core = { path = "../litgraph-core" }` in `src-tauri/Cargo.toml`. The duplication is **not** a workaround for missing dep wiring.
- **14 byte-identical duplicate files** = **40,794 LOC** of pure copy-paste in `src-tauri/src/`. Heaviest culprits: `linguistic_entities.rs` (26,642 LOC), `dict/generated_cognates.rs` (10,220 LOC), `ukrainian_semantic_categories.rs` (1,950 LOC).
- **6 files have diverged with conflicting drift**: 5 where tauri is ahead of core (`chapters.rs`, `parser/mod.rs`, `cognate.rs`, `ai/mod.rs`, `models/node.rs`) and 1 where core is ahead of tauri (`languagetool_weights.rs`, +968 LOC).
- **3 files are already clean re-export shims** proving the refactor pattern works: `parser/epsilon.rs` (5 LOC), `linguistic/mod.rs` (8 LOC), and approximately `models/mod.rs` (12 LOC, near-shim).
- **`src-tauri/src/lib.rs`** (lines 6–17) declares 7 in-tree modules that should be replaced by `pub use litgraph_core::*`: `parser`, `models`, `ai`, `dict`, `languagetool_weights`, `linguistic_entities`, `ukrainian_semantic_categories`.
- **Net impact of refactor**: delete ~46,000 LOC from `src-tauri/src/` (≈ 64 % of its current 72,237 LOC), single canonical source of truth in `litgraph-core`, drift becomes impossible by construction.
- **No semantic blockers**: all duplicated files are pure data structures, rule tables, or parsers with no Tauri-side state, no `tauri::` imports, no IPC handlers.
- **No `TODO`/`FIXME`/`HACK`/`XXX` markers** in the duplicated files indicating awareness of the debt.
- **`commands/poler.rs`** (verified head-of-file) shows the bridge pattern is healthy: DTOs are defined locally in tauri with `#[serde(rename_all = "camelCase")]` for the React frontend, while the actual computation flows through `crate::poler::*` → `litgraph_core::*`. This is the correct layering.
- **`src-tauri/src/reasoning/`** is a *different*, legacy subsystem (Facts/State/Inference/Cycle/Planner, 13 files) and is **not** a duplicate of `litgraph-core/src/reasoning/` (which is Layer E narrative-graph analysis, 4 files). The two coexist intentionally — see `poler/mod.rs` docstring lines 19–25.

---

## 7. Next actions (concrete, ordered)

1. **Backport tauri-only patches into `litgraph-core`** (6 files, see §5.3 table). Each is a small, isolated diff — total estimated effort: 2–4 hours including tests.
2. **Run `litgraph-core` test suite** (`cd litgraph-core && cargo test`) after each backport to ensure no regression — the crate has its own `tests/` directory with `parser_test.rs`, `chapters_only_test.rs`, `profile_test.rs`, `sfera_test.rs`, `test_lt.rs`.
3. **Replace `src-tauri/src/lib.rs` module declarations** per §5.2.
4. **Delete the 20 duplicated files** from `src-tauri/src/` (14 identical + 6 diverged-after-backport).
5. **`cargo build` + `cargo test` in `src-tauri/`** — fix any path adjustments (e.g. `crate::models::LitNodeType` → `crate::models::node::LitNodeType`).
6. **Verify frontend IPC contract unchanged**: spot-check `commands/ai.rs` JSON serialization for `AiProvider` (the `OpenAiCompat` rename is the only wire-touching change, and it serializes identically due to `rename_all = "lowercase"`).
7. **Add a CI guard** (e.g. a `xtask` script or `diff -r litgraph-core/src src-tauri/src` check) so the duplication cannot silently regress in future PRs.

---

## 8. Three-sentence summary (for the orchestrator)

`src-tauri` duplicates 14 byte-identical files (40,794 LOC) and 6 divergent files from `litgraph-core`, even though `litgraph-core` is already a path Cargo dependency — the duplication is unrefactored technical debt, not a structural necessity. `poler/mod.rs` (39 lines, pure `pub use litgraph_core::*` re-exports) is the existence proof that the bridge pattern works. Refactor feasibility is **HIGH**: backport 6 small tauri-only patches into `litgraph-core`, then replace 7 `mod` declarations in `src-tauri/src/lib.rs` with `pub use litgraph_core::*` shims and delete ~46,000 LOC of duplicated source.
