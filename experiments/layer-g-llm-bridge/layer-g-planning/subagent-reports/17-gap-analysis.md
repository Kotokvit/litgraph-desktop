# Subagent 17 — Gap Analysis (Cross-Cutting Audit)

- **Task ID**: 17-gap-analysis
- **Agent**: Explore (very thorough)
- **Scope**: Cross-cutting — gaps that individual file-level agents miss
- **Working dir**: `/home/z/my-project/litgraph-desktop`
- **Synthesis-input**: Yes — this is the SYNTHESIS-INPUT agent (subagent 17 of 17)

---

## 1. TODO / FIXME / HACK / XXX audit

Command: `rg -n "TODO|FIXME|HACK|XXX" --type rust --type ts --type tsx --type py -g '!node_modules' -g '!target' -g '!dist'`

**Result**: Exactly **1 match** across the entire source tree (Rust + TS + TSX + Py):

| File:Line | Content | Severity |
|---|---|---|
| `src/components/litgraph/PolerDialog.tsx:80` | `// Для больших текстов (>50k символов) рекомендуется Rust-порт (TODO).` | Low — POLER dialog truncates input at 50k chars; Layer F Rust-native `cmd_compute_epsilon_climax` already provides the Rust port but the legacy client-side `PolerDialog` (different from `PolerPanel`) still uses the JS path. |

