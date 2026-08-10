# POLER Layer G + GUI Refactoring: Detailed Work Prompt

> **Document ID**: `99-DETAILED_WORK_PROMPT.md`
> **Source**: Synthesized from 17 atomic subagent reports (5,879 LOC total) in `/home/z/my-project/subagent-reports/`
> **Repository State**: `c143a7f` on `feature/symbolic-ua-lp-engine`
> **Status**: **AWAITING USER APPROVAL** — do not execute until approved
> **Estimated Total Effort**: 40–60 hours (4 work weeks at 10–15 h/week)

---

## Part A — Executive Summary

### A.1 Project State Snapshot

LitGraph Desktop v0.2.2 is a Tauri 2 + React 19 + Rust desktop application for
computational narratology on Ukrainian literature. The codebase spans **177
source files** (5,436 LOC Rust, 2,975 LOC TS, 5,224 LOC TSX, 1,306 LOC Python)
plus **8 POLER specification documents**.

**What works end-to-end (Layers A–F.2):**
- Layer A (chapter detection), B (character/location/theme), C (SVO parser),
  D (ε_climax v7.5-LEM), E (NarrativeGraph + ParadoxDetector + ConflictAnalyzer)
  are implemented in `litgraph-core` and duplicated into `src-tauri`.
- Layer F.1 (Tauri IPC: `cmd_compute_epsilon_climax`, `cmd_extract_svo`,
  `cmd_detect_paradoxes`) is registered and tested (289 src-tauri tests PASS).
- Layer F.2 (React Visualizer: `PolerPanel.tsx` + `SvoHighlighter.tsx` +
  TS DTOs in `tauri-commands.ts`) shipped in `c143a7f`, typechecks clean.
- Test suite: **413 Rust tests PASS** (117 litgraph-core + 7 integration + 289 src-tauri).

**What's broken or missing:**
- AI commands fail at runtime — dialogs don't pass `provider` to `invoke()`.
- `tauri.conf.json` CSP is `null` (CRITICAL security issue).
- `capabilities/default.json` grants `fs:**` wildcard (overly permissive).
- 14 byte-identical files duplicated between `litgraph-core` and `src-tauri`
  (40,794 LOC of pure copy-paste).
- `@xyflow/react` is imported but never rendered (240 LOC dead code).
- README claims 47 shadcn/ui components (actually 7); CHANGELOG stops at v0.2.1.
- Zero TypeScript tests; CI runs only on tag push, no quality gates.
- Layer G spec exists only as 12 lines of prose in the master roadmap.

### A.2 Layer G Readiness Matrix

| Prerequisite | Status | Source |
|---|---|---|
| ParadoxDetector output (Layer E) | ✅ Ready | `litgraph-core/src/reasoning/paradox.rs` |
| ConflictReport / NarrativeGraph | ✅ Ready | `litgraph-core/src/reasoning/narrative_graph.rs` |
| AiProvider infrastructure | ✅ Ready | `litgraph-core/src/ai/{types,ollama,openai_compat}.rs` |
| Tauri command pattern (poler.rs template) | ✅ Ready | `src-tauri/src/commands/poler.rs` |
| TS DTO + wrapper pattern | ✅ Ready | `src/lib/tauri-commands.ts` (Layer F.2) |
| UI integration point (PolerPanel Paradox Feed) | ✅ Ready | `src/components/litgraph/PolerPanel.tsx` |
| Validator hook (`reasoning_validate_text`) | ✅ Ready | `src-tauri/src/commands/reasoning.rs` |
| Prompt template infra (`build_*_prompt`) | ⚠️ Partial | `litgraph-core/src/ai/prompts.rs` — needs new `build_paradox_resolution_prompt` |

**Verdict: 7/8 ✅, 1/8 ⚠️, 0/8 ❌ — Layer G can begin immediately after fixing the AI plumbing bug.**

---

## Part B — Critical Blockers (P0: Must-Fix Before Layer G)

### B.1 AI Provider Plumbing Bug (CRITICAL)

**Symptom**: Every AI invoke (`ai_assistant`, `ai_continue_chapter`, `ai_analyze_plot`) fails at runtime with `missing field \`provider\``.

**Root cause** (confirmed by subagents 05, 08, 11, 17):
- `src-tauri/src/commands/ai.rs:13/25/43` declares `provider: AiProvider` as required.
- `src/lib/tauri-commands.ts:72-93` wrappers (`aiAssistant`, `aiContinueChapter`, `aiAnalyzePlot`) correctly forward `provider`.
- `src/components/litgraph/AIDialog.tsx:69-77` and `AssistantDialog.tsx:71-77` **bypass the typed wrappers** and call `callApi()` from `lib/litgraph/api.ts` with a hand-crafted payload that **omits `provider`**.
- The missing `AiSettingsDialog.tsx` (claimed in README, never created) is the proximate root cause — there's no UI to configure the provider.

**Fix plan**:
1. Create `src/components/litgraph/AiSettingsDialog.tsx` — a shadcn Dialog with provider selector (Ollama / OpenAI-compat / Z.ai), model name, API URL, API key, temperature.
2. Persist provider config in Zustand store + Tauri Store plugin (not localStorage).
3. Refactor `AIDialog.tsx` and `AssistantDialog.tsx` to use the typed `aiAssistant` / `aiContinueChapter` wrappers from `tauri-commands.ts` and pass the stored provider.
4. Add a "⚙ AI Settings" button to Toolbar that opens `AiSettingsDialog`.

**Effort**: M (6–8 hours)
**Blocks**: Layer G (the LLM bridge needs a working `AiProvider` to call)

### B.2 CSP null + Capabilities Over-permissive (CRITICAL Security)

**Findings** (subagents 15, 16, 17):
- `src-tauri/tauri.conf.json:24` has `"csp": null` — XSS trivially exploitable, especially once LLM-generated content is rendered.
- `src-tauri/capabilities/default.json` exists (44 lines) but grants `fs:allow-read-*` and `fs:allow-write-*` with scope `"**"` — a compromised renderer could read `~/.ssh/id_rsa` or overwrite `~/.bashrc`.
- `store:default` capability is granted but unused (frontend uses localStorage).

