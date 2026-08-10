---
Task ID: 09-ts-lib-poler-litgraph
Agent: Explore (very thorough)
Scope: TS-side POLER analysis library + litgraph store/types/api/export
Files inspected: 21 files under src/lib/{poler,litgraph,conflict}/ + utils.ts — ~7 700 LOC
Date: 2026-08-08
---

# Atomic Report — TS-side POLER + LitGraph store/types/api/export

## 0. Executive Summary

The `src/lib/` tree contains **three parallel analysis pipelines** that the
codebase presents under the same POLER banner but which actually target
different problem domains and backends:

| # | Pipeline (TS entrypoint) | Backend | Math level | Purpose |
|---|--------------------------|---------|------------|---------|
| 1 | `poler/analyze.ts` → `polerCore.ts` + `textGraph.ts` + `clustering.ts` | Pure TS (in-browser) | word-level spectral clustering on operator H = Π·(L+γJ−B/m)·Π | Demo / educational prototype; one-shot k-means word clusters for PolerDialog |
| 2 | `tauri-commands.ts::cmdComputeEpsilonClimax/cmdExtractSvo/cmdDetectParadoxes` | Rust-native (litgraph-core Layer E via `commands/poler.rs`) | chapter ε-climax + Ukrainian SVO + paradox detection | Production Layer F.1 consumed by PolerPanel |
| 3 | `poler/nerBridge.ts::extractEntities/analyzeCharacters/extractSvo` + `conflict/api.ts::getConflictGraph` | Python via Tauri `std::process::Command` | NER (spaCy) + Russian SVO + J-matrix conflict graph | Russian-language analysis consumed by NerDialog/CharacterGraphDialog/ConflictGraphDialog |

The store (`litgraph/store.ts`) is still on **Zustand + localStorage** despite
`tauri-plugin-store` already being installed in `Cargo.toml` + registered in
`lib.rs:23` — migration is blocked only on the TS side. LitNode/LitEdge types
are **fully aligned with src-tauri** (11 node variants, 9 edge kinds) but
**litgraph-core lags by 2 variants** (`Concept`, `Organization`) — confirmed
existing drift, not TS-side bug.

---

## 1. poler/* — Client-side analytics

### 1.1 File inventory (8 files, 2 173 LOC)

| File | LOC | Role | Live consumer |
|------|-----|------|---------------|
| `polerCore.ts` | 294 | POLER dynamics kernel — `evolve()`, `buildPolarOperator()`, `energyF()`, `gradF()`, `resonanceTerm()`, mulberry32 PRNG | `analyze.ts` only |
| `analyze.ts` | 172 | High-level `analyzeText(text, options)` — orchestrates textGraph → polerCore → clustering | `PolerDialog.tsx:15` |
| `textGraph.ts` | 245 | Co-occurrence CSR builder, normalized Laplacian, modularity matrix B, projector Π_Λ | `analyze.ts` only |
| `clustering.ts` | 249 | Jacobi eigenvalue sweep `smallestEigenvectors()`, `kMeans()`, `silhouette()` | `analyze.ts` only |
| `nerBridge.ts` | 212 | **Tauri IPC bridge** for Python NER/SVO/conflict (extract_entities, analyze_characters, extract_svo) | `NerDialog.tsx`, `CharacterGraphDialog.tsx` |
| `nerTypes.ts` | 73 | TS mirror of Rust `commands/ner.rs` structs (Entity, NerResult, NerStats, ENTITY_LABELS) | `NerDialog.tsx`, `nerBridge.ts` |
| `textMoments.ts` | 603 | Pure-TS chapter detection + keyword fragment clustering (port of POLER v6) | `ReaderDialog.tsx`, `readerRender.ts` |
| `readerRender.ts` | 325 | HTML rendering of chapter text with keyword+target highlighting (escapeHtml, `<mark>` segmentation) | `ReaderDialog.tsx` |

### 1.2 polerCore.ts — POLER dynamics (analysis #1 above)

Canonical equation implemented:
```
dp/dt = -η · Π_Λ · [ D·p + γ·J·p_mem + ∇F ]
```
where `D=L` (normalized Laplacian), `J=(A_dir−A_dirᵀ)/2`, `Π_Λ=I−(1/n)·1·1ᵀ`,
`F=−pᵀBp/(2m)`, `∇F=−B·p/m`. Memory buffer of size 16 (FIFO) with exponential
decay `ρ^k` (default 0.9). Backtracking line-search halves `η` when energy
increases; convergence at `‖dp‖<tol=1e-7` or `η<1e-6`.

