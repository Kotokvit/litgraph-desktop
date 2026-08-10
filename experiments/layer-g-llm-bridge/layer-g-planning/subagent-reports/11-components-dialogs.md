# Subagent 11 — Dialog Components Audit (`src/components/litgraph/`)

**Task ID:** 11-components-dialogs
**Agent:** Explore (very thorough)
**Scope:** All dialog components in `src/components/litgraph/`
**Date:** 2026-08-08
**Repository path:** `/home/z/my-project/litgraph-desktop/`

---

## 1. File Inventory (10 files inspected)

| # | File | LOC | Modal primitive | Layer | IPC backend |
|---|------|----:|-----------------|-------|-------------|
| 1 | `AIDialog.tsx` | 291 | shadcn `<Dialog>` | Wave 3 | Tauri `ai_continue_chapter` / `ai_analyze_plot` |
| 2 | `AssistantDialog.tsx` | 251 | shadcn `<Dialog>` | Wave 3 | Tauri `ai_assistant` |
| 3 | `PolerDialog.tsx` (legacy) | 384 | shadcn `<Dialog>` | Wave 2 | **client-only** (`@/lib/poler/analyze`) |
| 4 | `PolerPanel.tsx` (new F.2) | 785 | **custom `fixed inset-0 z-50`** | Layer F.2 | Tauri `cmdComputeEpsilonClimax` / `cmdExtractSvo` / `cmdDetectParadoxes` |
| 5 | `SvoHighlighter.tsx` | 208 | *(not a dialog — presentational)* | Layer F.2 | none (consumes SVO DTOs) |
| 6 | `NerDialog.tsx` | 319 | shadcn `<Dialog>` | Wave 2 | Tauri `extractEntities` via `@/lib/poler/nerBridge` |
| 7 | `CharacterGraphDialog.tsx` | 477 | shadcn `<Dialog>` | Wave 2 | `analyzeCharacters` (Python+spaCy via Tauri sidecar) |
| 8 | `ConflictGraphDialog.tsx` | 783 | shadcn `<Dialog>` | Wave 4 | `getConflictGraph` (`@/lib/conflict/api`) |
| 9 | `ReasoningDialog.tsx` | 555 | shadcn `<Dialog>` | Wave 5 | `reasoningExtractEvents` / `reasoningGetWorldState` / `reasoningRunCycle` |
| 10 | `ReaderDialog.tsx` | 408 | shadcn `<Dialog>` (fullscreen variant) | Wave 4 | none (uses zustand store + `@/lib/poler/readerRender`) |
| 11 | `TextMomentsDialog.tsx` | 423 | shadcn `<Dialog>` | Wave 4 | none (client-side `findTextMoments`) |

**Total:** ~4 884 LOC across 11 files (one of which, `SvoHighlighter.tsx`, is a child component, not a dialog).

---

## 2. Dialog Consistency Audit

### 2.1 Modal primitive matrix

| Component | Uses shadcn `<Dialog>` / `<DialogContent>` | Custom modal markup | Notes |
|-----------|:---:|:---:|---|
| `AIDialog` | ✅ | — | standard |
| `AssistantDialog` | ✅ | — | standard |
| `PolerDialog` (legacy) | ✅ | — | standard |
| `NerDialog` | ✅ | — | standard |
| `CharacterGraphDialog` | ✅ | — | standard |
| `ConflictGraphDialog` | ✅ | — | standard |
| `ReasoningDialog` | ✅ | — | wrapped in internal `ErrorBoundary` |
| `ReaderDialog` | ✅ | — | fullscreen variant: `max-w-[100vw] w-screen h-screen`, `showCloseButton={false}`, custom header |
| `TextMomentsDialog` | ✅ | — | standard |
| **`PolerPanel` (new F.2)** | ❌ | ✅ **`fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm`** | rolls its own modal — see §2.2 |

### 2.2 PolerPanel custom modal — verified inconsistency

`PolerPanel.tsx:196-202`:

```tsx
return (
  <div
    className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4"
    role="dialog"
    aria-modal="true"
    aria-labelledby="poler-panel-title"
  >
    <div className="bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl w-full max-w-5xl h-[85vh] flex flex-col overflow-hidden">
```