**Fix plan**:
1. Set CSP to a restrictive policy: `"default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' ipc: http://ipc.localhost"`.
2. Replace `fs:**` wildcards with explicit paths: project save dir, export dir, background image dir.
3. Remove unused `store:default` capability OR migrate store to Tauri Store plugin (see B.4).

**Effort**: S (2–3 hours)
**Blocks**: Layer G (LLM content rendering requires CSP)

### B.3 Two Parallel Paradox Types (Architectural Split)

**Finding** (subagent 03): `litgraph-core/src/reasoning/paradox.rs::Paradox` (Layer E, POS-based) and `src-tauri/src/reasoning/contradictions.rs::TemporalParadox` (Wave 5, semantic-IR-based) are two parallel types. `cmd_detect_paradoxes` (Layer F.1) emits the former; `reasoning_validate_text` emits the latter. Layer G must consume both or unify them.

**Fix plan**:
1. Add an `id: String` field and `evidence_text: Vec<String>` field to Layer E `Paradox` struct (breaking change — update tests).
2. In `src-tauri/src/commands/poler.rs::ParadoxDto`, expose the new fields.
3. In `tauri-commands.ts::ParadoxDto`, mirror the new fields.
4. Document in `POLER_LAYER_G_IMPLEMENTATION_PLAN.md` which paradox type each Layer G function consumes.
5. **Do NOT unify the two types** — they serve different purposes (Layer E is deterministic POS-based; Wave 5 is semantic-IR-based with constraint engine). Unification would break both.

**Effort**: M (4–6 hours)
**Blocks**: Layer G hypothesis generation (needs `Paradox.id` to reference in prompts)

### B.4 Store.ts localStorage → Tauri Store Plugin Migration

**Finding** (subagents 09, 17): `@tauri-apps/plugin-store` is in `package.json` and registered in `src-tauri/src/lib.rs:23`, but `src/lib/litgraph/store.ts:627` still uses `window.localStorage`. The 5MB `backgroundLayer` cap is artificial (Tauri Store has no such limit). `sourceMarkdown` is not persisted.