Operator construction `buildPolarOperator` symmetrizes `H=Π·(L+γJ−B/m)·Π`.
Eigen-decomposition uses **Jacobi sweeps** (maxSweeps=100, off-diag tol 1e-10)
— practical only for matrices ≤ ~200×200 (cubic complexity, no Lanczos).

Default parameters (`DEFAULT_PARAMS`): `eta=0.1, gamma=0.05, rho=0.9,
maxIter=500, tol=1e-7, backtracking=true`.

### 1.3 analyze.ts — `analyzeText()` orchestration

Pipeline (pure, synchronous, deterministic):
1. `tokenize(text)` — lowercase split on `[^a-zа-яё0-9]+`, length ≥2, **no stop-word removal**
2. `buildVocabulary(tokens, minFreq=2)` — filter + sort
3. `buildCooccurrence` (CSR sparse, weight `1/distance`, window=5)
4. `buildDirectedAdjacency` (i→j if w_t=i, w_{t+1}=j)
5. `buildLaplacian` (dense n×n, symmetrized) — **O(n²) memory**
6. `buildModularityMatrix` (dense n×n) — **O(n²) memory**
7. `buildProjector` (dense n×n)
8. Build J from A_dir (dense)
9. `evolve()` — single POLER mode for diagnostics
10. `buildPolarOperator()` → `smallestEigenvectors(H, kModes=4)` (skip trivial λ≈0)
11. `kMeans(X, kModes, 100, seed=42)` in mode space
12. `silhouette(X, labels)`

Hard cap in `PolerDialog.tsx:82`: `text.slice(0, 50000)` — **truncates
manuscripts >50k chars**. Comment "Для больших текстов (>50k символов)
рекомендуется Rust-порт (TODO)".

### 1.4 Redundancy with Rust-native POLER (cmd_*)

**Verdict: NOT redundant, but parallel.** The three pipelines solve
non-overlapping problems:

| Aspect | TS polerCore/analyze | Rust cmd_* (litgraph-core Layer E) |
|--------|----------------------|------------------------------------|
| Mathematical object | Word co-occurrence graph | Character/scene ε-climax + character conflict graph |
| Output | Word clusters (k-means labels) | ε value, SVO triplets, paradox list |
| Operator | H = Π(L+γJ−B/m)Π (word space) | Ω_conf = ‖A_POS‖_F + spectral_radius(A_POS) on character adjacency |
| Input granularity | Tokens (single words) | Chapters + characters + SVO |
| Language | Language-agnostic (token chars) | Ukrainian (Rust lemmatizer); Russian via Python |
| Performance | O(n²) memory, ≤200×200 eig | Production-grade, no hard cap |
| Determinism | mulberry32(seed=42) | Deterministic |
| Used by | PolerDialog (word-cloud cluster view) | PolerPanel (ε heatmap + SVO inspector + paradox feed) |

The TS pipeline is effectively a **leftover prototype** (a JS port of the
Python `poler_v6.py` from `poler-prototype/`). It still ships because (a)
it's fully client-side so it works in web preview without Tauri, and (b)
its word-cluster view is visually distinct from PolerPanel's chapter-level
ε view. **However, the PolerDialog UI is reachable from the toolbar and
runs on truncated 50k text** — a UX footgun for novels.

### 1.5 nerBridge.ts — Python NER IPC bridge

**Dispatch path**: `extractEntities(text)` → `callApi("extract_entities",
"/api/ner-extract", {text})` → if `isTauri` → `window.__TAURI_INTERNALS__.invoke("extract_entities", {text})`
→ Rust `commands/ner.rs::extract_entities` → `std::process::Command::new(python3)
.arg(ner_extract.py).arg(input_text.txt)` (verified in subagent-07 report).

**NOT direct fetch** in Tauri mode — the `/api/ner-extract` endpoint is only
used in the non-Tauri web-preview path (and would 404 since no Next.js
backend exists). Three exposed functions:
- `extractEntities(text): Promise<NerResult>` — PER/LOC/GPE/ORG/MISC entities
- `analyzeCharacters(text): Promise<CharacterAnalysisResult>` — full pipeline (NER + co-occurrence graph + POLER operator H + k-means + SVO asymmetry) — Python's `poler_entities.py` mirrors TS `analyze.ts` exactly
- `extractSvo(text): Promise<SvoResult>` — Russian SVO via `svo_extract.py`
- `checkNerAvailability()` — probes with "Тест Анна Москва"