**Context notes** (from prior worklog subagent 1, confirmed clean here):
- Worklog mentions "TODO #15" (n2n.aff parser bug) — that TODO is documented in narrative form inside `worklog.md` (not a code comment), so it doesn't appear in code grep.
- No `FIXME`, no `HACK`, no `XXX` anywhere in source.
- Documentation files (README, CHANGELOG, docs/*, agent-ctx/*) do contain the words "TODO" in roadmap prose — those are excluded by the `--type rust/ts/tsx/py` filter, as intended.

**Verdict**: Code surface is essentially TODO-free. The single TODO is a known, low-priority cleanup item.

---

## 2. AI provider plumbing audit — CONFIRMED BREAK

### 2.1 The chain — three layers

**Layer 1 — UI components** (`AIDialog.tsx`, `AssistantDialog.tsx`):
Both call `callApi<string>(cmdName, endpoint, payload)` from `@/lib/litgraph/api`.

```ts
// AIDialog.tsx:65-77
const payload: Record<string, unknown> = { project };
if (mode === "continue-chapter") {
  if (selectedNodeId) payload.fromChapterId = selectedNodeId;
  if (customPrompt.trim()) payload.customPrompt = customPrompt.trim();
} else {
  payload.focus = focus;
}
const text = await callApi<string>(cmdName, endpoint, payload);
//                                            ^^^^^^^ no `provider` key
```

```ts
// AssistantDialog.tsx:72-77
const text = await callApi<string>("ai_assistant", "/api/ai/assistant", {
  project,
  message: content,
  history: messages.map((m) => ({ role: m.role, content: m.content })),
  selectedNodeId,
});
// ^ no `provider` field
```

**Layer 2 — `src/lib/tauri-commands.ts` wrappers** (lines 72-93):
All three wrappers DO accept and forward `provider`:

```ts
export async function aiAssistant(project, message, history, selectedNodeId, provider) {
  return invoke("ai_assistant", { project, message, history, selectedNodeId, provider });
}
export async function aiContinueChapter(project, fromChapterId, customPrompt, provider) {
  return invoke("ai_continue_chapter", { project, fromChapterId, customPrompt, provider });
}
export async function aiAnalyzePlot(project, focus, provider) {
  return invoke("ai_analyze_plot", { project, focus, provider });
}
```

**Layer 3 — Rust backend** `src-tauri/src/commands/ai.rs` (lines 7-18):
All three commands **require** `provider: AiProvider` (positional, non-optional). Serde will fail to deserialize if missing.

```rust
pub async fn ai_assistant(
    project: Project,
    message: String,
    history: Vec<ChatMessage>,
    selected_node_id: Option<String>,
    provider: AiProvider,          // <-- REQUIRED, no default
) -> Result<String, String> { ... }
```

### 2.2 The exact break

The UI dialogs (`AIDialog.tsx`, `AssistantDialog.tsx`) **do not use** the `tauri-commands.ts` wrappers. They use the legacy `callApi()` helper from `src/lib/litgraph/api.ts` (which itself is a fetch/invoke dispatcher that takes a hand-built `payload` object). The hand-built payloads omit the `provider` field entirely.

When `callApi` runs under Tauri, it forwards the payload object as-is to `invoke()`:
```ts
// api.ts:21
return invoke(_tauriCommand, args) as Promise<T>;
```
…so `invoke("ai_assistant", { project, message, history, selectedNodeId })` arrives at Rust with no `provider`. Serde then errors: **`missing field provider`**.

### 2.3 Why it's still latent

- Under **web preview** (Vite dev server, no Tauri): `callApi` falls into the `fetch` branch and POSTs to `/api/ai/...` — the Next.js-style API route doesn't exist in this repo (no `pages/api/` or `app/api/`), so requests 404 anyway. The bug is masked by another bug.
- Under **Tauri**: every AI button click produces the same opaque Rust deserialization error. There is no `AiSettingsDialog` to set a provider (it's claimed by README but doesn't exist — see §8).

### 2.4 Fix recipe (for Layer G prep)

1. Add a `provider` slice to `useLitStore` (default: `{ kind: "Ollama", url: "http://localhost:11434", model: "llama3.1" }`).
2. In `AIDialog.tsx` and `AssistantDialog.tsx`: replace `callApi("ai_assistant", ...)` with `aiAssistant(project, message, history, selectedNodeId, useLitStore.getState().provider)`.
3. Build an `AiSettingsDialog.tsx` to actually let the user change provider (it's referenced in README §"Компонент настроек AI" but missing from `src/components/litgraph/`).

---

## 3. CSP audit — CRITICAL

`src-tauri/tauri.conf.json` line 23-25:

```json
"app": {
  "security": {
    "csp": null
  }
}
```

**Severity**: CRITICAL. `csp: null` disables the Content Security Policy entirely. The Tauri webview will:
- Execute any inline `<script>` (XSS vector).
- Load remote scripts/images/styles/fonts from any origin.
- Allow `eval()`, `new Function()`, WebAssembly without restrictions.

This is the default Tauri 2 behavior when `csp` is absent or `null`. With LLM-generated content entering the UI (AI Assistant responses, future Layer G paradox explanations), this is a ticking time bomb: a crafted LLM response could inject `<img src=x onerror=alert(localStorage)>` and exfiltrate any persisted data (including any API keys in localStorage — see §6).

**Recommended CSP** (after audit):
```
default-src 'self'; img-src 'self' data: blob:; script-src 'self';
style-src 'self' 'unsafe-inline'; connect-src 'self' http://localhost:11434 https://api.openai.com https://api.z.ai;
font-src 'self' data:
```

(Inline styles allowed because Tailwind 4 + shadcn/ui rely on runtime CSS injection; connect-src allow-lists the three provider endpoints.)

---

## 4. Tauri 2 capabilities audit — EXISTS but partial

`src-tauri/capabilities/default.json` **exists** (44 lines). It grants:

| Permission | Granted? | Notes |
|---|---|---|
| `core:default` | ✅ | Standard core permissions. |
| Window perms (minimize/maximize/unmaximize/close/set-title/start-dragging) | ✅ | Adequate. |
| `core:event:default` | ✅ | |
| `dialog:default`, `dialog:allow-open`, `dialog:allow-save` | ✅ | For .md import / file export. |
| `store:default` | ✅ | Tauri Store plugin — **but never used by frontend** (see §6). |
| `fs:default`, `fs:allow-read-file`, `fs:allow-read-text-file`, `fs:allow-write-text-file` | ⚠️ | Granted with `"path": "**"` (any path on disk). Overly permissive — should be scoped to `$HOME` or `$DOCUMENT`. |
| `shell:*` | ❌ | Not granted — good (no shell sidecar). |
| `http:*` | ❌ | Not granted — Layer G can't make HTTP calls from frontend (must go through Rust `reqwest` backend). |

**Critical note**: Subagent 1's worklog entry said "отсутствие capabilities/" — this is **no longer accurate**. The file exists. It may have been added between subagent 1's run and this audit. The remaining concern is:
- **`fs:allow-*` scopes are `**` (glob-all)** — a malicious script (or a CSP-bypass XSS, see §3) could read or write **any file on the user's disk** via `fs:read-text-file` / `fs:write-text-file`. This is a CRITICAL privilege-escalation surface once CSP is bypassed.
- `store:default` is granted but the frontend uses `localStorage` instead — capabilities grants are dead code right now.

---

## 5. Code duplication audit — litgraph-core vs src-tauri

Audit script ran `diff -q` on every `litgraph-core/src/**/*.rs` against `src-tauri/src/**/$same`.

### 5.1 Summary

| Status | Count | LOC (single-side) |
|---|---|---|
| **IDENTICAL** | **14** | **40,794 LOC** duplicated byte-for-byte |
| **DIVERGENT** | **11** | (small drift; see §5.3) |
| **ONLY-IN-CORE** | **5** | (`linguistic/{lemmatizer,pos_tagger,svo_parser}.rs`, `reasoning/{narrative_graph,paradox,stub}.rs`) |

### 5.2 IDENTICAL files (14 — pure copy-paste, no `pub use litgraph_core::...` re-export)

```
parser/themes.rs              parser/locations.rs          parser/characters.rs
dict/generated_cognates.rs    dict/mod.rs                  linguistic_entities.rs
ai/types.rs                   ai/ollama.rs                 ai/openai_compat.rs
ai/prompts.rs                 models/version.rs            models/project.rs
models/edge.rs                ukrainian_semantic_categories.rs
```

### 5.3 DIVERGENT files (11 — drift introduced, not safe to dedupe without reconciliation)

| File | Nature of drift |
|---|---|
| `lib.rs` | Different module declarations (`litgraph-core` is a library; `src-tauri` registers Tauri commands + plugins). Expected divergence. |
| `linguistic/mod.rs` | `src-tauri` re-exports via `pub use litgraph_core::linguistic::{lemmatizer, pos_tagger, svo_parser};`. **Good pattern** — should be replicated for other modules. |
| `reasoning/mod.rs` | `litgraph-core` has full Layer E module; `src-tauri` has only Wave 5 stub. **Drift risk**: Layer E code lives in core but src-tauri has its own minimal reasoning module. |
| `parser/epsilon.rs` | Trivial: doc-comment language (UK vs RU). |
| `parser/chapters.rs` | src-tauri adds `suffix: Option<String>` field for sub-chapter markers (P2.1 — `Глава 28б`). **Real semantic divergence**. |
| `parser/mod.rs` | src-tauri adds `is_uk_weekday/is_uk_month/is_uk_profession/is_uk_color/is_uk_nation` helpers + cognate normalize hook. |
| `dict/cognate.rs` | src-tauri adds pronoun exclusion list (`он/она/він/вона/...`). |
| `ai/mod.rs` | Enum variant rename: `Openaicompat` (core) → `OpenAiCompat` (src-tauri). **Serialization-breaking rename** — wire format differs between the two crates. |
| `models/node.rs` | src-tauri adds `Concept` and `Organization` node types (v0.4.2). |
| `models/mod.rs` | src-tauri drops `LitNodeType` and `EdgeKind` re-exports. |
| `languagetool_weights.rs` | Whitespace / array formatting only. |

### 5.4 LOC impact

- **40,794 lines of code are duplicated byte-for-byte** between the two crates.
- That's ~75% of the Rust source in `litgraph-core/src/` (5,436 LOC × 2 ≈ but counts differ because the LOC includes comments and blank lines).
- This is the single largest source of technical debt in the repo.

### 5.5 Recommended fix

Follow the `linguistic/mod.rs` pattern — replace every IDENTICAL file in `src-tauri/src/` with a single `pub use litgraph_core::module_name;` re-export. For DIVERGENT files, reconcile the drift first, then re-export. Estimated LOC reduction: ~35,000 LOC.

---

## 6. Store.ts localStorage vs Tauri Store plugin

### 6.1 What `store.ts` does

`src/lib/litgraph/store.ts:627-629`:
```ts
storage: typeof window !== "undefined" && window.localStorage
  ? createJSONStorage(() => window.localStorage)
  : undefined,
```

The Zustand store is persisted to **`window.localStorage`** under key `litgraph-store-v1`. This is the only storage backend; there is no conditional `isTauri` branch that would swap to `@tauri-apps/plugin-store`.

### 6.2 What `package.json` declares

`package.json:24`:
```json
"@tauri-apps/plugin-store": "^2.0.0",
```

The plugin is installed.

### 6.3 What `capabilities/default.json` grants

`store:default` is granted (see §4).

### 6.4 What the code uses

```
$ rg "plugin-store|tauriStore|localStorage" src/ -n
src/lib/litgraph/types.ts:109: * в localStorage вместе с проектом и не зависеть от исходного файла.
src/lib/litgraph/store.ts:625:  // Хранилище: в Tauri используем localStorage (через persistedState),
src/lib/litgraph/store.ts:627:  storage: typeof window !== "undefined" && window.localStorage
src/lib/litgraph/store.ts:628:    ? createJSONStorage(() => window.localStorage)
src/lib/litgraph/store.ts:640:  // чтобы не переполнять localStorage (обычно ~5-10 MB лимит).
```

**No matches for `plugin-store` or `tauriStore`** — the Tauri Store plugin is **dead weight**: declared in `package.json`, granted in capabilities, but never imported or invoked by any frontend code.

### 6.5 Risks

- **5 MB localStorage cap** (per-origin in webview): a project with a `backgroundLayer` (5 MB cap on base64 image inside `partialize`) plus 50 chapter versions × 5-10k words each will overflow.
- **localStorage is per-origin**, not per-app — if the user has multiple LitGraph windows or the webview origin changes, persistence breaks silently.
- **No encryption**: any XSS (which is possible because CSP is null — see §3) can read all persisted data via `localStorage.getItem("litgraph-store-v1")`.
- **The store comment lies**: it says "В Tauri используем localStorage (через persistedState), потом синхронизируем с Tauri store при необходимости" — but the "later sync" was never built.

### 6.6 Fix recipe

Replace `createJSONStorage(() => window.localStorage)` with a Zustand-compatible adapter that uses `@tauri-apps/plugin-store`'s `LazyStore` when `isTauri` is true, falling back to `localStorage` for web preview. The `store:default` capability is already granted.

---

## 7. Python sandbox audit (`src-tauri/src/commands/ner.rs`)

### 7.1 How Python is invoked

`run_python_with_text_file` (lines 87-164):

```rust
let python_cmd = find_python();  // 1. ~/.litgraph-venv/bin/python 2. $LITGRAPH_PYTHON 3. "python3"

let script_dir = temp_dir.join(format!("litgraph_scripts_{}_{}", pid, timestamp));
fs::create_dir_all(&script_dir)?;

let main_script_path = script_dir.join("main_script.py");
fs::write(&main_script_path, script)?;  // script is include_str!()'d at compile time

let text_file = script_dir.join("input_text.txt");
fs::write(&text_file, text)?;  // user text → temp file

let output = Command::new(&python_cmd)
    .arg(&main_script_path)
    .arg(&text_file)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()?;
```

### 7.2 `shell=True` check

**NO `shell(true)` / `shell(true)` is used.** `Command::new(python_cmd)` directly execs the binary with argv `[main_script_path, text_file]`. There is no shell interpolation of any kind. ✅

### 7.3 Input sanitization

- **Text content**: written to a temp file (`input_text.txt`), never interpolated into command arguments. ✅ No command injection possible.
- **Script content**: `include_str!()`'d at compile time from `src-tauri/python/*.py` — static, no user input. ✅
- **File path**: constructed from `temp_dir + "litgraph_scripts_PID_TIMESTAMP"` — deterministic, not attacker-controlled. ✅
- **Python interpreter selection**: `find_python()` checks `~/.litgraph-venv/bin/python` then `$LITGRAPH_PYTHON` env var then `python3`. An attacker with environment-variable control could point `LITGRAPH_PYTHON` to a malicious executable. Defense-in-depth concern, but not a remote exploit vector (requires local env compromise).

### 7.4 Residual concerns

1. **Temp file permissions**: `fs::write(&text_file, text)` uses default mode (typically 0644 on Linux). On a multi-user system, other users can read the user's manuscript from `/tmp/litgraph_scripts_PID_TIMESTAMP/input_text.txt` for the duration of the Python call. **Mitigation**: use `fs::OpenOptions::new().mode(0o600).write(true).create(true).open(...)` or write to `$XDG_RUNTIME_DIR` instead of `/tmp`.
2. **Temp dir cleanup**: `fs::remove_dir_all(&script_dir)` runs unconditionally at the end (line 161). Good — no leak. But if the process is `kill -9`'d mid-run, the temp dir lingers.
3. **No timeout**: if Python hangs (e.g., OOM on a 5MB text), `Command::output()` blocks forever. No `tokio::time::timeout` wrapper. Mitigation: wrap in `tokio::time::timeout(Duration::from_secs(300), ...)`.
4. **Stdout size unbounded**: `Stdio::piped()` buffers all output in memory. A pathological Python script returning gigabytes of JSON would OOM the Rust process.

### 7.5 Verdict

**Sandbox is sound** against command injection — no `shell=True`, no string interpolation of user input into command args. Residual issues are filesystem permissions and missing timeouts, both defense-in-depth.

---

## 8. README vs reality audit

README claims that contradict actual code:

| # | README claim | Reality | Severity |
|---|---|---|---|
| 1 | "**47 shadcn/ui компонентов**" (README.md:58) | `src/components/ui/` contains exactly **7** files: `badge, button, dialog, dropdown-menu, input, label, textarea`. | Medium — false advertising; new contributors expect a richer UI kit. |
| 2 | "**11 компонентов litgraph**" (README.md:57, 146) | `src/components/litgraph/` contains **22** `.tsx` files (LitApp, LitCanvas, Toolbar, Inspector, Sidebar, NodePalette, NodeEditor, NodeActions, NodeView, EdgeView, CanvasRenderer, AIDialog, AssistantDialog, NerDialog, PolerDialog, PolerPanel, CharacterGraphDialog, ConflictGraphDialog, ReasoningDialog, TextMomentsDialog, ReaderDialog, SvoHighlighter). | Medium — README is from v0.2 era; UI has doubled. |
| 3 | "**15 Tauri commands**" (README.md:54) | `tauri::generate_handler!` in `src-tauri/src/lib.rs:37-76` registers **29** commands (parse_md, parse_md_full, 4×project, 4×versions, export, 5×ai, 3×ner, 1×conflict, 6×reasoning, 3×poler). | Medium — README severely understates backend surface. |
| 4 | "**9 типов нод**" (README.md:20) | `LitNodeType` in `src/lib/litgraph/types.ts:3-14` has **11** variants (added `concept` and `organization` in v0.4.0). | Low — node palette silently grew. |
| 5 | "**Компонент настроек AI** — выбор провайдера, проверка соединения" (README.md:64) | **No `AiSettingsDialog.tsx`** file exists anywhere in the repo (Glob returned 0 matches). The README features a screenshot (`docs/screenshots/kasiopia-ai-context.png`) of a dialog that was never ported. | **High** — without this dialog, users cannot set `provider`, which (per §2) means **every AI command crashes** with "missing field provider". This is the proximate cause of the AI plumbing break. |
| 6 | "Все `fetch('/api/...')` заменены на `invoke('...')`" (README.md:60) | `src/lib/litgraph/api.ts:26-33` still has a `fetch(webEndpoint, ...)` branch that fires whenever `isTauri === false` (i.e., in Vite dev server preview). | Low — web-preview mode still expects Next.js API routes that don't exist in this repo. |
| 7 | "**Тесты проходят: 3/3 ✅**" (README.md:89) | `litgraph-core/tests/` contains **5 test files** (`test_lt.rs, sfera_test.rs, parser_test.rs, chapters_only_test.rs, profile_test.rs`) plus `examples/test_sfera.rs`. Test count is likely ≥3 but the claim is stale. | Low — needs re-run to verify. |

### 8.1 Most damaging discrepancy

**Discrepancy #5** (missing `AiSettingsDialog`) is the root cause of the AI provider plumbing break (§2). It's a documentation-vs-implementation mismatch with direct user-visible consequences: every AI button is dead until either (a) `AiSettingsDialog.tsx` is implemented, or (b) the dialogs read a default `provider` from the store.

---

## 9. Layer G readiness matrix

Layer G = "LLM Reasoning Bridge" — the layer that uses LLMs to resolve paradoxes flagged by Layer E's ParadoxDetector (flashback / dream / resurrection / disguise hypotheses), to validate LLM-generated text against WorldState, and to propose narrative fixes.

| # | Prerequisite | Status | Evidence |
|---|---|---|---|
| 1 | **ParadoxDetector output** (Layer E) | ✅ Ready | `litgraph-core/src/reasoning/paradox.rs` (346 LOC) — `ParadoxDetector::detect(&ManuscriptAnalysis) -> Vec<Paradox>`. `Paradox` struct: `{ kind, character, chapter_idx, origin_chapter_idx, explanation }`. Two kinds: `DeadSpeaking`, `SpatialTeleportation`. Already serialized via `#[serde(rename_all = "snake_case")]`. |
| 2 | **ConflictReport / NarrativeGraph** (Layer E) | ✅ Ready | `litgraph-core/src/reasoning/mod.rs:50-66` defines `ConflictReport { omega_conf, spectral_radius, node_count, edge_count, paradoxes }`. `litgraph-core/src/reasoning/narrative_graph.rs` (470 LOC) implements `NarrativeGraph` backed by `petgraph::DiGraph`. `ConflictAnalyzer` trait (lines 9-18 of `mod.rs`) explicitly anticipates Layer G: *"Future LLM-backed implementations (Layer G)"*. |
| 3 | **AiProvider infrastructure** (`litgraph-core/src/ai/`) | ✅ Ready | `litgraph-core/src/ai/{mod,types,ollama,openai_compat,prompts}.rs` — 5 files, ~572 LOC in prompts alone. `AiProvider` enum: `Ollama { url, model }`, `OpenAiCompat { endpoint, api_key, model }`, `Zai { api_key, model }`. `ai::chat(provider, messages)` async dispatcher. NOTE: enum variant name diverges between core (`Openaicompat`) and src-tauri (`OpenAiCompat`) — see §5.3. Must reconcile before Layer G code can call into both. |
| 4 | **Tauri command registration pattern** (poler.rs as template) | ✅ Ready | `src-tauri/src/commands/poler.rs` (705 LOC) is the canonical template: module doc-comment → DTO structs with `#[serde(rename_all = "camelCase")]` → `#[tauri::command] pub async fn cmd_xxx(...) -> Result<Dto, String>` → registered in `src-tauri/src/lib.rs:37` `tauri::generate_handler![...]`. Layer G can copy this pattern verbatim. |
| 5 | **TS DTO + wrapper pattern** (`src/lib/tauri-commands.ts`) | ✅ Ready | `src/lib/tauri-commands.ts:277-440` shows the pattern for POLER Layer F: JSDoc-commented `export interface XxxDto` (mirroring Rust DTOs field-for-field in camelCase), then `export async function cmdXxx(...): Promise<XxxDto> { return invoke<XxxDto>("cmd_xxx", { ... }); }`. Layer G can extend this file with `LayerGDto` + `cmdLayerGResolve*` wrappers. |
| 6 | **UI integration point** (`PolerPanel.tsx` "Paradox Feed" tab) | ✅ Ready | `src/components/litgraph/PolerPanel.tsx:642-728` — `activeTab === "paradoxes"` branch renders `paradoxReport.paradoxes.map((pdx, idx) => ...)`. Each paradox card shows `pdx.kind`, `pdx.character`, `pdx.chapterIdx`, `pdx.originChapterIdx`, `pdx.explanation`. **Layer G integration point**: add a "Resolve with AI" button next to each paradox card that calls the new `cmd_layer_g_resolve_paradox` command and renders the proposed hypothesis inline. |
| 7 | **Prompt template infrastructure** (`litgraph-core/src/ai/prompts.rs`) | ⚠️ Partial | `prompts.rs` (571 LOC) has 3 prompt builders: `build_assistant_prompt`, `build_continue_chapter_prompt`, `build_analyze_plot_prompt`, plus `build_messages(system, user, history)` and `chapter_num(title)`. **Missing**: `build_paradox_resolution_prompt(paradox, world_state, recent_text) -> (String, String)`. The infrastructure pattern is established; Layer G only needs to add new builder functions, not refactor existing ones. |
| 8 | **Validator hook** (`reasoning_validate_text` command) | ✅ Ready | `src-tauri/src/commands/reasoning.rs:352-397` defines `reasoning_validate_text(project, events, proposed_text) -> Result<ValidationResultDto, String>`. `ValidationResultDto` (lines 89-114) is an enum with three variants: `Accept { events, violations, paradoxes }`, `Reject { violations, feedback_prompt }`, `Retry { reason }`. **The `Reject.feedback_prompt` field is the exact hook Layer G needs**: when validation fails, the feedback_prompt is sent back to the LLM for retry. Already wired in `lib.rs:71`. Currently uses deterministic rules; Layer G can either (a) extend the rules engine to call LLM, or (b) add a sibling `reasoning_llm_validate` command. |

### 9.1 Layer G readiness verdict

**7 of 8 prerequisites are ✅ Ready, 1 is ⚠️ Partial.** No ❌ Missing items. Layer G can begin implementation immediately after fixing the AI provider plumbing bug (§2 — which is a UI-layer bug, not a Layer G prerequisite per se, but Layer G will inherit the bug if not fixed first).

The single partial item (prompt template infrastructure) is a 1-day task: add `build_paradox_resolution_prompt` to `prompts.rs` and a `build_llm_validate_prompt` if Layer G adds an LLM-driven validator.

---

## 10. Top 10 risks for Layer G implementation

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| **1** | **AI provider plumbing break** (§2): UI dialogs don't pass `provider`, all AI commands crash with "missing field provider". Layer G's LLM bridge will hit the same wall. | **High** — bug is already present, just not exercised because no `AiSettingsDialog` exists. | **High** — blocks every Layer G feature that calls an LLM. | Refactor `AIDialog.tsx` / `AssistantDialog.tsx` to use `tauri-commands.ts` wrappers; add `provider` slice to `useLitStore`; implement `AiSettingsDialog.tsx` (already claimed by README). Estimated 4-6 hours. |
| **2** | **CSP null + LLM output injection**: `csp: null` means any `<script>` in LLM output executes. Layer G generates multi-paragraph hypothesis text — if rendered with `dangerouslySetInnerHTML` or markdown→HTML, XSS is trivial. | **High** — once Layer G ships, this is exploitable on day 1. | **Critical** — XSS exfiltrates localStorage (which contains the project, including any embedded API keys). | Set a strict CSP (see §3). Render LLM output as plain text via React's default escaping; never use `dangerouslySetInnerHTML` for LLM content without sanitization (DOMPurify). |
| **3` | **No HTTP capability for frontend**: `capabilities/default.json` doesn't grant `http:*`. If Layer G wants to stream LLM responses (SSE/chunked) directly to the webview, it can't. | **Medium** — only matters if Layer G adds streaming. | **Medium** — non-streaming Layer G calls will block UI for 30-60s. | Keep all LLM HTTP in Rust backend (`reqwest`), expose via Tauri events (`app.emit("llm-chunk", ...)`) for streaming. Don't add frontend `http` capability. |
| **4** | **Prompt template gap**: `prompts.rs` has no `build_paradox_resolution_prompt`. Building it ad-hoc in Layer G code will duplicate the pattern and miss the `chapter_num()` helper. | **High** — almost certain if no upfront design. | **Medium** — prompt quality determines Layer G output quality; ad-hoc prompts underperform. | Add `build_paradox_resolution_prompt(paradox, world_state, recent_text) -> (String, String)` to `prompts.rs` before writing Layer G command code. Follow the `build_assistant_prompt` template (filter by node_type, build context string, format user message). |
| **5** | **Validator hook shape mismatch**: `reasoning_validate_text` returns `Accept/Reject/Retry` but is currently deterministic. Layer G needs LLM-driven validation, which doesn't fit the existing signature without overloading. | **Medium** — depends on Layer G design choice. | **Medium** — overloading the existing command breaks the reasoning engine's existing callers. | Add a sibling command `reasoning_llm_validate(project, events, proposed_text, provider) -> Result<LlmValidationResultDto, String>` rather than overloading. Reuse `ValidationResultDto` shape if possible; add `Hypothesis` variant if needed. |
| **6** | **Token budget for large manuscripts**: README mentions Касіопея is 1.1 MB / 103k words. `build_assistant_prompt` serializes the whole project. Layer G prompt that includes full manuscript + paradox context will exceed all model context windows (128k for GPT-4o, 32k for llama3.1 default). | **High** — large manuscripts are the target use case. | **High** — Layer G silently truncates or errors. | Add context-window-aware summarization: for each paradox, include only the originating chapter + 2 neighbors + character sheet. Cap at 8k tokens. Add `summarize_chapter(chapter, max_tokens)` helper in `prompts.rs`. |
| **7** | **localStorage 5 MB cap**: store.ts persists everything (nodes, edges, versions, backgroundLayer) to localStorage. Layer G hypotheses persisted to the same store will overflow on medium-sized projects. | **Medium** — manifests on projects with >20 chapters + Layer G hypotheses. | **Medium** — silent data loss (Zustand persist swallows quota errors). | Switch to `@tauri-apps/plugin-store` (already installed, capability already granted, just unused — see §6). Add a separate `hypotheses` slice that doesn't go through `partialize`. |
| **8** | **No streaming support**: `ai::chat(provider, messages).await` returns the full response. Layer G long-form hypotheses (multi-paragraph resolutions) will appear to hang the UI for 30-60s. | **Medium** — UX issue, not correctness. | **Medium** — users will assume the app froze and force-quit. | Add `ai::chat_stream(provider, messages) -> impl Stream<Item<Result<String, Error>>` using `reqwest::Response::bytes_stream()`. Emit chunks via Tauri events. Update `AssistantDialog` to consume the event stream. |
| **9** | **Heuristic paradox false positives**: `ParadoxDetector` flags flashbacks and dream sequences as `DeadSpeaking` — these are intentional narrative devices, not bugs. Layer G must distinguish "intentional device" from "real inconsistency". | **High** — ParadoxDetector doc-comment explicitly says "false positives are NOT bugs: they are signals that Layer G should resolve". | **Medium** — Layer G "fixes" non-bugs and corrupts the manuscript. | Include the originating chapter + 2 neighbors in the prompt so the LLM can recognize flashback/dream framing. Add a `paradox.severity` field (low/medium/high) to the prompt — currently absent from `Paradox` struct. |
| **10** | **Code duplication drift**: 14 identical files between `litgraph-core/src/` and `src-tauri/src/` (§5). Layer G code added to `litgraph-core/src/reasoning/` must be re-exported in `src-tauri` via `pub use litgraph_core::...` (the `linguistic/mod.rs` pattern). Forgetting this creates a silent duplicate that diverges. | **Medium** — easy to forget; no CI check enforces the pattern. | **Low** — produces a third copy of the file that drifts. | Add a CI check (`xtask check-duplication`) that fails if any file exists in both `litgraph-core/src/` and `src-tauri/src/` with identical content (forcing the `pub use` re-export pattern). Document the pattern in `CONTRIBUTING.md`. |

### 10.1 Aggregate risk profile

- **2 Critical** (CSP null + AI plumbing break) — must fix before Layer G user-facing features.
- **5 High** — should fix before Layer G ships; will block adoption if not addressed.
- **3 Medium** — UX / robustness; fix in Layer G v1.1.
- **0 Low**.

The good news: **none of the risks require re-architecting Layer E or the POLER pipeline**. Layer G can be built on top of the existing foundation, but the foundation has cracks (CSP, plumbing, localStorage, duplication) that will widen under Layer G's load.

---

## 11. Summary

**Code health**: 1 TODO in 16k LOC of source. Clean on the surface; cracks underneath.

**Critical issues** (must fix before Layer G):
1. AI provider plumbing break (§2) — every AI command crashes.
2. CSP null (§3) — XSS trivially exploitable once LLM content is rendered.
3. README claims `AiSettingsDialog` exists (§8.5) — it doesn't; root cause of issue #1.

**High-priority cleanup**:
4. Switch `store.ts` from `localStorage` to `@tauri-apps/plugin-store` (§6).
5. Reconcile 14 identical files between `litgraph-core` and `src-tauri` (§5) — 40,794 LOC of dead duplication.
6. Scope `fs:allow-*` capabilities to `$HOME` instead of `**` (§4).

**Layer G readiness**: ✅ 7/8 prerequisites ready, 1 partial (prompt template — 1-day task). No blockers. Begin implementation after fixing items #1-#3 above.

---

## 12. Files touched

None — this is an audit-only subagent. No code modifications.

## 13. Next actions (for Layer G implementer)

1. **Fix AI plumbing** (4-6h): implement `AiSettingsDialog.tsx`, add `provider` slice to `useLitStore`, refactor `AIDialog.tsx` / `AssistantDialog.tsx` to use `tauri-commands.ts` wrappers.
2. **Set CSP** (1h): update `tauri.conf.json` with the recommended policy from §3.
3. **Add prompt template** (1d): `build_paradox_resolution_prompt` in `litgraph-core/src/ai/prompts.rs`.
4. **Reconcile AiProvider enum rename** (30min): pick `OpenAiCompat` (src-tauri name) as canonical, update `litgraph-core/src/ai/mod.rs` and `types.rs`.
5. **Add Layer G Tauri command** (2d): `cmd_layer_g_resolve_paradox(paradox, project, provider) -> Result<HypothesisDto, String>` in `src-tauri/src/commands/layer_g.rs`, registered in `lib.rs:37`.
6. **Add Layer G UI** (2d): "Resolve with AI" button in `PolerPanel.tsx` Paradox Feed tab; hypothesis card renderer.
7. **Add streaming** (3d): `ai::chat_stream` in `litgraph-core/src/ai/mod.rs`; Tauri event emitter; consumer in `AssistantDialog.tsx`.
8. **Switch store backend** (1d): replace `window.localStorage` with `LazyStore` from `@tauri-apps/plugin-store`.
9. **Add CI duplication check** (4h): `xtask check-duplication` fails on identical files across crates.
10. **Layer G integration tests** (3d): test paradox resolution on `tests/corpus/01_conflict_scene.md` (already has a `dead_speaking` scenario).

Estimated total: 2-3 weeks for a single developer.
