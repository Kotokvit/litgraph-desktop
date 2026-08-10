# Subagent Report 04 — `litgraph-core` Models, AI Providers & Standalone Modules

**Task ID:** 04-core-models-ai
**Agent:** Explore (medium thoroughness)
**Scope:** `litgraph-core/src/{lib.rs, models/*, ai/*, languagetool_weights.rs, ukrainian_semantic_categories.rs}`
**Date:** 2026-08-08 (worklog session)

---

## 1. Scope

Inspected 12 files under `/home/z/my-project/litgraph-desktop/litgraph-core/`:

| # | File | LOC | Purpose |
|---|------|-----|---------|
| 1 | `src/lib.rs` | 13 | Crate root: module declarations only (no re-exports) |
| 2 | `src/models/mod.rs` | 13 | Re-exports of public model types |
| 3 | `src/models/node.rs` | 46 | `LitNode`, `LitNodeData`, `LitNodeType`, `Position` |
| 4 | `src/models/edge.rs` | 28 | `LitEdge`, `EdgeData`, `EdgeKind` |
| 5 | `src/models/project.rs` | 68 | `Project`, `ProjectMeta`, `GraphData`, `ParseParams`, `ParseResult`, `ParseStats` |
| 6 | `src/models/version.rs` | 13 | `ChapterVersion` |
| 7 | `src/ai/mod.rs` | 66 | `AiProvider` enum, `AiError`, dispatcher `chat()` / `test_connection()` / `list_ollama_models()` |
| 8 | `src/ai/types.rs` | 17 | `ChatMessage`, `AiResponse` |
| 9 | `src/ai/ollama.rs` | 47 | Ollama HTTP client (chat + list_models) |
| 10 | `src/ai/openai_compat.rs` | 33 | OpenAI-compatible HTTP client |
| 11 | `src/ai/prompts.rs` | 571 | 4 prompt builders + `chapter_num` + `build_messages` |
| 12 | `src/languagetool_weights.rs` | 4917 | Static LT-derived lexical rules (RU/UK) |
| 13 | `src/ukrainian_semantic_categories.rs` | 1950 | Static UK semantic category tables |