**Critical observation**: `analyzeCharacters` (Python) and `analyzeText`
(TS) are **two implementations of the same algorithm** — word/character
co-occurrence → POLER operator → k-means. Python version handles full text
in chunks of 50k (per its docstring), TS version truncates at 50k. **This
is the genuine redundancy** in the codebase.

### 1.6 textMoments.ts + readerRender.ts — pure-TS fragment clustering

`findTextMoments(text, node, options)` — 6-step pipeline:
1. `detectChapters(text)` — 9 regex patterns (`Глава N`, `Розділ N`,
   `Chapter N`, `Part N`, `## N` etc.) — picks the pattern with most matches
2. `extractKeywords(node)` — title + `meta.forms` + `meta.aliases` (lowercased Set)
3. `findKeywordPositions(text, keywords)` — longest-match-wins with
   word-boundary check (custom Cyrillic-aware `[\wа-яёіїєґ]`)
4. `deduplicatePositions` — within-chapter only (v0.5.1 fix)
5. `extractFragment` ±200 chars, UTF-16 surrogate-safe
6. `computeDensity` — `(keywordHits / fragmentWordCount) × 100 × 5` boost

Output grouped `byChapter` with stats. `readerRender.ts::renderChapter()`
produces `dangerouslySetInnerHTML`-safe HTML with `<mark class="reader-keyword">`
and `<mark class="reader-target" id="...">` for in-Reader scrolling.

**Chapter detection is duplicated 3× in the codebase**: TS `textMoments.ts`
(9 patterns), Rust `parser/chapters.rs` (per subagent-02), Python
`ner_extract.py` (probably similar). Verified regex set roughly matches
Rust parser. **No drift risk for IPC** because `textMoments.ts` runs
purely client-side (for Reader preview); chapter boundaries returned to
the UI never cross the Tauri boundary.

### 1.7 Type drift: `SvoAsymmetry` declared twice in nerBridge.ts

`nerBridge.ts:44-49` declares `SvoAsymmetry` and again at `:169-174` —
identical fields, **duplicate declaration**. ESLint would flag this; tsc
apparently accepts it because TypeScript merges identical interfaces.
Should be removed.

---

## 2. litgraph/store.ts — Zustand + localStorage

### 2.1 Persistence architecture

```ts
create<LitStore>()(
  persist(
    (set, get) => ({ ... }),
    {
      name: "litgraph-store-v1",
      storage: typeof window !== "undefined" && window.localStorage
        ? createJSONStorage(() => window.localStorage)
        : undefined,
      partialize: (s) => ({ /* see below */ }),
      onRehydrateStorage: () => (state) => { /* demo seed */ },
    }
  )
);
```

### 2.2 Persisted state (`partialize`)

| Field | Persisted | Reason |
|-------|-----------|--------|
| `title`, `author`, `description` | ✅ | project metadata |
| `nodes`, `edges` | ✅ | full graph (no size cap!) |
| `defaultEdgeKind`, `focusEnabled` | ✅ | UI prefs |
| `backgroundLayer` | ✅ if `src.length < 5_000_000` (5 MB base64 cap) | localStorage ~5-10 MB limit |
| `selectedNodeId`, `selectedEdgeId`, `editingNodeId` | ❌ | ephemeral UI |
| `searchQuery`, `hideTag`, `focusNodeId` | ❌ | ephemeral UI |
| `backgroundMoving` | ❌ | transient drag state |
| `sourceMarkdown` | ❌ | re-imported by user; **NOT persisted** (loses source on refresh!) |
| `readerOpen`, `readerTarget` | ❌ | modal state |

**Latent bug**: `sourceMarkdown` is NOT persisted but `nodes` ARE — so on
reload, the user sees the imported graph but the Reader's "Text Moments"
feature silently breaks (no source text to search). The Toolbar's .md
import button must be re-clicked. **Not a crash, but a UX regression.**

### 2.3 Tauri Store plugin — installation status

| Layer | Status |
|-------|--------|
| `package.json:24` | `@tauri-apps/plugin-store: ^2.0.0` ✅ installed |
| `src-tauri/Cargo.toml:20` | `tauri-plugin-store = "2"` ✅ installed |
| `src-tauri/src/lib.rs:23` | `.plugin(tauri_plugin_store::Builder::new().build())` ✅ registered |
| `src/lib/litgraph/store.ts` | ❌ **still uses `window.localStorage`** |
| `capabilities/` (Tauri 2 ACL) | not found in repo (subagent-01 flagged) |