**Fix plan** (per subagent 09's 30-LOC adapter):
1. Create `src/lib/litgraph/tauri-store-adapter.ts` — a small wrapper around `@tauri-apps/plugin-store` with the same `getItem`/`setItem`/`removeItem` signature as localStorage.
2. In `store.ts:627`, replace `window.localStorage` with the adapter (gated on `isTauriEnv()`).
3. Persist `sourceMarkdown` (1-line fix).
4. Remove the 5MB `backgroundLayer` cap (or raise to 50MB).

**Effort**: S (2–3 hours)
**Blocks**: AI provider config persistence (B.1 depends on this)

---

## Part C — GUI Refactoring Plan (Priority-Ordered)

### C.1 [P1] Extract DialogHost.tsx from Toolbar.tsx

**Problem** (subagent 10): `Toolbar.tsx` is 951 LOC and manages 10 dialog states (`metaOpen`, `exportOpen`, `importMdOpen`, `assistantOpen`, `polerOpen`, `polerPanelOpen`, `nerOpen`, `charGraphOpen`, `conflictGraphOpen`, `reasoningOpen`) plus mounts 11 `<Dialog>` components inline.

**Fix**: Create `src/components/litgraph/DialogHost.tsx` that:
- Reads dialog open-states from Zustand store (add a `dialogState` slice).
- Renders all 11 dialogs in one place.
- Toolbar.tsx just dispatches `openDialog("polerPanel")` etc.

**Effort**: M (4–5 hours) | **Benefit**: Toolbar.tsx drops from 951 → ~400 LOC; dialog lifecycle centralized.

### C.2 [P1] Refactor PolerPanel.tsx to Use shadcn Dialog

**Problem** (subagent 11): `PolerPanel.tsx` rolls its own `fixed inset-0 z-50` custom modal markup, missing Radix's focus trap, scroll-lock, and Escape handling. It uses dark `slate-900` theme, diverging from every other dialog's light theme.

**Fix**: Replace the custom modal wrapper with:
```tsx
<Dialog open={isOpen} onOpenChange={(o) => !o && onClose()}>
  <DialogContent className="max-w-5xl h-[85vh] bg-slate-900 text-slate-100 border-slate-700/80">
    {/* existing content */}
  </DialogContent>
</Dialog>
```
This inherits Radix UX (focus trap, scroll-lock, Escape) while keeping the dark visual identity.

**Effort**: S (1–2 hours) | **Benefit**: UX consistency; removes manual keydown listener.

### C.3 [P1] Extract useDerivedText Hook

**Problem** (subagent 10): `collectedText`, `selectedChapterText`, `selectedChapterIndex` selectors in `Toolbar.tsx` are un-memoized and recomputed on every render.

**Fix**: Create `src/lib/litgraph/hooks.ts`:
```ts
export function useDerivedText() {
  const collectedText = useLitStore(/* ... */);
  const selectedChapterText = useLitStore(/* ... */);
  const selectedChapterIndex = useLitStore(/* ... */);
  return { collectedText, selectedChapterText, selectedChapterIndex };
}
```

**Effort**: S (1 hour) | **Benefit**: Reusable across components; cleaner Toolbar.

### C.4 [P2] Remove Dead @xyflow/react Dependency

**Problem** (subagent 10): `@xyflow/react` is imported in `App.tsx`, `LitNodeView.tsx`, `LitEdgeView.tsx` but never rendered — the app uses custom `CanvasRenderer.tsx` (862 LOC) for all canvas rendering.

**Fix**:
1. Remove `<ReactFlowProvider>` from `App.tsx`.
2. Remove `@xyflow/react` imports from `LitNodeView.tsx` and `LitEdgeView.tsx`.
3. Uninstall `@xyflow/react` from `package.json`.
4. **Decision point**: Either commit to custom canvas (delete ReactFlow remnants) OR migrate to ReactFlow (delete CanvasRenderer). Recommend: keep custom canvas (it works, 862 LOC invested).

**Effort**: S (2 hours) | **Benefit**: -240 LOC dead code; -1 npm dependency; smaller bundle.

### C.5 [P2] Add Missing shadcn/ui Primitives

**Problem** (subagent 12): Only 7 shadcn primitives exist. 9 UI patterns are hand-rolled: Tabs, Card, Select, Separator, Progress, ScrollArea, Tooltip, Toast, Skeleton.

**Fix**: Add via `npx shadcn@latest add`:
- `tabs` (for PolerPanel's 3-tab layout — currently hand-rolled)
- `card` (for metric cards in PolerPanel, Inspector, ReasoningDialog)
- `select` (for AI provider picker in AiSettingsDialog)
- `separator` (for Toolbar dividers)
- `tooltip` (for button hover hints)
- `sonner` (for toast notifications — useful for Layer G "Hypothesis generated" feedback)
- `scroll-area` (replace `.lit-scroll` custom CSS)

**Effort**: M (3–4 hours) | **Benefit**: Consistency; less custom CSS; better a11y.

### C.6 [P2] Wire Up Dark Mode Toggle

**Problem** (subagent 12): Dark mode CSS exists in `globals.css` (`@custom-variant dark` + `.dark` token block + `dark:*` utilities) but never toggled at runtime. `PolerPanel.tsx` hardcodes dark `slate-900` classes with zero `dark:` prefixes.

**Fix**:
1. Install `next-themes` (or a small custom `useTheme` hook).
2. Add a theme toggle button to Toolbar.
3. Refactor `PolerPanel.tsx` to use semantic tokens (`bg-background`, `bg-card`, `text-muted-foreground`) instead of hardcoded `slate-900`.

**Effort**: M (4–5 hours) | **Benefit**: Dark mode actually works; PolerPanel fits both themes.

### C.7 [P3] Rename PolerDialog → PolerClustersDialog

**Problem** (subagent 11): Two dialogs named "Poler*" — `PolerDialog.tsx` (legacy spectral k-means clustering) and `PolerPanel.tsx` (Layer F.2 visualizer). Confusing.

**Fix**: Rename `PolerDialog.tsx` → `PolerClustersDialog.tsx`, update imports in `Toolbar.tsx`.

**Effort**: S (30 min) | **Benefit**: Naming clarity.

---

## Part D — Layer G Implementation Plan

### D.1 Architecture Overview

Layer G is the **LLM Reasoning Bridge** — it consumes paradoxes from Layer E's `ParadoxDetector`, generates 4 canonical resolution hypotheses via an LLM, and validates the LLM's proposed text against the deterministic `WorldState` + `ConstraintEngine`.

**Data flow**:
```
ParadoxDetector (Layer E)
    ↓ Vec<Paradox>
LlmBridge::generate_hypotheses(paradoxes, provider)
    ↓ Vec<Hypothesis>
[User picks a hypothesis in PolerPanel UI]
    ↓ selected Hypothesis
LlmBridge::generate_resolution_text(hypothesis, provider)
    ↓ String (proposed chapter text)
Validator::validate(proposed_text, world_state, constraint_engine)
    ↓ ValidationResultDto (accept | reject | retry)
[If reject → feed back to LLM with feedback_prompt]
```

### D.2 File Plan

#### D.2.1 Rust: `litgraph-core/src/reasoning/llm_bridge.rs` (NEW, ~400 LOC)

```rust
use serde::{Deserialize, Serialize};
use crate::ai::{AiProvider, chat};
use crate::reasoning::{Paradox, NarrativeGraph, ConflictReport};
use crate::reasoning::paradox::ParadoxKind;

/// Canonical hypothesis kinds per POLER_UA_LP_MASTER_ROADMAP_V8.md §Layer G.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HypothesisKind {
    Flashback,           // спогад про померлого персонажа
    DreamSequence,       // сон або галюцинація
    UnrecordedResurrection, // сюжетне воскресіння (магія/медицина)
    DisguisedIdentity,   // самозванець / подвійник
}

/// A single LLM-generated hypothesis for resolving a paradox.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hypothesis {
    pub id: String,                    // UUID
    pub paradox_id: String,            // references Paradox.id (B.3 fix)
    pub kind: HypothesisKind,
    pub summary: String,               // 1-2 sentence summary
    pub proposed_text: Option<String>, // full proposed chapter text (None until generated)
    pub confidence: f64,               // LLM self-reported confidence [0,1]
    pub rationale: String,             // why this hypothesis fits
}

/// Result of validating LLM-proposed text against WorldState.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ValidationOutcome {
    Accept { violations: Vec<String>, paradoxes: Vec<String> },
    Reject { violations: Vec<String>, feedback_prompt: String },
    Retry { reason: String },
}

pub struct LlmBridge {
    provider: AiProvider,
    narrative_graph: NarrativeGraph,
}

impl LlmBridge {
    pub fn new(provider: AiProvider) -> Self { ... }

    /// Generate 4 canonical hypotheses for a single paradox.
    pub async fn generate_hypotheses(&self, paradox: &Paradox) -> Result<Vec<Hypothesis>, String> {
        let prompt = build_paradox_resolution_prompt(paradox, &self.narrative_graph);
        let response = chat(&self.provider, &prompt).await?;
        parse_hypotheses(&response, &paradox.id)
    }

    /// Generate full resolution text for a chosen hypothesis.
    pub async fn generate_resolution_text(&self, hypothesis: &Hypothesis) -> Result<Hypothesis, String> {
        let prompt = build_resolution_text_prompt(hypothesis);
        let response = chat(&self.provider, &prompt).await?;
        Ok(Hypothesis { proposed_text: Some(response), ..hypothesis.clone() })
    }

    /// Validate LLM-proposed text against WorldState + ConstraintEngine.
    pub fn validate(&self, proposed_text: &str, world_state: &WorldStateView) -> ValidationOutcome {
        // Reuse reasoning_validate_text logic from src-tauri/src/commands/reasoning.rs
    }
}
```

#### D.2.2 Rust: `litgraph-core/src/ai/prompts.rs` (EXTEND, +150 LOC)

Add two new prompt builders:
```rust
/// Build a prompt asking the LLM to generate 4 canonical hypotheses for a paradox.
pub fn build_paradox_resolution_prompt(paradox: &Paradox, graph: &NarrativeGraph) -> String {
    // Includes:
    // - Paradox kind, character, chapter indices, explanation
    // - Character state from NarrativeGraph (alive/dead, location)
    // - SVO triplets involving the character
    // - Instruction: "Generate 4 hypotheses: Flashback, Dream, Resurrection, Impostor"
    // - Output format: JSON with kind, summary, rationale, confidence
}

/// Build a prompt asking the LLM to generate full chapter text for a hypothesis.
pub fn build_resolution_text_prompt(hypothesis: &Hypothesis) -> String {
    // Includes:
    // - Hypothesis kind + summary + rationale
    // - Original paradox context
    // - Instruction: "Write a 500-1000 word chapter section that resolves this paradox"
    // - Constraint: "Must be consistent with the WorldState (no new paradoxes)"
}
```

#### D.2.3 Rust: `src-tauri/src/commands/llm_bridge.rs` (NEW, ~250 LOC)

Three new Tauri commands, following the `poler.rs` template:
```rust
#[tauri::command]
pub async fn cmd_generate_llm_hypotheses(
    paradox_id: String,
    manuscript_text: String,
    provider: AiProvider,
) -> Result<Vec<HypothesisDto>, String> { ... }

#[tauri::command]
pub async fn cmd_generate_resolution_text(
    hypothesis: HypothesisDto,
    manuscript_text: String,
    provider: AiProvider,
) -> Result<HypothesisDto, String> { ... }

#[tauri::command]
pub async fn cmd_validate_llm_response(
    proposed_text: String,
    manuscript_text: String,
) -> Result<ValidationOutcomeDto, String> { ... }
```

Register in `src-tauri/src/commands/mod.rs` and `src-tauri/src/lib.rs::generate_handler!`.

#### D.2.4 TS: `src/lib/tauri-commands.ts` (EXTEND, +120 LOC)

Mirror the Rust DTOs + add 3 wrappers:
```typescript
export interface HypothesisKind {
  // Discriminated union mirroring Rust enum
  kind: "flashback" | "dreamSequence" | "unrecordedResurrection" | "disguisedIdentity";
}

export interface HypothesisDto {
  id: string;
  paradoxId: string;
  kind: string;
  summary: string;
  proposedText: string | null;
  confidence: number;
  rationale: string;
}

export interface ValidationOutcomeDto {
  kind: "accept" | "reject" | "retry";
  violations?: string[];
  paradoxes?: string[];
  feedbackPrompt?: string;
  reason?: string;
}

export async function cmdGenerateLlmHypotheses(
  paradoxId: string,
  manuscriptText: string,
  provider: AiProviderConfig
): Promise<HypothesisDto[]> { ... }

export async function cmdGenerateResolutionText(
  hypothesis: HypothesisDto,
  manuscriptText: string,
  provider: AiProviderConfig
): Promise<HypothesisDto> { ... }

export async function cmdValidateLlmResponse(
  proposedText: string,
  manuscriptText: string
): Promise<ValidationOutcomeDto> { ... }
```

#### D.2.5 TS: `src/lib/llm-bridge/api.ts` (NEW, ~100 LOC)

A high-level API mirroring `nerBridge.ts`:
```typescript
export async function generateHypothesesForParadox(paradox: ParadoxDto): Promise<HypothesisDto[]> {
  const provider = useLitStore.getState().aiProviderConfig;
  return cmdGenerateLlmHypotheses(paradox.id /* B.3 fix */, collectedText, provider);
}

export async function generateResolution(hypothesis: HypothesisDto): Promise<HypothesisDto> {
  const provider = useLitStore.getState().aiProviderConfig;
  return cmdGenerateResolutionText(hypothesis, collectedText, provider);
}

export async function validateResolution(text: string): Promise<ValidationOutcomeDto> {
  return cmdValidateLlmResponse(text, collectedText);
}
```

#### D.2.6 React: Add 4th Tab to PolerPanel.tsx

Per subagent 10's recommendation (over subagent 11's ReasoningDialog proposal — PolerPanel is the Layer F visualizer, Layer G extends F naturally):

Add a 4th tab "🧪 LLM Hypotheses" to `PolerPanel.tsx`:
- Shows list of paradoxes (reuse from Paradox Feed tab).
- Each paradox has a "🧪 Hypothesize" button.
- Clicking generates 4 hypotheses via `cmdGenerateLlmHypotheses`.
- Each hypothesis card shows: kind, summary, rationale, confidence.
- "Generate Full Text" button → calls `cmdGenerateResolutionText`.
- "Validate" button → calls `cmdValidateLlmResponse`, shows accept/reject/retry badge.
- If reject, shows `feedbackPrompt` and offers "Regenerate with feedback" button.

**Effort**: L (8–10 hours) | **Benefit**: User-in-the-loop Layer G workflow.

### D.3 Layer G Test Plan

1. **Unit tests** (`litgraph-core/src/reasoning/llm_bridge.rs`):
   - `test_generate_hypotheses_returns_4_kinds` — mock `chat()` to return canned JSON, verify 4 hypotheses with distinct kinds.
   - `test_parse_hypotheses_handles_malformed_json` — verify graceful error.
   - `test_validate_accepts_consistent_text` — proposed text with no paradoxes → Accept.
   - `test_validate_rejects_dead_speaking` — proposed text where dead character speaks → Reject with feedback.

2. **Integration tests** (`src-tauri/src/commands/llm_bridge.rs`):
   - `test_cmd_generate_llm_hypotheses_smoke` — smoke test with mock provider.
   - `test_cmd_validate_llm_response_smoke` — smoke test.

3. **TS tests** (NEW — currently zero, see Part E.3):
   - `src/components/litgraph/PolerPanel.test.tsx` — render test for 4th tab.

---

## Part E — Architectural Debt Cleanup

### E.1 [P1] Eliminate litgraph-core ↔ src-tauri Duplication

**Finding** (subagents 06, 17): 14 byte-identical files (40,794 LOC) duplicated. 6 divergent files. 5 files only in litgraph-core. `src-tauri/Cargo.toml` already has `litgraph-core` as a path dependency. `src-tauri/src/poler/mod.rs` is a 39-line `pub use litgraph_core::*` shim proving the pattern works.

**Fix plan** (per subagent 06):
1. Backport 6 small tauri-only patches into litgraph-core:
   - `ParsedChapter.suffix` field
   - `Concept` / `Organization` LitNodeType variants (drift found by subagent 04)
   - Pronoun blacklist in `dict/cognate.rs` (drift found by subagent 01)
   - `Openaicompat` → `OpenAiCompat` rename (wire-format breaking — coordinate with B.1)
   - `ukrainian_semantic_categories` callsites
   - +968 LOC of LanguageTool rules
2. Replace 7 `mod` declarations in `src-tauri/src/lib.rs` with `pub use litgraph_core::*` shims.
3. Delete ~46,000 LOC of duplicated source from `src-tauri/src/{parser,linguistic,dict,ai,models,linguistic_entities,languagetool_weights,ukrainian_semantic_categories}.*`.

**Effort**: L (10–12 hours) | **Risk**: HIGH — wire-format breaking changes need careful coordination. **Mitigation**: Do this AFTER B.1 (AI provider fix) to avoid merge conflicts.

### E.2 [P1] Fix SpatialTeleportation Paradox Stub

**Finding** (subagents 02, 03): `ParadoxKind::SpatialTeleportation` is declared but never emitted. `locations.rs::lemmatize_simple` fails on short names (Рэй/Рэя don't merge). `count_in_text` lacks word-boundary checks. `ParsedLocation` has no per-chapter index, no character co-occurrences, no transit-verb field.

**Fix plan**:
1. Add `per_chapter_index: Vec<usize>` to `ParsedLocation`.
2. Add `character_co_occurrences: Vec<String>` to `ParsedLocation`.
3. Fix `lemmatize_simple` to handle short names (use morphological dictionary).
4. Add word-boundary regex to `count_in_text`.
5. Implement `ParadoxDetector::detect_spatial_teleportation` — character in location A at chapter N, location B at chapter M > N, no transit verb between.

**Effort**: L (8–10 hours) | **Blocks**: Layer G (spatial teleportation paradoxes need to be detectable before LLM can hypothesize)

### E.3 [P2] Add TypeScript Test Infrastructure

**Finding** (subagent 16): Zero TS tests. No test runner installed.

**Fix plan**:
1. Install Vitest: `bun add -D vitest @testing-library/react @testing-library/jest-dom jsdom`.
2. Add `vitest.config.ts`.
3. Add `test` script to `package.json`: `"test": "vitest run"`.
4. Write first tests:
   - `src/lib/tauri-commands.test.ts` — DTO type tests.
   - `src/components/litgraph/SvoHighlighter.test.tsx` — render test.
   - `src/components/litgraph/PolerPanel.test.tsx` — tab switching test.

**Effort**: M (4–6 hours) | **Benefit**: Catches regressions in Layer F.2 and Layer G.

### E.4 [P2] Add CI Quality Gates

**Finding** (subagent 16): `.github/workflows/release.yml` runs only on tag push, builds Linux-only, zero quality gates.

**Fix plan**:
1. Create `.github/workflows/ci.yml` triggered on push + PR:
   - `cargo test --workspace`
   - `cargo clippy -- -D warnings`
   - `cargo fmt --check`
   - `bun run build` (runs `tsc --noEmit && vite build`)
   - `bun run test` (after E.3)
2. Add `cargo audit` + `bun audit` for vulnerability scanning.
3. Add macOS + Windows build matrix (cross-platform Tauri).

**Effort**: M (3–4 hours) | **Benefit**: Catches regressions before merge.

### E.5 [P3] Fix SVO Parser Case-Frame Validation

**Finding** (subagent 01): `svo_templates.json.gz` (UD-Ukrainian-IU patterns) is loaded but only used for a +0.05 confidence bump. Case-frame validation (`allowed_subject_cases`, `allowed_object_cases`, `is_transitive`) is not wired up.

**Fix plan**: In `litgraph-core/src/linguistic/svo_parser.rs:254-258`, replace the +0.05 bump with full case-frame validation: reject triplets whose subject/object cases don't match the template.

**Effort**: M (4–5 hours) | **Benefit**: Higher SVO precision; fewer false-positive triplets in PolerPanel.

### E.6 [P3] Fix detect_characters Speech Verb Coverage

**Finding** (subagent 02): `SPEECH_VERBS` lexicon (~70 entries) contains only past-tense forms. Missing infinitives, future tense, imperatives. This caused the `db8abf3` test patches.

**Fix plan**: Expand `SPEECH_VERBS` to include infinitive (`сказати`, `відповісти`), future (`скаже`, `відповість`), imperative (`скажи!`, `ответь!`) forms.

**Effort**: S (2 hours) | **Benefit**: Present-tense prose ("Петро каже: ...") now detects characters.

---

## Part F — Documentation Alignment

### F.1 [P0] Create POLER_LAYER_G_IMPLEMENTATION_PLAN.md

**Finding** (subagents 13, 15): Layer G is specified in only 12 lines of prose in the master roadmap. No dedicated spec doc exists. Target file paths in the roadmap (`litgraph-core/src/reasoning/{llm_bridge,hypotheses}.rs`) are wrong — those files belong to Wave 5.

**Fix**: Create `POLER_LAYER_G_IMPLEMENTATION_PLAN.md` based on Part D of this document. Include:
- Concrete Rust struct signatures (from D.2.1)
- Prompt template bodies (from D.2.2)
- Tauri command signatures (from D.2.3)
- TS DTOs + wrappers (from D.2.4)
- React UI spec (from D.2.6)
- Test plan (from D.3)

**Effort**: M (3–4 hours) | **Benefit**: Layer G becomes implementable without ambiguity.

### F.2 [P1] Fix README + CHANGELOG Accuracy

**Finding** (subagent 15): README claims 47 shadcn/ui (actually 7), 11 litgraph components (actually 22), 15 Tauri commands (actually 29), non-existent `AiSettingsDialog.tsx`. CHANGELOG stops at 0.2.1; 17 commits including Layer F.2 undocumented.

**Fix**:
1. README: update component counts, remove `AiSettingsDialog.tsx` reference (until B.1 creates it), update feature list.
2. CHANGELOG: add 0.2.2 entry with Layer F.1 + F.2, 0.2.3 entry (planned) with Layer G + GUI refactoring.
3. Bump version to 0.2.3 after Layer G ships.

**Effort**: S (2 hours).

### F.3 [P1] Patch Layer F Spec DTOs + Formula

**Finding** (subagent 14): `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` §3.4 climax formula is mathematically WRONG (missing ln denominator, wrong coefficients, includes A_SVO that canonical forbids). Spec's TS DTOs (`totalParadoxes`, `chapterBreakdowns`, `frobeniusNorm`, `noiseFiltered`) don't match actual Rust DTOs (`paradoxes`, `chapters`, `omegaConf`, `isNoise`).

**Fix**:
1. Patch §3.4 to match canonical formula: `(κ · I_loc · d̄² + γ_emo · E + λ_conf · Ω_conf) / ln(e + |U|)`.
2. Patch §4 TS DTOs to match actual Rust DTOs (source of truth: `src-tauri/src/commands/poler.rs`).
3. Add a note: "When this spec disagrees with code, code is the source of truth."

**Effort**: S (1 hour).

### F.4 [P2] Update SECURITY.md

**Finding** (subagent 15): SECURITY.md is a GitHub-token how-to, not a security policy. Silent on CSP null, capabilities wildcards, Python subprocess.

**Fix**: Rewrite SECURITY.md to document:
- CSP policy (after B.2 fix)
- Capabilities scope (after B.2 fix)
- Python subprocess invocation pattern (no shell=True, temp-file protocol)
- Reporting vulnerabilities

**Effort**: S (1 hour).

---

## Part G — Concrete Task List (Dependency-Ordered)

### Phase 0: Critical Blockers (12–17 h, must complete before Layer G)

| # | Task | Deps | Effort | Files |
|---|------|------|--------|-------|
| G.0.1 | Migrate store.ts to Tauri Store plugin | — | S (2-3h) | `store.ts`, `tauri-store-adapter.ts` (new) |
| G.0.2 | Fix AI provider plumbing (create AiSettingsDialog) | G.0.1 | M (6-8h) | `AiSettingsDialog.tsx` (new), `AIDialog.tsx`, `AssistantDialog.tsx`, `store.ts` |
| G.0.3 | Fix CSP + capabilities | — | S (2-3h) | `tauri.conf.json`, `capabilities/default.json` |
| G.0.4 | Add `id` + `evidence_text` to Paradox struct | — | M (4-6h) | `litgraph-core/src/reasoning/paradox.rs`, `src-tauri/src/commands/poler.rs`, `tauri-commands.ts`, update tests |

### Phase 1: GUI Refactoring (10–14 h, parallel with Phase 0)

| # | Task | Deps | Effort | Files |
|---|------|------|--------|-------|
| G.1.1 | Extract DialogHost.tsx from Toolbar.tsx | — | M (4-5h) | `DialogHost.tsx` (new), `Toolbar.tsx`, `store.ts` |
| G.1.2 | Refactor PolerPanel.tsx to use shadcn Dialog | — | S (1-2h) | `PolerPanel.tsx` |
| G.1.3 | Extract useDerivedText hook | — | S (1h) | `hooks.ts` (new), `Toolbar.tsx` |
| G.1.4 | Remove dead @xyflow/react | — | S (2h) | `App.tsx`, `LitNodeView.tsx`, `LitEdgeView.tsx`, `package.json` |
| G.1.5 | Add missing shadcn primitives | — | M (3-4h) | `src/components/ui/*` |
| G.1.6 | Rename PolerDialog → PolerClustersDialog | — | S (0.5h) | rename + `Toolbar.tsx` |

### Phase 2: Architectural Debt (18–26 h, after Phase 0)

| # | Task | Deps | Effort | Files |
|---|------|------|--------|-------|
| G.2.1 | Eliminate litgraph-core ↔ src-tauri duplication | G.0.2 | L (10-12h) | `src-tauri/src/{parser,linguistic,dict,ai,models}/*`, `lib.rs` |
| G.2.2 | Fix SpatialTeleportation paradox stub | G.0.4 | L (8-10h) | `litgraph-core/src/parser/locations.rs`, `reasoning/paradox.rs` |
| G.2.3 | Add Vitest + first TS tests | — | M (4-6h) | `vitest.config.ts`, `*.test.ts(x)` |
| G.2.4 | Add CI quality gates | G.2.3 | M (3-4h) | `.github/workflows/ci.yml` |
| G.2.5 | Fix SVO parser case-frame validation | — | M (4-5h) | `svo_parser.rs` |
| G.2.6 | Expand SPEECH_VERBS lexicon | — | S (2h) | `litgraph-core/src/parser/characters.rs` |

### Phase 3: Layer G Implementation (20–28 h, after Phase 0 + Phase 1)

| # | Task | Deps | Effort | Files |
|---|------|------|--------|-------|
| G.3.1 | Create POLER_LAYER_G_IMPLEMENTATION_PLAN.md | — | M (3-4h) | `POLER_LAYER_G_IMPLEMENTATION_PLAN.md` (new) |
| G.3.2 | Implement `litgraph-core/src/reasoning/llm_bridge.rs` | G.0.4, G.3.1 | L (8-10h) | `llm_bridge.rs` (new), `reasoning/mod.rs` |
| G.3.3 | Extend `prompts.rs` with paradox resolution prompts | G.3.2 | M (4-5h) | `litgraph-core/src/ai/prompts.rs` |
| G.3.4 | Create `src-tauri/src/commands/llm_bridge.rs` | G.3.2 | M (4-5h) | `llm_bridge.rs` (new), `commands/mod.rs`, `lib.rs` |
| G.3.5 | Add TS DTOs + wrappers to `tauri-commands.ts` | G.3.4 | S (2h) | `tauri-commands.ts` |
| G.3.6 | Create `src/lib/llm-bridge/api.ts` | G.3.5 | S (2h) | `api.ts` (new) |
| G.3.7 | Add "🧪 LLM Hypotheses" tab to PolerPanel.tsx | G.3.6, G.1.2 | L (8-10h) | `PolerPanel.tsx` |
| G.3.8 | Layer G unit + integration tests | G.3.2, G.3.4 | M (4-6h) | `llm_bridge.rs` tests, `poler.rs` tests |

### Phase 4: Documentation (5–8 h, parallel with all phases)

| # | Task | Deps | Effort | Files |
|---|------|------|--------|-------|
| G.4.1 | Fix README + CHANGELOG accuracy | — | S (2h) | `README.md`, `CHANGELOG.md` |
| G.4.2 | Patch Layer F spec DTOs + formula | — | S (1h) | `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` |
| G.4.3 | Rewrite SECURITY.md | G.0.3 | S (1h) | `SECURITY.md` |
| G.4.4 | Wire up dark mode toggle | — | M (4-5h) | `useTheme` hook, `Toolbar.tsx`, `PolerPanel.tsx` |

### Total Estimated Effort

| Phase | Hours | Notes |
|-------|-------|-------|
| Phase 0 (blockers) | 12–17 | Must complete first |
| Phase 1 (GUI) | 10–14 | Parallel with Phase 0 |
| Phase 2 (debt) | 18–26 | After Phase 0 |
| Phase 3 (Layer G) | 20–28 | After Phase 0 + 1 |
| Phase 4 (docs) | 5–8 | Parallel |
| **Total** | **65–93** | **~4–6 weeks at 15 h/week** |

---

## Part H — Risk Analysis & Mitigations

### H.1 [HIGH] Wire-format breaking change in duplication refactor (G.2.1)

**Risk**: Backporting `Openaicompat` → `OpenAiCompat` rename changes the serde wire format. Existing AI invocations (after G.0.2) would break if the frontend still sends `Openaicompat`.

**Mitigation**: Coordinate G.0.2 and G.2.1 — both must ship in the same commit. Add `#[serde(alias = "Openaicompat")]` for backward compatibility.

### H.2 [HIGH] LLM non-determinism breaks tests (G.3.8)

**Risk**: Layer G tests that call real LLMs will be non-deterministic (different output each run).

**Mitigation**: Use a mock `AiProvider` in tests. Add a `MockProvider` variant to `AiProvider` enum that returns canned responses. Never call real LLMs in CI.

### H.3 [HIGH] Layer G prompt injection attacks

**Risk**: LLM-generated text rendered in PolerPanel could contain `<script>` tags or other injection vectors, especially with CSP null (B.2).

**Mitigation**: Fix B.2 (CSP) BEFORE Layer G ships. Render LLM output via React's default JSX escaping (never `dangerouslySetInnerHTML`). Sanitize any HTML in LLM output with `DOMPurify` if rich text is needed.

### H.4 [MEDIUM] SpatialTeleportation false positives (G.2.2)

**Risk**: Aggressive location normalization could merge distinct locations (e.g., "Ліс" the forest and "Ліс" the surname).

**Mitigation**: Require character co-occurrence + transit verb absence before flagging. Add a confidence score; only show paradoxes with confidence ≥ 0.7.

### H.5 [MEDIUM] Tauri Store plugin migration breaks existing projects (G.0.1)

**Risk**: Users with existing localStorage-saved projects might lose data on upgrade.

**Mitigation**: Add a one-time migration: on first launch after upgrade, read all localStorage keys and write them to Tauri Store, then clear localStorage. Show a toast: "Migrated N projects to Tauri Store."

### H.6 [MEDIUM] PolerPanel 4th tab complexity (G.3.7)

**Risk**: Adding LLM Hypotheses tab makes PolerPanel.tsx exceed 800 LOC, becoming hard to maintain.

**Mitigation**: Extract tab content into separate components: `PolerPanel.tsx` (shell + tabs), `EpsilonHeatmapTab.tsx`, `SvoInspectorTab.tsx`, `ParadoxFeedTab.tsx`, `LlmHypothesesTab.tsx`.

### H.7 [LOW] CI quality gates block development (G.2.4)

**Risk**: Adding strict `clippy -D warnings` + `tsc --noEmit` to CI might fail on existing code.

**Mitigation**: First pass: add CI as non-blocking (warnings only). Fix warnings over a week. Second pass: make CI blocking.

### H.8 [LOW] Dark mode toggle breaks PolerPanel styling (G.4.4)

**Risk**: Refactoring PolerPanel from hardcoded `slate-900` to semantic tokens might introduce visual regressions.

**Mitigation**: Add visual regression tests (Storybook + Chromatic, or Playwright screenshots) before the refactor. Test both themes manually after refactor.

---

## Part I — Verification Checklist

Before declaring Layer G complete, verify ALL of the following:

### Phase 0 Verification
- [ ] `AiSettingsDialog.tsx` opens from Toolbar "⚙ AI Settings" button
- [ ] Provider config persists across app restarts (Tauri Store, not localStorage)
- [ ] `AIDialog.tsx` and `AssistantDialog.tsx` successfully invoke AI commands (no `missing field provider`)
- [ ] `tauri.conf.json` CSP is set to restrictive policy
- [ ] `capabilities/default.json` has no `**` wildcards
- [ ] `Paradox` struct has `id: String` and `evidence_text: Vec<String>` fields
- [ ] All 289 src-tauri tests still PASS after Paradox struct change

### Phase 1 Verification
- [ ] `Toolbar.tsx` is under 500 LOC
- [ ] `DialogHost.tsx` renders all 11 dialogs correctly
- [ ] `PolerPanel.tsx` uses shadcn `<Dialog>` (focus trap works, Escape closes, scroll locked)
- [ ] `useDerivedText()` hook is used in Toolbar and at least one other component
- [ ] `@xyflow/react` is uninstalled, `bun run build` succeeds
- [ ] 7 new shadcn primitives added (tabs, card, select, separator, tooltip, sonner, scroll-area)
- [ ] `PolerClustersDialog.tsx` (renamed from PolerDialog) works

### Phase 2 Verification
- [ ] `diff -r litgraph-core/src src-tauri/src` shows zero identical files
- [ ] `src-tauri/src/lib.rs` has 7 `pub use litgraph_core::*` shims
- [ ] `cargo test --workspace` — 413+ tests PASS
- [ ] `ParadoxDetector::detect_spatial_teleportation` emits paradoxes on test fixture with teleportation
- [ ] `vitest run` — at least 5 TS tests PASS
- [ ] `.github/workflows/ci.yml` runs on PR, executes cargo test + clippy + tsc + vitest
- [ ] SVO parser rejects triplets with wrong case frames
- [ ] `detect_characters` finds characters in present-tense prose ("Петро каже: ...")

### Phase 3 Verification (Layer G)
- [ ] `POLER_LAYER_G_IMPLEMENTATION_PLAN.md` exists, 500+ LOC
- [ ] `litgraph-core/src/reasoning/llm_bridge.rs` compiles, exports `LlmBridge`, `Hypothesis`, `HypothesisKind`, `ValidationOutcome`
- [ ] `litgraph-core/src/ai/prompts.rs` has `build_paradox_resolution_prompt` + `build_resolution_text_prompt`
- [ ] `src-tauri/src/commands/llm_bridge.rs` registers 3 commands: `cmd_generate_llm_hypotheses`, `cmd_generate_resolution_text`, `cmd_validate_llm_response`
- [ ] `src/lib/tauri-commands.ts` has matching DTOs + wrappers
- [ ] `src/lib/llm-bridge/api.ts` exports `generateHypothesesForParadox`, `generateResolution`, `validateResolution`
- [ ] PolerPanel "🧪 LLM Hypotheses" tab:
  - Shows paradox list
  - "🧪 Hypothesize" button generates 4 hypotheses
  - Each hypothesis card shows kind, summary, rationale, confidence
  - "Generate Full Text" produces resolution text
  - "Validate" shows accept/reject/retry badge
  - Reject → shows feedback prompt → "Regenerate" works
- [ ] Layer G unit tests PASS with mock provider
- [ ] `cargo test --workspace` — 430+ tests PASS (413 + ~17 new Layer G tests)

### Phase 4 Verification
- [ ] README.md component counts match reality
- [ ] CHANGELOG.md has 0.2.3 entry with Layer G
- [ ] `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md` §3.4 formula matches canonical
- [ ] SECURITY.md documents CSP, capabilities, Python subprocess
- [ ] Dark mode toggle button in Toolbar works
- [ ] PolerPanel renders correctly in both light and dark themes

---

## Part J — Decision Points Requiring User Input

Before execution, the user must decide:

1. **Phase ordering**: Execute Phase 0 → 1 → 2 → 3 → 4 sequentially, OR parallelize Phase 1 + Phase 4 with Phase 0?
   - **Recommendation**: Parallelize Phase 1 (GUI refactoring is independent of Phase 0 blockers).

2. **@xyflow/react decision** (G.1.4): Delete the dead ReactFlow imports entirely, OR migrate from custom CanvasRenderer to ReactFlow?
   - **Recommendation**: Delete — 862 LOC invested in CanvasRenderer, it works, no need to migrate.

3. **Layer G UI placement** (G.3.7): 4th tab in PolerPanel (subagent 10 recommendation) OR new tab in ReasoningDialog (subagent 11 recommendation)?
   - **Recommendation**: 4th tab in PolerPanel — Layer G extends Layer F naturally, and PolerPanel already has the paradox list.

4. **Duplication refactor scope** (G.2.1): Do the full 46,000 LOC elimination in one PR, OR split into 7 smaller PRs (one per module)?
   - **Recommendation**: Split — 7 smaller PRs are easier to review and less likely to break.

5. **Version bump**: Bump to 0.2.3 after Layer G, OR 0.3.0 (minor version bump signaling new feature)?
   - **Recommendation**: 0.3.0 — Layer G is a significant feature addition.

6. **Mock provider for tests** (G.3.8): Add `MockProvider` variant to `AiProvider` enum, OR use a trait-based approach with `dyn AiProviderTrait`?
   - **Recommendation**: Trait-based — more Rust-idiomatic, allows multiple mock implementations, doesn't pollute the production enum.

---

## Part K — References

- **Meta-prompt**: `docs/layer-g-planning/00-META_PROMPT.md`
- **17 subagent reports**: `/home/z/my-project/subagent-reports/01-*.md` through `17-*.md` (5,879 LOC total)
- **Shared worklog**: `/home/z/my-project/worklog.md` (17 new entries appended)
- **Source commit**: `c143a7f` on `feature/symbolic-ua-lp-engine`
- **Spec docs**: `POLER_UA_LP_MASTER_ROADMAP_V8.md`, `POLER_EPSILON_CANONICAL_SPECIFICATION.md`, `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md`, `POLER_V7_5_AUDIT_AND_CORRECTION_PLAN.md`, `POLER_LAYER_B_C_IMPLEMENTATION_PLAN.md`

---

**END OF DETAILED WORK PROMPT**

_Awaiting user approval. Once approved, execution begins with Phase 0 (Critical Blockers)._