Total: **7 822 LOC** in scope (≈48 % of litgraph-core's total Rust LOC).

---

## 2. Atomic Inventory

### 2.1 `lib.rs` (crate root)

- Crate name: `litgraph_core`, version `0.2.0` (per `Cargo.toml`).
- Declares 9 top-level modules: `models`, `parser`, `ai`, `languagetool_weights`, `linguistic_entities`, `ukrainian_semantic_categories`, `dict`, `linguistic`, `reasoning`.
- **No `pub use` re-exports at crate root** — consumers must use fully-qualified paths (`litgraph_core::models::LitNode`).
- Standalone crate (no `tauri` dependency). `Cargo.toml` deps: `serde`, `serde_json`, `fancy-regex`, `regex`, `unicode-segmentation`, `reqwest` (with `json`), `tokio` (full), `dirs`, `chrono`, `uuid`, `thiserror`, `phf`, `flate2`, `petgraph`.

### 2.2 Models layer (`models/`)

| Struct / Enum | Key fields | Serde |
|---|---|---|
| `Position` | `x: f64, y: f64` | `camelCase` |
| `LitNodeType` (enum) | `Scene, Character, Plotpoint, Conflict, Dialogue, Location, Idea, Chapter, Theme` (9 variants) | `lowercase` |
| `LitNodeData` | `title, body, node_type: String` (renamed `"type"`), `tags: Vec<String>`, `meta: Option<serde_json::Value>`, `full_text: Option<String>`, `versions: Option<Vec<ChapterVersion>>` | `camelCase` |
| `LitNode` | `id, node_type: String` (renamed `"type"`), `position, data` | `camelCase` |
| `EdgeKind` | type alias for `String` | — |
| `EdgeData` | `kind: Option<String>, note: Option<String>` | `camelCase` |
| `LitEdge` | `id, source, target, sourceHandle?, targetHandle?, edge_type?` (renamed `"type"`), `animated?, data?` | `camelCase` |
| `Project` | `title, author, description, nodes: Vec<LitNode>, edges: Vec<LitEdge>, created_at: u64, updated_at: u64` | `camelCase` |
| `ProjectMeta` | `id, title, updated_at, size_bytes, node_count, edge_count` | `camelCase` |
| `GraphData` | `nodes, edges` | `camelCase` |
| `ParseParams` | `markdown, project_title, author` | `camelCase` |
| `ParseResult` | `title, author, description, nodes, edges, created_at, updated_at, stats` | `camelCase` |
| `ParseStats` | `chapters, characters, locations, edges, words` (all `usize`) | `camelCase` |
| `ChapterVersion` | `id, timestamp: u64, full_text, word_count: usize, label?, source?` (source string ∈ `"auto"|"manual"|"ai"|"restore"|"import"`) | `camelCase` |

`models/mod.rs` re-exports: `LitNode, LitNodeData, LitNodeType, Position, LitEdge, EdgeData, EdgeKind, Project, ProjectMeta, GraphData, ParseParams, ParseResult, ParseStats, ChapterVersion`.

### 2.3 AI layer (`ai/`)

#### `ai/types.rs`
- `ChatMessage { role: String, content: String }` — `role` is a free-form string (no enum constraint); convention: `"user" | "assistant" | "system"`.
- `AiResponse { text: String, model: Option<String>, tokens_used: Option<u32> }` — **defined but never populated** by any provider implementation (dead struct).

#### `ai/mod.rs` — provider contract
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum AiProvider {
    Ollama      { url: String, model: String },
    Openaicompat{ endpoint: String, api_key: String, model: String },
    Zai         { api_key: String, model: String },
}

#[derive(Debug, thiserror::Error)]
pub enum AiError {
    Network(#[from] reqwest::Error),
    InvalidResponse(String),
    ConnectionFailed(String),
}

pub async fn chat(provider: &AiProvider, messages: Vec<ChatMessage>) -> Result<String, AiError>;
pub async fn test_connection(provider: &AiProvider) -> Result<bool, AiError>;
pub async fn list_ollama_models(url: &str) -> Result<Vec<String>, AiError>;
```

Wire format (TS-facing JSON): `{ "type": "ollama" | "openaicompat" | "zai", ... }`. `Zai` is dispatched to the OpenAI-compat client with hardcoded endpoint `https://api.z.ai/v1`.

#### `ai/ollama.rs`
- `chat(url, model, messages)` → POST `{url}/api/chat` with JSON `{model, messages:[{role,content}], stream:false}`. Reads `body["message"]["content"]` as `&str`; on missing → `""`.
- `list_models(url)` → GET `{url}/api/tags`, returns `body["models"][*].name` as `Vec<String>`.

#### `ai/openai_compat.rs`
- `chat(endpoint, api_key, model, messages)` → POST `{endpoint}/chat/completions` with `bearer_auth(api_key)` and body `{model, messages}`. Reads `body["choices"][0]["message"]["content"]`; missing → `""`.

#### `ai/prompts.rs` — prompt builders
| Function | Signature | Returns |
|---|---|---|
| `chapter_num(title: &str)` | → `Option<u32>` | Regex `(?i)Глава\s+(\d+)` extractor |
| `build_assistant_prompt(project, user_message, &Option<String>)` | → `(String, String)` | `(system, user)` for general assistant. System: "Ты — литературный редактор, соавтор и аналитик…". User: aggregated project context (counts of chapters/scenes/characters/locations/plotpoints/conflicts/themes; top-10 characters; plot points; conflicts; locations; chapter outline; selected node body capped at 4000 chars). |
| `build_continue_chapter_prompt(project, &Option<String>, &Option<String>)` | → `(String, String)` | "Write the next chapter" prompt. Uses last 3 chapters as context (3000-char cap each), filters characters/locations active in those chapters via edge `kind == "character"`/`"location"`, pulls plot points connected to last chapter via `kind == "cause"`. |
| `build_analyze_plot_prompt(project, focus: &str)` | → `(String, String)` | Plot analysis prompt. `focus` ∈ `{"plot","characters","pacing",_}`. Computes avg/min/max chapter word counts, flags underused characters (bottom 5), orphan chapters (no character edges), short (<40% avg) and long (>180% avg) chapters. Output template forces Markdown sections: 🟢 Сильные стороны / 🔴 Слабые места / 💡 Рекомендации / ⚠️ Логические нестыковки. |
| `build_messages(system, user, history: &[ChatMessage])` | → `Vec<ChatMessage>` | Prepends `[system, user]`, then appends last **6** history messages (rev/take(6)/rev pattern). |

All prompts auto-detect language (RU/UK/EN) by instruction ("Отвечай на том же языке…").

### 2.4 `languagetool_weights.rs` (4917 LOC)

- Source: LanguageTool `ru/grammar.xml` (811 rules, 8 categories) + `uk/grammar-barbarism.xml` (212 rules).
- Licence attribution block: LGPL 2.1 (lexical facts not copyrightable; structural compilation is derivative).
- `#![allow(dead_code)]` + `#![allow(clippy::type_complexity)]` — large surface area, partially unused.
- **Public types**: `LexicalRule { rule_id, rule_name, category, rule_type, wrong_token_groups, correct_suggestions, example }`.
- **Static tables**: `RUSSIAN_LT_RULES` (253 rulegroups), `UKRAINIAN_LT_RULES` (129 rulegroups), `RUSSIAN_TAUTOLOGY_RULE_IDS` (22), `RUSSIAN_PARONYM_PAIRS` (34), `RUSSIAN_COLLOCATION_FIXES` (38), `UKRAINIAN_BARBARISM_FIXES` (129).
- **Functions**: `rule_matches(rule, text)`, `find_russian_tautology(text)`, `russian_paronym_correction(word)`, `russian_collocation_fix(text)`, `ukrainian_barbarism_fix(text)`, `find_russian_lt_violation(text)`, `find_ukrainian_lt_violation(text)`, `russian_rules_count()`, `ukrainian_rules_count()`.
- Stem-matching heuristic: exact match → prefix-match (N = len-3, min 5) → strip `-ся`/`-сь` and retry.

### 2.5 `ukrainian_semantic_categories.rs` (1950 LOC)

- 17 `pub const` arrays + 1 paronym tuple array:
  `UK_WEEKDAYS, UK_MONTHS, UK_ABBREV_MONTHS, UK_COLORS, UK_NATIONS, UK_TIME_WORDS, UK_HUMAN_QUALITIES, UK_PROFESSIONS, UK_VVODNOE_WORDS, UK_PREP_V_WORDS, UK_PREP_NA_WORDS, UK_ZAGL_WORDS, UK_DUAL_NUMBER_NOUNS, UK_ROST_WORDS, UK_DEFIS_BUD_WORDS, UK_ABBREVIATIONS, UK_PARONYMS`.
- Sources cited: LanguageTool `uk/grammar.xml`, `uk/replace.txt`, Ukrainian Orthography 2019, SUM dictionary.
- Designed as a mirror of `linguistic_entities.rs` (RU) for UK — every `RU_*` constant has a `UK_*` counterpart.
- Functions: `ukrainian_paronym_correct(word)`, `is_uk_weekday(word)`, `is_uk_month(word)`, `is_uk_color(word)` (truncated list — full inspection of 1950 LOC not performed; further `is_uk_*` predicates likely present below line 1751).

---

## 3. Current State

- **Compiles as a standalone crate** with no `tauri` dependency — confirmed via `Cargo.toml`. `src-tauri` depends on `litgraph_core` via `pub use litgraph_core::parser::epsilon::*`, `litgraph_core::linguistic::*`, `litgraph_core::reasoning`, etc. (verified: `src-tauri/src/poler/mod.rs`, `src-tauri/src/parser/epsilon.rs`, `src-tauri/src/linguistic/mod.rs`).
- **Models layer is stable, well-typed, serde-correct** for TS consumption (`rename_all = "camelCase"`, `type` field renaming).
- **AI provider contract is minimal but functional**: 3 backends (Ollama / OpenAI-compat / Z.ai) all routed through a single `chat()` dispatcher returning raw `String` content. No streaming, no temperature/max_tokens, no tool calling, no JSON-mode.
- **Prompt infrastructure is mature**: 4 domain-specific builders covering assistant chat, chapter continuation, plot analysis, and message history packaging. All are pure functions on `&Project` — no IO, no async, trivially unit-testable.
- **Linguistic tables are massive static data**: combined 6 867 LOC of `&[&str]`/`&[(&str,&str)]`/`LexicalRule` constants. Acts as the "weights" of the rule-based semantic parser.

---

## 4. Gaps

| # | Gap | Severity | Detail |
|---|---|---|---|
| G1 | **`litgraph-core/src/models/node.rs` is OUT OF SYNC with `src-tauri/src/models/node.rs`** | HIGH | src-tauri adds 2 new `LitNodeType` variants: `Concept` and `Organization` (v0.4.2). litgraph-core still has only the original 9. Worklog Task 1 claimed "models/ byte-identical" — this is INCORRECT for `node.rs` (md5 `4b81fce…` vs `54f58f69…`) and `mod.rs` (`4a90780f…` vs `32f76fe4…`). `edge.rs`, `project.rs`, `version.rs` ARE identical. |
| G2 | **`models/mod.rs` re-exports diverge** | MED | core re-exports `LitNodeType, EdgeKind, GraphData`; src-tauri omits these 3. Means downstream code in src-tauri cannot `use litgraph_core::models::LitNodeType` and have it auto-re-exported the same way (though path access works). |
| G3 | **`AiResponse` struct is dead** | LOW | Defined in `types.rs`, re-exported in `mod.rs`, but neither `ollama::chat` nor `openai_compat::chat` returns it — both return `Result<String, AiError>`. `model` and `tokens_used` fields are never populated. Should either be removed or wired into the contract. |
| G4 | **HTTP error handling is silent** | HIGH | Both AI clients call `.json().await?` on the response body without checking `resp.status().is_success()`. On a 4xx/5xx response, the body is parsed as JSON; the path lookups `body["message"]["content"]` / `body["choices"][0]["message"]["content"]` return `None` → `.unwrap_or("")` → returns empty string. Caller cannot distinguish "model returned empty" from "auth failed / 500". |
| G5 | **No timeouts on `reqwest::Client::new()`** | MED | Both AI clients construct a fresh `reqwest::Client::new()` per call (no pooling, no `.timeout()`). A hung Ollama/OpenAI endpoint will block the async task indefinitely. |
| G6 | **No retries / no backoff** | LOW | Single POST; any transient network error fails the call. |
| G7 | **No streaming support** | MED | `stream: false` hardcoded for Ollama; OpenAI-compat omits `stream` (defaults to false). Layer G may want token-by-token streaming for UX. |
| G8 | **No request parameters (temperature, max_tokens, top_p)** | MED | Neither AI client exposes any generation parameters. Layer G cannot tune creativity/length without extending the contract. |
| G9 | **No JSON / structured-output mode** | LOW | OpenAI `response_format: { "type": "json_object" }` not supported. Layer G structured tasks would have to parse free-form text. |
| G10 | **`ChatMessage.role` is a `String`, not an enum** | LOW | No compile-time guarantee that role ∈ `{system, user, assistant}`. Risk of typos in prompt builders. |
| G11 | **`build_messages` history window is hardcoded to 6** | LOW | Not configurable. Long conversations may lose context unexpectedly. |
| G12 | **Prompt builders cap context at fixed sizes** (4000 / 3000 chars) | LOW | No token-aware truncation; UTF-8 boundary-safe char-slicing is used (good), but cap is byte-based, not token-based — may overflow small-context models (e.g., 4k-token Ollama models). |
| G13 | **`AiProvider::Openaicompat` variant name diverges from src-tauri (`OpenAiCompat`)** | LOW | Wire format identical (both `rename_all="lowercase"` → `"openaicompat"`), but Rust variant identifier differs. Cross-crate `match` statements won't compile if migrated. |
| G14 | **`lib.rs` has no `pub use` re-exports** | LOW | Consumers must write `litgraph_core::models::LitNode` instead of `litgraph_core::LitNode`. Stylistic only. |
| G15 | **`AiError::InvalidResponse(String)` variant is never constructed** | LOW | Defined but unused; all parse failures silently return empty strings instead. |

---

## 5. Refactoring Recommendations

| # | Action | Priority | Effort |
|---|---|---|---|
| R1 | **Sync `litgraph-core/src/models/node.rs` with src-tauri** — add `Concept` and `Organization` variants to `LitNodeType`. | P0 | XS |
| R2 | **Sync `litgraph-core/src/models/mod.rs` re-exports** to match src-tauri (drop or align `LitNodeType, EdgeKind, GraphData`). | P1 | XS |
| R3 | **Introduce an `AiClient` trait** with `async fn chat(&self, messages: Vec<ChatMessage>, options: ChatOptions) -> Result<AiResponse, AiError>` where `ChatOptions { temperature, max_tokens, top_p, response_format, stream }`. Have `Ollama` and `OpenAiCompat` implement it. Preserve current free functions as thin wrappers for backward compat. | P1 | M |
| R4 | **Add HTTP status check + `AiError::InvalidResponse` population**: `if !resp.status().is_success() { return Err(AiError::InvalidResponse(format!("HTTP {}: {}", status, body))); }` before `.json()`. | P0 | XS |
| R5 | **Add per-request timeout** via `reqwest::Client::builder().timeout(Duration::from_secs(60)).build()?`. Cache the client in a `once_cell::sync::Lazy` to avoid per-call construction. | P1 | S |
| R6 | **Populate `AiResponse`** — extract `model` from response body (`body["model"]` for Ollama; `body["model"]` for OpenAI). `tokens_used` from `body["usage"]["total_tokens"]`. | P2 | XS |
| R7 | **Convert `ChatMessage.role` to an enum** `ChatRole { System, User, Assistant }` with serde rename_all lowercase. | P2 | S |
| R8 | **Add a `prompts::build_layer_g_prompt` builder** reusing existing pattern (`&Project` → system+user). Layer G should not duplicate prompt scaffolding. | P1 | M |
| R9 | **Add token-aware truncation helper** — `truncate_to_tokens(text, max_tokens)` using `unicode-segmentation` (already a dep) or a tiktoken-rust optional dep. | P2 | S |
| R10 | **Add module-level `pub use` re-exports in `lib.rs`** for top-level types (`LitNode`, `Project`, `AiProvider`, `ChatMessage`) for ergonomic access. | P3 | XS |

---

## 6. Layer G Relevance

**Layer G can directly reuse:**

1. **`AiProvider` enum + `chat()` dispatcher** — the single entry point for all LLM calls. Layer G only needs to construct an `AiProvider` value (deserialized from frontend JSON `{type: "ollama"|"openaicompat"|"zai", ...}`) and call `ai::chat(provider, messages).await`. No provider-specific code needed.
2. **`ChatMessage` + `build_messages(system, user, history)`** — drop-in for assembling chat turns. The 6-message history window is sufficient for short Layer G tasks; for longer ones, R10 above would help.
3. **`prompts::build_assistant_prompt`** — the canonical project-context builder. Layer G domain tasks (e.g., thematic analysis, character consistency check) should reuse the same chapter/character/location/plot/conflict/theme aggregation logic; either call this builder directly or factor its sub-aggregations into reusable helpers.
4. **`prompts::build_analyze_plot_prompt`** — template for output-format-constrained prompts (Markdown section headings). Layer G structured tasks can copy this pattern.
5. **`Project`, `LitNode`, `LitEdge` model types** — stable serde contract; Layer G commands should accept/return these directly.
6. **`languagetool_weights` + `ukrainian_semantic_categories`** — pre-computed linguistic "weights" available for any Layer G feature that needs RU/UK lexical grounding (tautology detection, paronym correction, semantic category tagging). No IO required — pure static data.

**Layer G will need to ADD (not reuse):**

- Structured-output support (JSON mode) — see G9.
- Streaming — see G7.
- Tool/function calling — not present at all.
- Provider-specific options (e.g., Ollama `num_ctx`, OpenAI `response_format`) — see G8.
- Token usage accounting — see G6.

**Readiness verdict:** AI infrastructure is **minimal-but-coherent**. The contract (`AiProvider` + `chat()` + `ChatMessage` + 4 prompt builders) is sufficient for non-streaming, single-shot Russian/Ukrainian/English text generation and analysis. Layer G can build on it immediately for v0-style features. For production-grade Layer G (streaming, structured output, tool use), the contract must be extended per R3/R7/R9.

---

## 7. Next Actions

1. **P0 — Fix drift**: Apply R1 (add `Concept`, `Organization` to `LitNodeType` in litgraph-core) and R2 (align mod.rs re-exports). Verify with `cargo build -p litgraph-core` and `cargo build -p litgraph-desktop` (src-tauri).
2. **P0 — Fix silent HTTP failures**: Apply R4 in both `ollama.rs` and `openai_compat.rs`. Add a unit test simulating a 401 response.
3. **P1 — Trait-ify AI client**: Apply R3 (introduce `AiClient` trait + `ChatOptions`). Keep existing `chat()` free function as a backward-compat shim.
4. **P1 — Prompts for Layer G**: Apply R8 — extract shared aggregation helpers (`collect_chapters`, `collect_characters_in_scope`, `truncate_char_safe`) from `prompts.rs` into `prompts::util` so Layer G builders compose rather than duplicate.
5. **P2 — Populate `AiResponse`**: Apply R6 to surface model name + token usage. Wire into Tauri `commands::ai` for frontend observability.
6. **P3 — Add `lib.rs` re-exports** (R10) for ergonomic `litgraph_core::LitNode` access.

---

## 8. Dependencies

**Upstream (this task depends on):**
- Task 1-analysis (worklog baseline) — note: worklog claim "models/ byte-identical" is partially wrong (G1/G2 must be reported back).

**Downstream (this task unblocks):**
- Any subagent working on `src-tauri/src/ai/` (likely Subagent 05) — must reconcile the `Openaicompat` vs `OpenAiCompat` variant name divergence (G13) and the `mod.rs` re-export divergence (G2).
- Any subagent working on `litgraph-core/src/parser/`, `linguistic/`, `reasoning/`, `dict/` — must respect the `LitNodeType` enum expansion (R1) since parser may need to emit `Concept`/`Organization` nodes.
- Layer G implementation tasks — depend on R3 (trait + ChatOptions) and R8 (reusable prompt helpers).

**Cross-cutting:**
- `languagetool_weights.rs` and `ukrainian_semantic_categories.rs` are pure static data — no runtime dependencies on other litgraph-core modules. Safe to refactor independently.

---

## 9. Summary (3 sentences)

`litgraph-core` ships a **minimal-but-coherent AI stack**: a single `AiProvider` enum dispatching to Ollama / OpenAI-compat / Z.ai backends via `chat()`, plus 4 mature prompt builders (`build_assistant_prompt`, `build_continue_chapter_prompt`, `build_analyze_plot_prompt`, `build_messages`) that Layer G can reuse directly for non-streaming single-shot RU/UK/EN tasks. **Critical gaps for Layer G**: HTTP errors are silently swallowed (G4), no streaming / JSON-mode / tool-calling / temperature control (G7–G9), and the model layer has drifted out of sync with `src-tauri` (`Concept`/`Organization` node types missing in litgraph-core — G1). **Layer G readiness verdict**: usable as-is for v0 features, but the `AiClient` trait + `ChatOptions` refactor (R3) and HTTP-status hardening (R4) are mandatory before production.