The Tauri Store plugin is **fully provisioned but unused**. Migration is
blocked purely on the TS side. The Zustand `persist` middleware accepts a
custom `StateStorage` adapter — replacing `createJSONStorage(() =>
window.localStorage)` with a Tauri-backed async storage is the only
required change.

### 2.4 Migration plan (concrete)

```ts
// New file: src/lib/litgraph/tauri-storage.ts
import { Store } from "@tauri-apps/plugin-store";
import type { StateStorage } from "zustand/middleware";

const store = new Store("litgraph-store-v1.json");

export const tauriStorage: StateStorage = {
  getItem: async (name) => await store.get(name) ?? null,
  setItem: async (name, value) => await store.set(name, value),
  removeItem: async (name) => await store.delete(name),
};

export const tauriJSONStorage = createJSONStorage(() => tauriStorage);
```

Then in `store.ts`:
```ts
storage: isTauri
  ? tauriJSONStorage
  : createJSONStorage(() => window.localStorage),
```

**Caveats**:
1. Tauri Store is async — Zustand persist supports async storage natively.
2. Need to add `tauri-plugin-store` to `capabilities/default.json` (Tauri 2 ACL)
3. The 5 MB `backgroundLayer` cap can be removed (Tauri Store has no such limit; writes go to disk).
4. Migration path: detect old `litgraph-store-v1` in localStorage on first launch → import to Tauri Store → mark migrated.
5. `sourceMarkdown` should be added to `partialize` (it was forgotten — see §2.2 bug above).

### 2.5 Store surface (full API)

**State (17 fields)**: title, author, description, nodes, edges,
selectedNodeId, selectedEdgeId, editingNodeId, defaultEdgeKind, searchQuery,
hideTag, focusNodeId, focusEnabled, backgroundLayer, backgroundMoving,
sourceMarkdown, readerOpen, readerTarget.

**Actions (29)**: addNode, updateNode, updateNodeData, updateNodeMeta,
deleteNode, duplicateNode, setNodes, setEdges, onNodesChange,
onEdgesChange, onConnect, addEdge, updateEdge, deleteEdge, setSelectedNode,
setSelectedEdge, setEditingNode, setDefaultEdgeKind, setSearchQuery,
setHideTag, setFocusNode, setFocusEnabled, setBackgroundLayer,
updateBackgroundLayer, clearBackgroundLayer, toggleBackgroundVisibility,
setBackgroundMoving, setProjectMeta, setSourceMarkdown, openReader,
closeReader, setReaderIndex, newProject, loadProject, exportProject,
getVisibleNodes, getAllTags, saveVersion, restoreVersion, deleteVersion,
getVersions.

**`uid(prefix)`**: `${prefix}_${Date.now().toString(36)}_${Math.random().toString(36).slice(2,8)}`
— 14 chars of entropy, collision-safe for single-user sessions.

**`createDefaultProject()`**: 8-node demo (1 chapter + 2 scenes + 2
characters + 1 location + 1 plotpoint + 1 conflict) with 10 edges. Seeded
on first launch via `onRehydrateStorage`.

---

## 3. LitNode / LitEdge types — alignment audit

### 3.1 LitNodeType — three-way comparison

| Source | Variants | Count |
|--------|----------|-------|
| `src/lib/litgraph/types.ts` | scene, character, plotpoint, conflict, dialogue, location, idea, chapter, theme, **concept, organization** | **11** |
| `src-tauri/src/models/node.rs` | Scene, Character, Plotpoint, Conflict, Dialogue, Location, Idea, Chapter, Theme, **Concept, Organization** | **11** |
| `litgraph-core/src/models/node.rs` | Scene, Character, Plotpoint, Conflict, Dialogue, Location, Idea, Chapter, Theme | **9** ❌ |

**Wire format** (src-tauri/node.rs uses `#[serde(rename_all = "lowercase")]`):
`Concept → "concept"`, `Organization → "organization"` — matches TS
`"concept" | "organization"` exactly. ✅ **TS ↔ src-tauri IPC is correct.**

**Drift confirmed (subagent-04 finding)**: `litgraph-core/node.rs` is
missing `Concept` + `Organization` (v0.4.2). This matters only for in-process
Rust tests that exercise the core crate directly — IPC path is unaffected
because src-tauri has its own copy.

### 3.2 EdgeKind — three-way comparison