**Implications:**
- No Radix `DialogPrimitive` → no built-in focus trap, no `Escape`-close via Radix (panel registers its own `keydown` listener at L175-182), no scroll-lock on `<body>`.
- `aria-modal` + `role="dialog"` are set manually — accessibility is *partially* correct, but Radix also manages `aria-labelledby`/`describedby` wiring automatically; here it's hand-rolled.
- Visual style is dark slate (`bg-slate-900`), diverging from every other dialog's light theme (`bg-white` from shadcn default).
- Props use `isOpen`/`onClose` (camelCase), not `open`/`onClose` like the other 9 dialogs — naming inconsistency.

**Recommendation:** Either
1. **Refactor `PolerPanel` to use shadcn `<Dialog>` + `<DialogContent className="max-w-5xl h-[85vh] bg-slate-900 text-slate-100">`** — keeps dark theme but inherits focus-trap, scroll-lock, Escape handling, and consistent props; or
2. Document `PolerPanel` as an intentional exception (it's a "Power Panel" with three tabs) and extract a reusable `DarkModalShell` primitive if more such panels are planned.

Option (1) is preferred — minimal code churn, max UX consistency.

---

## 3. AI Provider Plumbing — CONFIRMED BROKEN

### 3.1 Backend contract (Rust)

`src-tauri/src/commands/ai.rs:11-50`:

```rust
#[tauri::command]
pub async fn ai_continue_chapter(provider: AiProvider, /* … */) -> Result<…> { … }
#[tauri::command]
pub async fn ai_analyze_plot(provider: AiProvider, /* … */) -> Result<…> { … }
#[tauri::command]
pub async fn ai_assistant(provider: AiProvider, /* … */) -> Result<…> { … }
```

`AiProvider` is a Rust enum with three variants (`Ollama`, `OpenAiCompat`, `Zai`) — each requires fields (url/model/endpoint/api_key). Tauri's `invoke` deserializes arguments via `serde_json::from_value`, so a **missing `provider` field fails the call before it reaches the handler** with a "missing field `provider`" error.

### 3.2 Frontend — exact lines that omit `provider`

**`AIDialog.tsx:69-77`** — `runAI()` builds payload without `provider`:

```tsx
const payload: Record<string, unknown> = { project };
if (mode === "continue-chapter") {
  if (selectedNodeId) payload.fromChapterId = selectedNodeId;
  if (customPrompt.trim()) payload.customPrompt = customPrompt.trim();
} else {
  payload.focus = focus;
}
const text = await callApi<string>(cmdName, endpoint, payload);  // ← no provider
```

**`AssistantDialog.tsx:71-77`** — `send()` builds payload without `provider`:

```tsx
const text = await callApi<string>("ai_assistant", "/api/ai/assistant", {
  project,
  message: content,
  history: messages.map((m) => ({ role: m.role, content: m.content })),
  selectedNodeId,
});  // ← no provider
```

### 3.3 No provider state anywhere on the frontend

- `src/lib/litgraph/store.ts` — grep for `provider|aiProvider|ai_provider` returns **zero matches**.
- `src/lib/litgraph/types.ts` — grep for `AiProvider|provider` returns **zero matches**.
- No `AiSettingsDialog.tsx` exists (despite README claiming it does — see worklog task #1).
- `src/lib/litgraph/api.ts` `callApi()` is a generic passthrough; it does NOT inject a provider.

### 3.4 Web-preview parity

In web-preview mode `callApi()` falls back to `fetch(webEndpoint, …)` with the same `payload` JSON — meaning the Next.js API routes `/api/ai/continue-chapter`, `/api/ai/analyze-plot`, `/api/ai/assistant` will also fail with `provider: undefined` on the server. This is consistent breakage across both transports.

### 3.5 Fix required (out of scope for this audit, but flagged for Layer G)

Add to `store.ts`:

```ts
aiProvider: AiProvider | null;          // { kind: 'ollama'|'openai'|'zai', url?, apiKey?, model? }
setAiProvider: (p: AiProvider) => void; // persisted to localStorage
```

Then both dialogs inject `provider: aiProvider` into `callApi` payloads. A new `AiSettingsDialog` (or a `<AiProviderPicker>` embedded in `AIDialog` / `AssistantDialog` when `aiProvider === null`) is needed. This is a prerequisite for **Layer G: LLM-Bridge**.

---

## 4. PolerDialog (legacy) vs PolerPanel (new F.2) — Naming Collision

### 4.1 PolerDialog.tsx — legacy spectral k-modes clustering visualizer

- **Engine:** `@/lib/poler/analyze.ts` → `analyzeText()` — **100% client-side TypeScript** port of the original POLER spectral operator.
- **No IPC, no Tauri, no Python.** Truncates input to 50k chars.
- **Output:** Word co-occurrence graph → dissipative dynamics `dp/dt = -η·Π_Λ·[L·p + γ·J·p - B·p/m]` → k-modes clustering via eigenvectors.
- **UI:** gamma / k / window / minFreq sliders; renders cluster chips colored by tab10 palette; spectral bar chart.
- **Opened from:** `Toolbar.tsx:911-915` (`polerOpen` state).

### 4.2 PolerPanel.tsx — new Layer F.2 POLER Engine Ψ visualizer

- **Engine:** Tauri IPC → `src-tauri/src/commands/poler.rs` (Layers A–E Rust pipeline, `litgraph-core`).
- **Three parallel IPC calls on open:** `cmdComputeEpsilonClimax`, `cmdExtractSvo`, `cmdDetectParadoxes`.
- **Tabs:**
  1. **ε-Climax Heatmap** — ε value, Ω_conf Frobenius norm, spectral radius ρ(A), CLIMAX/NOISE/NORMAL badge, SVO-highlighted chapter reader (uses `<SvoHighlighter>`).
  2. **SVO Inspector** — table of triplets with actor/verb/target/instrument/location/polarity/confidence, filter All/Affirmative/Negated.
  3. **Paradox Feed** — dead_speaking + spatial_teleportation paradoxes with chapter provenance + per-chapter breakdown.
- **Ukrainian-localized**, dark slate theme, "UA-LP v7.5-LEM" badge.
- **Opened from:** `Toolbar.tsx:942-948` (`polerPanelOpen` state).
- **Web-preview fallback:** shows friendly notice if `window.__TAURI_INTERNALS__` absent.

### 4.3 Naming recommendation

Two components both called `Poler*` will confuse users and maintainers. They are **fundamentally different engines**:

| Aspect | PolerDialog (legacy) | PolerPanel (new) |
|---|---|---|
| Backend | TS client (`polerCore`) | Rust (`litgraph-core`) |
| Math | Spectral k-modes on word co-occurrence | ε-Climax scalar + SVO + paradox detection |
| Purpose | Topic discovery (clusters of words) | Climax/SVO/paradox forensics on a chapter |
| Language | Russian UI | Ukrainian UI |
| Theme | Light (shadcn default) | Dark slate |

**Recommendation:** Rename `PolerDialog.tsx` → `PolerClustersDialog.tsx` (or `WordClustersDialog.tsx`). Keep `PolerPanel.tsx` as the canonical "POLER" surface since it's the active Layer F.2 surface and the spec doc references it by that name. Update `Toolbar.tsx` import and the `polerOpen` state name to `polerClustersOpen`.

---

## 5. ReasoningDialog vs PolerPanel's "Paradox Feed" Tab — Overlap Analysis

### 5.1 ReasoningDialog (Wave 5) — what it currently shows

Pipeline (3 sequential IPC calls, `ReasoningDialog.tsx:258-288`):

1. `reasoningExtractEvents(text, project)` → `Event[]` (SVO regex parser on Rust side)
2. `reasoningGetWorldState(project, events)` → `WorldStateView` (characters with alive/location/attributes)
3. `reasoningRunCycle(project, events)` → `CycleReport` (inference + constraints + paradoxes + hypotheses)

**Rendered sections (after run):**
- **Metric row:** events processed, facts asserted, violations, temporal paradoxes, hypotheses accepted/generated.
- **Temporal paradoxes list** (`report.temporalParadoxes`) — red cards with `description` only.
- **Constraint violations** (`report.violations`) — amber cards with raw JSON dump.
- **Character state grid** (sorted: dead first, then alphabetical) — 💚/💀/❓ with attributes.
- **Top 30 events by confidence** — actor.action→target + chapter time + source text snippet.
- **Empty-state hints** + "narrative consistent" success card.

Footer: `Reasoning Engine v0.1 · stateless pipeline · без LLM`.

### 5.2 PolerPanel "Paradox Feed" tab — what it shows

- Detects paradoxes via `cmdDetectParadoxes(fullManuscriptText)` — Layer E `ParadoxDetector` in Rust.
- Paradox kinds: `dead_speaking`, `spatial_teleportation`.
- Renders: paradox count, total characters, total triplets, **per-chapter breakdown** (characterCount + tripletCount per chapter), and paradox cards with `kind` badge, character name, explanation, **origin chapter + manifest chapter** indices.

### 5.3 Overlap matrix

| Concern | ReasoningDialog | PolerPanel Paradox Feed |
|---|---|---|
| Source of paradoxes | `CycleReport.temporalParadoxes` from `reasoningRunCycle` (Wave 5) | `ParadoxReportDto.paradoxes` from `cmdDetectParadoxes` (Layer E, F.1) |
| Paradox schema | `{ description: string }` (single field — opaque) | `{ kind, character, explanation, originChapterIdx, chapterIdx }` (structured) |
| Paradox kinds supported | unspecified (whatever the reasoning engine emits) | `dead_speaking`, `spatial_teleportation` explicitly |
| Per-chapter breakdown | ❌ no | ✅ yes |
| Character origin/manifest provenance | ❌ no | ✅ yes |
| Plus character state | ✅ yes (alive/location/attributes) | ❌ no |
| Plus events list | ✅ yes (top 30 by confidence) | ❌ no |
| Plus constraint violations | ✅ yes | ❌ no |
| Plus hypotheses | ✅ yes (count only, no detail) | ❌ no |

### 5.4 Verdict

The two surfaces **do overlap on paradox detection** but use **different backends** with **different schemas**. PolerPanel's Paradox Feed is **richer in provenance** (origin/manifest chapter indices, per-chapter breakdown); ReasoningDialog's paradox section is **simpler** (just description) but bundled with character/event/hypothesis state.

**Recommendation:** Do NOT merge them yet. Treat them as two views on overlapping-but-distinct concerns:
- PolerPanel → "forensic microscope" on a single chapter's paradoxes (with cross-chapter manuscript context).
- ReasoningDialog → "narrative consistency dashboard" combining paradoxes + character state + events + hypotheses.

For Layer G, consider **unifying the paradox backend** behind `cmdDetectParadoxes` (Layer E) and have ReasoningDialog consume `ParadoxReportDto` instead of `CycleReport.temporalParadoxes`. That would eliminate the schema divergence and let both surfaces render the same structured data. This is a backend refactor, not a dialog change.

---

## 6. Layer G UI Placement — "Generate LLM Hypotheses"

### 6.1 Three options considered

**Option A: 4th tab in PolerPanel** ("LLM Hypotheses")
- Pros: PolerPanel already has the paradox list (Layer E output) — hypotheses naturally *explain* those paradoxes (the original Wave 5 reasoning engine already generates flashback/dream/text-error hypotheses). Same surface, same dataset, single user mental model.
- Cons: PolerPanel is currently Ukrainian + dark themed + chapter-scoped (ε/SVO use `chapterText`, only paradox uses `fullManuscriptText`). Hypotheses might need project-scope context (all chapters), breaking the chapter-focused framing. Also: PolerPanel has no LLM provider plumbing today (see §3), so adding LLM here means introducing the provider picker into a previously pure-symbolic surface.

**Option B: New tab in ReasoningDialog** ("LLM Hypotheses")
- Pros: ReasoningDialog is *already* the "narrative consistency dashboard" with `hypothesesAccepted/hypothesesGenerated` in its metric row — adding a tab to **view+generate** LLM hypotheses fits the existing taxonomy perfectly. The reasoning engine already generates 3 rule-based hypotheses (flashback/dream/text-error) — LLM hypotheses are a natural extension ("AI proposes additional explanations"). Project-scope is the default. Light theme, Russian UI — same as AIDialog/AssistantDialog.
- Cons: ReasoningDialog is already wide (`max-w-6xl`) and busy. Adding LLM generation may bloat it.

**Option C: New dedicated `LlmBridgeDialog`**
- Pros: Clean separation; can be invoked from multiple places (Toolbar button, "Send to LLM" action on a paradox card, etc.). Future LLM features (e.g., "Explain this character's arc", "Generate missing scene") can live here.
- Cons: Yet another modal in a UI that already has 9. Users have to discover it.

### 6.2 Recommendation: **Option B — add a 4th tab to ReasoningDialog**

**Rationale:**

1. **Conceptual fit:** ReasoningDialog is the only surface where "hypotheses" already appear in the metric row. Users will look for LLM hypotheses there. The current rule-based hypotheses (flashback/dream/text-error) and the new LLM-generated ones can be displayed side-by-side in a "Hypotheses" tab, color-coded by source (rule-based = green, LLM = blue).

2. **Backend reuse:** `reasoningRunCycle` already returns `hypothesesGenerated`/`hypothesesAccepted`. Layer G should *extend* the reasoning engine to optionally call an LLM (via the same `AiProvider` plumbing fixed in §3) for additional hypotheses, then return them in the same `CycleReport`. No new IPC command needed — just an optional `llmProvider?: AiProvider` parameter on `reasoningRunCycle`.

3. **Provider picker co-location:** The AI provider settings UI (once added per §3.5) can be a single shared component used by `AIDialog`, `AssistantDialog`, and the new ReasoningDialog tab — three entry points, one configuration source. Putting LLM hypotheses in PolerPanel (Option A) would force PolerPanel to also host the provider picker, breaking its "pure symbolic engine" framing.

4. **Avoids dialog sprawl:** Option C would create an 11th dialog without clear gain. The reasoning engine already owns the "hypotheses" concept — a new dialog would split that ownership.

5. **Project scope fits:** ReasoningDialog already runs on the full project (`reasoningExtractEvents(text, project)` — note the `project` argument). PolerPanel is chapter-scoped except for paradox detection. LLM hypotheses need full-project context.

### 6.3 Concrete UX for the new tab

```
┌─ ReasoningDialog ─────────────────────────────────────────────────┐
│ [Events] [Characters] [Paradoxes] [Violations] [Hypotheses★]      │
│                                                                   │
│ Hypotheses (3 rule-based + 2 LLM)                                 │
│ ┌─────────────────────────────────────────────────────────────┐  │
│ │ 🟢 [rule] flashback — "Пётр умер в Г12, говорит в Г15"       │  │
│ │    accepted · verified by constraint: flashback_marker(G15)  │  │
│ ├─────────────────────────────────────────────────────────────┤  │
│ │ 🔵 [LLM:gpt-4] dream sequence — "Г15 — сон Анны, см. намёк   │  │
│ │    в начале главы" · confidence 0.78 · [Accept] [Reject]    │  │
│ ├─────────────────────────────────────────────────────────────┤  │
│ │ ⚪ [LLM:gpt-4] text-error — "возможна опечатка, Пётр→Павел"   │  │
│ │    rejected by user · 2026-08-08 14:32                       │  │
│ └─────────────────────────────────────────────────────────────┘  │
│                                                                   │
│ [Generate LLM Hypotheses]  ← opens AiProviderPicker if not set    │
└───────────────────────────────────────────────────────────────────┘
```

### 6.4 Out of scope (but prerequisite)

- **Fix the AI provider plumbing (§3) first.** Without `AiProvider` flowing from frontend → Tauri → `ai::chat`, no LLM hypothesis generation can work.
- **Add `llm_hypotheses` field to `CycleReport` DTO** in `src-tauri/src/commands/reasoning.rs` (or wherever Wave 5's `CycleReport` lives).
- **Add an optional `provider: Option<AiProvider>` arg** to `reasoningRunCycle` so the Rust side can call the LLM only when configured.

---

## 7. Other Findings

### 7.1 ErrorBoundary pattern — only in ReasoningDialog

`ReasoningDialog.tsx:40-75` wraps the inner dialog in a class-based `ErrorBoundary` that catches render crashes (e.g., unexpected `Action` variant from Rust). This is **good defensive engineering** given the heterogeneity of the reasoning DTOs.

**Recommendation:** Consider extracting `ErrorBoundary` into `@/components/ui/error-boundary.tsx` and wrapping every dialog that consumes Rust DTOs with non-trivial variants (`ReasoningDialog`, `PolerPanel`, `ConflictGraphDialog`). The cost is ~5 LOC per dialog; the payoff is no white-screen on schema drift.

### 7.2 MetricBox — duplicated 5 times

`MetricBox` is defined locally in: `PolerDialog.tsx:364`, `NerDialog.tsx:302`, `CharacterGraphDialog.tsx:467`, `TextMomentsDialog.tsx:402`, `ReasoningDialog.tsx:125`. All five have near-identical shape (`{label, value, color, hint?}`) with slight styling variance.

**Recommendation:** Extract to `@/components/ui/metric-box.tsx`. Removes ~80 LOC of duplication and ensures consistent styling.

### 7.3 ReaderDialog — fullscreen Dialog variant

`ReaderDialog.tsx:219-223` uses shadcn `<Dialog>` but overrides `DialogContent` to be fullscreen (`max-w-[100vw] w-screen h-screen max-h-[100vh] rounded-none p-0`) and disables the close button (`showCloseButton={false}`). Custom header with prev/next moment navigation, ToC sidebar, and inline `<style>` block for `.reader-content` styles.

This is a **legitimate special case** — fullscreen reader is its own UX. Document it as such.

### 7.4 SvoHighlighter — presentational component, not a dialog

`SvoHighlighter.tsx` is consumed by `PolerPanel.tsx:518` (in the ε-Climax Heatmap tab). It is **not** a dialog and should not be confused with one. It does:
- Tokenize text by whitespace + punctuation (single regex split).
- Build three `Map<string, SvoTripletDto>` lookups (actor/verb/target, lowercased).
- Render each token as a colored span: violet pill (actor), amber bold (verb, with strike-through if negated), cyan underline (target).
- Click on a token fires `onTripletSelect(triplet)` — used to focus a row in the SVO Inspector tab.

Performance: O(N) in text length per render, memoized on `[text, triplets]`. Good.

### 7.5 Toolbar wiring — all 10 dialogs render unconditionally

`Toolbar.tsx:900-948` renders all dialogs at the top level with `open={…}` props. None are lazy-loaded. For a desktop app this is fine; for the web preview, consider `React.lazy()` + `Suspense` for `ConflictGraphDialog` (783 LOC, imports `html-to-image` + `jspdf` for PNG/PDF export) and `PolerPanel` (785 LOC) — they'd shave ~50KB off the initial bundle.

### 7.6 PolerPanel Escape handling — manual

`PolerPanel.tsx:175-182` registers its own `keydown` listener for `Escape` because it doesn't use shadcn `<Dialog>` (which would handle this via Radix). If PolerPanel is refactored per §2.2 recommendation, this manual handler can be removed.

### 7.7 Language inconsistency

| Dialog | UI language |
|---|---|
| AIDialog | Russian |
| AssistantDialog | Russian |
| PolerDialog (legacy) | Russian |
| NerDialog | Russian |
| CharacterGraphDialog | Russian |
| ConflictGraphDialog | Russian |
| ReasoningDialog | Russian |
| ReaderDialog | Russian |
| TextMomentsDialog | Russian |
| **PolerPanel (new)** | **Ukrainian** |

PolerPanel is the only Ukrainian-localized surface. The Layer F.2 spec apparently mandates Ukrainian ("UA-LP v7.5-LEM"). Decide whether the rest of the app should follow (large effort) or whether PolerPanel should be re-localized to Russian for consistency (small effort, regresses Layer F.2 intent).

---

## 8. Atomic Recommendations Summary

| # | Recommendation | Effort | Priority | Layer |
|---|---|---|---|---|
| R1 | Refactor `PolerPanel` to use shadcn `<Dialog>` (keep dark theme via className) | S | High | F.2 polish |
| R2 | Rename `PolerDialog` → `PolerClustersDialog` (and `polerOpen` → `polerClustersOpen`) | S | High | Naming clarity |
| R3 | Add `AiProvider` to zustand store + persist to localStorage; inject in `AIDialog`/`AssistantDialog` payloads | M | **Blocker for Layer G** | Prereq |
| R4 | Build `AiSettingsDialog` (or embed provider picker in the two AI dialogs when `aiProvider === null`) | M | **Blocker for Layer G** | Prereq |
| R5 | Add 4th tab "LLM Hypotheses" to `ReasoningDialog` (Option B); extend `reasoningRunCycle` with optional `provider` | L | High | **Layer G** |
| R6 | Extract shared `MetricBox` to `@/components/ui/metric-box.tsx` | S | Medium | Cleanup |
| R7 | Extract shared `ErrorBoundary` to `@/components/ui/error-boundary.tsx`; wrap PolerPanel, ConflictGraphDialog | S | Medium | Robustness |
| R8 | Unify paradox backend: have ReasoningDialog consume `ParadoxReportDto` (Layer E) instead of `CycleReport.temporalParadoxes` | M | Medium | Schema consistency |
| R9 | Decide on language policy (RU vs UA) and align all dialogs | L | Low | UX consistency |
| R10 | Lazy-load `ConflictGraphDialog` + `PolerPanel` in web preview | S | Low | Bundle size |

---

## 9. No Code Changes Made

This is an Explore subagent — no source files were modified. Findings reported only.

---

**End of report.**