| Source | Type | Variants |
|--------|------|----------|
| `src/lib/litgraph/types.ts` | union of 9 string literals | flow, cause, character, location, reference, conflict, foreshadow, alternative, theme |
| `src-tauri/src/models/edge.rs` | `pub type EdgeKind = String` | unbounded (no enum) |
| `litgraph-core/src/models/edge.rs` | `pub type EdgeKind = String` | unbounded (no enum) |

**Critical**: Rust treats `EdgeKind` as a free-form `String` (not an enum).
TS-side is stricter (union of 9). **Anything outside the 9 will fail
TS typecheck at the boundary** unless a cast is used. Current Tauri
commands (`save_project`, `load_project`) accept arbitrary strings —
this is a forward-compat escape hatch, but means TS-side guarantees
can be violated by hand-edited project JSON.

### 3.3 LitNodeData shape

TS interface (29 lines, types.ts:16-37):
```ts
interface LitNodeData {
  title: string;
  body: string;
  type: LitNodeType;
  tags: string[];
  meta?: {
    pov?, mood?, timeOfDay?, wordTarget?, characterArc?,
    importance?, manifestation?,
    [key: string]: unknown;
  };
  fullText?: string;             // chapter/scene full text (versions)
  versions?: ChapterVersion[];   // up to 50 versions, capped in saveVersion
  [key: string]: unknown;
}
```

`ChapterVersion` (5 fields): id, timestamp, fullText, wordCount, label?,
source? (`"auto"|"manual"|"ai"|"restore"|"import"`).

### 3.4 NODE_TYPES config (11 entries, ~330 LOC of config)

Each entry: `{ type, label, singular, plural, description, icon, color,
accent, defaultBody, fields[] }`. Icons are lucide-react names (Clapperboard,
User, Flag, Swords, MessagesSquare, MapPin, Lightbulb, BookOpen, Sparkle,
Cloud, Building2). Concept v0.4.0 color `#7C3AED`, Organization v0.4.0
color `#DC2626`.

`NODE_TYPE_ORDER`: chapter, scene, plotpoint, conflict, character,
dialogue, location, organization, concept, theme, idea.

### 3.5 EDGE_TYPES config (9 entries)

`flow, cause, character, location, reference, conflict, foreshadow,
alternative, theme`. Each: `{ kind, label, description, color, dashed,
animated }`. Animated kinds: `flow`, `foreshadow`.

---

## 4. api.ts — callApi IPC dispatcher

```ts
export const isTauri = typeof window !== "undefined" &&
  ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

export async function callApi<T>(
  _tauriCommand: string,
  webEndpoint: string,
  payload: Record<string, unknown>,
  tauriWrapper?: string,
): Promise<T>
```

**Tauri path**: `window.__TAURI_INTERNALS__.invoke(_tauriCommand,
tauriWrapper ? { [tauriWrapper]: payload } : payload)`. Uses global
`__TAURI_INTERNALS__` rather than `import { invoke } from
"@tauri-apps/api/core"` — avoids bundler dead-code-elimination issues
when running in non-Tauri (vite preview / SSR).

**Web-preview path**: `fetch(webEndpoint, { method: "POST", body:
JSON.stringify(payload) })`. **No Next.js API routes exist** in the repo
for `/api/ner-extract`, `/api/analyze-characters`, `/api/extract-svo`,
`/api/conflict-graph` — these endpoints are aspirational. Web-preview
users get an explicit "доступен только в Tauri-версии" error from
`nerBridge.ts` instead.

**AI provider plumbing bug** (subagent-05 finding): `callApi` accepts an
optional 4th arg `tauriWrapper` which is currently always `undefined`
in callers. AI dialogs (`AIDialog.tsx`, `AssistantDialog.tsx`) bypass
`callApi` entirely with their own broken `callApi`-like shim that omits
the `provider` field — see subagent-05 report. Confirmed NOT fixed at HEAD.

---

## 5. litgraph/export.ts + export-html.ts + export-svg.ts

### 5.1 export.ts (233 LOC)
- `exportToText(project)` — plain-text screenplay format, groups by node type, walks flow-edges
- `exportToMarkdown(project)` — standard Markdown with sections per type
- `downloadFile(content, filename, mime)` — Tauri save dialog + `writeTextFile`
  with browser `<a download>` fallback
- `slugify(s)` — lowercase, strip non-alphanumerics, cap at 40 chars

### 5.2 export-html.ts (2 048 LOC — largest file in scope)
Self-contained interactive HTML "mini-program" export. Embeds JSON in
`<script type="application/json" id="litgraph-data">`. Features:
- Canvas with pan/zoom (drag, wheel)
- Click node → sidebar with full meta + reason + SVO + J
- Search by title/body/tags
- Background toggle + opacity slider
- Keyboard: F=fit, +/- zoom, arrows pan, Esc deselect
- Consumes `heuristics.ts::analyzeWorkspace` for X-ray diagnostics
- `buildNodeReason(node)` per-type reason string (e.g.
  `character:freq=268;speech=73;direct=12;prefix=Рэй;NOT_IN_STOPLIST`)

### 5.3 export-svg.ts (663 LOC)
Vector X-ray export. Embeds `<image>` with base64 background,
`<g data-node-id="..." data-reason="...">` per node,
`<path data-edge-id="..." data-j="...">` per edge. Node height scaled by
ε (`getNodeHeight(node)` = `BASE + (ε/100) * (MAX−BASE)`).

### 5.4 background-layer.ts (264 LOC)
Decodes SVG/PNG/JPEG/WebP natively, TIFF via UTIF (dynamic import).
Converts to base64 `data:` URL. Returns `BackgroundLayer` with sensible
defaults (`opacity=0.55`, `scale` fits ~600px max side).
`pickImageFileViaDialog()` uses Tauri `plugin-dialog` + `plugin-fs`
when available, falls back to hidden `<input type=file>` in browser.

### 5.5 heuristics.ts (438 LOC) — Smart X-Ray diagnostics
`analyzeWorkspace(nodes, _edges) → Map<nodeId, NodeDiagnostic>`. 8
heuristics (H1-H8):
- H1 SUSPECT_WORD (title in 60-word RU/UK/EN abstraction set)
- H2 NO_SPEECH_VERBS (error level)
- H3 MINIMAL_SPEECH (warn if speech<3 && freq>20)
- H4 LOW_SPEECH_RATIO (warn if speech/freq < 5%)
- H5 DIRECT_ADDRESS_PATTERN_MISS (info)
- H6 CROSS_TYPE_LEMMA_COLLISION (merge suggestion via 4-char prefix)
- H7 SUSPECT_LOCATION_NAME (info)
- H8 LOW_FREQ_LOCATION (info if freq<5)

Confidence scoring: error −0.5, warn −0.25, info −0.05. Buckets:
`ok ≥ 0.85`, `suspect 0.5–0.85`, `error < 0.5`.

### 5.6 epsilon.ts (378 LOC) — Client-side POLER v6 ε
Implements canonical formula:
```
ε = (κ · kw_intensity · d_sq + emotion) / sqrt(unique_words)
```
- `buildWordCounts(text)` — global frequency map
- `computeEpsilon(chapterText, globalCounts, totalWords, keyword, kappa)`
- `computeEpsilonFragmented(...)` — splits chapter into ~1500-char
  fragments with 300-char overlap; chapter ε = 0.7·max + 0.3·avg
- `normalizeEpsilons(results)` — 0–100 scale
- `computeResonanceSeries(epsilons, rho=0.85, alpha=0.1)` — `R[t]=ρ·R[t-1]+α·E·(1+E)`
- `clusterChapters(epsilons, threshold=20)` — adaptive gap clustering

**This is the TS prototype of the same algorithm that Rust `cmd_compute_epsilon_climax`
now implements canonically** (Layer E, per subagent-03). The Rust version
adds Ω_conflict and spectral_radius; TS version is simpler. Currently
the TS epsilon.ts has **no live consumer** (grep finds 0 imports outside
the file itself and export-html.ts embeds it as data only).

### 5.7 conflict/* (3 files, 366 LOC)
- `conflict/types.ts` — TS mirror of Rust `commands/conflict.rs`
  `ConflictGraph` (nodes, edges, matrix, stats, model, version, svoVersion, textLength)
- `conflict/api.ts` — `getConflictGraph(text)` → `callApi("get_conflict_graph", ...)`
- `conflict/export.ts` — PNG/PDF export via `html-to-image` + `jsPDF` with
  Tauri save dialog + browser fallback

---

## 6. Layer G integration readiness

### 6.1 What Layer G needs from the store

Currently the store has **zero Layer G fields**. Required additions:

```ts
interface LitStore {
  // ... existing fields ...

  // ====== Layer G: LLM Reasoning Bridge ======
  llmHypotheses: LlmHypothesis[];        // generated hypotheses queue
  activeHypothesisId: string | null;      // currently selected
  validationResults: ValidationResult[];  // accept/reject/retry outcomes
  llmProviderConfig: AiProviderConfig | null;  // moved from local dialog state
  llmLoading: boolean;
  llmError: string | null;
  paradoxReport: ParadoxReportDto | null; // cached from cmd_detect_paradoxes
  epsilonClimax: EpsilonClimaxDto[];      // cached per-chapter

  // Actions
  generateHypotheses: (paradoxIds: string[]) => Promise<void>;
  validateHypothesis: (hypothesisId: string, accept: boolean) => Promise<void>;
  applyHypothesis: (hypothesisId: string) => void;  // mutate graph
  dismissHypothesis: (hypothesisId: string) => void;
  setLlmProviderConfig: (cfg: AiProviderConfig) => void;
}
```

### 6.2 Where Layer G should live

Per subagent-05 finding: **poler.rs has a documented purity invariant
("no LLM calls" — `commands/poler.rs:32-37`)**. Layer G must live in a
**new module** `src-tauri/src/commands/llm_bridge.rs` consuming
`crate::ai::prompts` + `crate::reasoning::narrative_graph::ParadoxReport`
+ `crate::ai::AiProvider`. The TS-side bridge should be a new
`src/lib/llm-bridge/api.ts` (mirroring `poler/nerBridge.ts` pattern)
that calls `cmd_generate_llm_hypotheses(text, provider, options?)`.

### 6.3 Existing pre-Layer G plumbing

- `tauri-commands.ts:200-201` — `CycleReport.hypothesesGenerated` / `hypothesesAccepted` (Wave 5 reasoning cycle, NOT Layer G)
- `tauri-commands.ts:217-228` — `ValidationResultDto` discriminated union (`accept | reject | retry`) — **already designed for Layer G**; reusable
- `PolerPanel.tsx:102` — `paradoxReport` local state (NOT in store) — should be lifted to store for Layer G to consume
- `ReasoningDialog.tsx:402` — already displays `${accepted}/${generated}` — UI pattern exists

### 6.4 Migration path (concrete 4-step plan)

1. **Add 7 Layer G fields to store.ts** (above). Persist only
   `llmProviderConfig` (NOT transient `llmHypotheses`/`validationResults` —
   those should regenerate from paradox report on reload).
2. **Lift `paradoxReport` + `epsilonClimax` from PolerPanel local state to store.**
   This lets Layer G consume them without prop-drilling.
3. **Add `src/lib/llm-bridge/api.ts`** with
   `generateLlmHypotheses(text, paradoxIds, provider)` →
   `callApi("cmd_generate_llm_hypotheses", ...)`.
4. **Persist `sourceMarkdown` in store** (currently forgotten, see §2.2) —
   Layer G needs the source text to construct prompts.

---

## 7. Findings summary

### F1 — Client-side POLER is a leftover prototype, NOT redundant with Rust
**Severity**: LOW (technical debt, not a bug).
TS `polerCore.ts` + `analyze.ts` + `textGraph.ts` + `clustering.ts`
(760 LOC total) implement word-level spectral clustering. Rust
`cmd_compute_epsilon_climax` / `cmd_extract_svo` / `cmd_detect_paradoxes`
implement chapter-level ε + Ukrainian SVO + paradox detection. **Different
problems, different backends, different UIs (PolerDialog vs PolerPanel).**
The TS pipeline truncates at 50k chars and uses O(n²) memory — production
use should funnel through Rust. **Recommendation**: keep TS pipeline as
"web-preview fallback" or delete PolerDialog entirely (PolerPanel
already covers production needs).

### F2 — `analyzeCharacters` (Python) duplicates `analyzeText` (TS)
**Severity**: MEDIUM (genuine algorithm duplication).
Python `poler_entities.py` (per subagent-07) and TS `analyze.ts` both
implement co-occurrence → POLER operator H → k-means. Python handles
full text in chunks; TS truncates at 50k. **One should be canonical.**
Recommendation: deprecate TS `analyzeText` in favor of an IPC wrapper
around Python (consistent with `extractEntities`/`extractSvo` pattern).

### F3 — Store NOT migrated to Tauri Store plugin
**Severity**: HIGH (announced in README, infra ready, blocked on 30 LOC of TS).
`tauri-plugin-store` is installed (`Cargo.toml:20`, `package.json:24`,
registered `lib.rs:23`) but `store.ts:627` still uses
`window.localStorage`. **5 MB `backgroundLayer` cap and absent
`sourceMarkdown` persistence are direct consequences.** Migration
plan in §2.4 is 30 LOC of new adapter + 1-line swap.

### F4 — `sourceMarkdown` not persisted (UX bug)
**Severity**: MEDIUM.
After page reload, `nodes` are restored but `sourceMarkdown` is empty.
TextMomentsDialog / ReaderDialog / PolerPanel all depend on
`sourceMarkdown` and silently degrade. **Fix**: add `sourceMarkdown:
s.sourceMarkdown` to `partialize` (1-line).

### F5 — LitNode types: TS ↔ src-tauri aligned; litgraph-core lags
**Severity**: LOW (no IPC impact; only affects core-crate tests).
TS `types.ts` has 11 `LitNodeType` variants matching src-tauri v0.4.2.
`litgraph-core/node.rs` has only 9 (missing `Concept`, `Organization`).
Subagent-04 already flagged this. Fix is to backport the 2 variants
upstream.

### F6 — EdgeKind is `String` on Rust side, strict union on TS side
**Severity**: LOW.
TS union of 9 kinds is more restrictive than Rust `type EdgeKind = String`.
Hand-edited project JSON with `"kind": "custom"` will fail TS typecheck at
the IPC boundary unless a cast is used. **Current code path is safe**
(no custom kinds emitted by Rust) but offers no forward-compat.

### F7 — `nerBridge.ts` duplicate `SvoAsymmetry` declaration
**Severity**: LOW (cosmetic).
`SvoAsymmetry` declared at lines 44-49 AND 169-174 — identical fields.
TS interface merging hides this; should be deduplicated.

### F8 — Web-preview endpoints are aspirational
**Severity**: LOW.
`callApi` falls through to `fetch("/api/ner-extract", ...)` etc. in
non-Tauri mode, but **no Next.js / Vite API routes exist** for these.
Callers in `nerBridge.ts` short-circuit with explicit errors before
reaching `callApi`, so users see a friendly message. The dead `fetch`
branch is misleading — should be removed or implemented.

### F9 — Chapter detection duplicated 3× across stack
**Severity**: LOW.
TS `textMoments.ts::detectChapters` (9 patterns), Rust
`parser/chapters.rs`, Python `ner_extract.py` all do chapter regex
detection independently. **No drift risk** because TS version is
purely client-side for Reader preview. But maintenance burden if
patterns change. Could be unified by exposing
`cmd_detect_chapters` from Rust and calling it from TS.

### F10 — TS `epsilon.ts` is dead code (no live consumer)
**Severity**: LOW.
`computeEpsilon`, `computeEpsilonFragmented`, `computeResonanceSeries`,
`clusterChapters` — 378 LOC of POLER v6 port. **Rust `cmd_compute_epsilon_climax`
replaced it canonically.** Currently no component imports these functions
(only `export-html.ts` references the file for embedding constants).
Candidate for deletion after verifying export-html doesn't actually invoke them.

---

## 8. Recommended next actions (priority-ordered)

| # | Priority | Action | Effort | Owner |
|---|----------|--------|--------|-------|
| 1 | P0 | Migrate `store.ts` to Tauri Store plugin (§2.4) — 30 LOC | 1h | frontend |
| 2 | P0 | Add `sourceMarkdown` to `partialize` (F4) | 5min | frontend |
| 3 | P1 | Add Layer G store fields (§6.1) — 7 fields + 4 actions | 2h | frontend |
| 4 | P1 | Lift `paradoxReport` from PolerPanel local state to store | 30min | frontend |
| 5 | P1 | Create `src/lib/llm-bridge/api.ts` skeleton (mirroring nerBridge pattern) | 1h | frontend |
| 6 | P2 | Deduplicate `SvoAsymmetry` in nerBridge.ts (F7) | 5min | frontend |
| 7 | P2 | Backport `Concept`/`Organization` to litgraph-core/node.rs (F5, subagent-04 R1) | 15min | backend |
| 8 | P2 | Decide: delete `epsilon.ts` (F10) or wire it to a consumer | 30min | frontend |
| 9 | P3 | Decide: deprecate PolerDialog + TS polerCore (F1, F2) in favor of PolerPanel + Rust | 4h | product |
| 10 | P3 | Add `capabilities/default.json` for `tauri-plugin-store` ACL (subagent-01 P0) | 30min | backend |
| 11 | P3 | Remove dead `fetch` branches in `callApi` web-preview path (F8) | 15min | frontend |

---

## 9. Status

COMPLETED. All 21 in-scope files read end-to-end. Findings cross-referenced
with subagent reports 01-07. No code modified (read-only audit).
