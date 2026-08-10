---
Task ID: 10-components-main
Agent: Explore (very thorough)
Scope: Main React application components under src/components/litgraph/
Files inspected: 12 components (LitApp, Toolbar, LitCanvas, CanvasRenderer, Sidebar, NodeEditor, NodeActions, Inspector, NodePalette, LitNodeView, LitEdgeView, PolerPanel) + App.tsx

Work Log:
- Read worklog.md (full 561 lines) — confirmed c143a7f commit context: Toolbar already extended for POLER Ψ button; Layer G LLM Reasoning Bridge is next planned stage; collectedText/selectedChapterText verified to exist post-c143a7f.
- Read LitApp.tsx (19 LOC) — root layout: <Toolbar/> + <LitCanvas/> + <Sidebar/> + <NodeEditor/>.
- Read App.tsx (13 LOC) — wraps <LitApp/> in <ReactFlowProvider> from @xyflow/react. Provider is dead (no <ReactFlow/> rendered downstream).
- Read Toolbar.tsx (951 LOC, full) — top toolbar with ~10 dialog states + 9 dialog instances mounted inline.
- Read LitCanvas.tsx (78 LOC) — thin wrapper around <CanvasRenderer/> + <FloatingActions/> + <NodeContextMenu/>. Also handles Del/Backspace/Ctrl+D hotkeys.
- Read CanvasRenderer.tsx (862 LOC, full) — custom HTML5 canvas renderer with manual viewport/zoom/cull/hit-test/drag/pan + background image layer + ε-resonance rendering.
- Read Sidebar.tsx (240 LOC) — 3 tabs (palette/inspector/legend), TextMomentsDialog/ReaderDialog mounts.
- Read NodeEditor.tsx (415 LOC) — full-screen Dialog hosting per-node <EditorBody key={node.id}> with title/body/fullText/tags/meta/versions UI.
- Read NodeActions.tsx (228 LOC) — right-click <NodeContextMenu> (window event-driven) + <FloatingActions> Figma-style hover bar.
- Read Inspector.tsx (576 LOC) — tri-mode: NodeInspector | EdgeInspector | BackgroundInspector.
- Read NodePalette.tsx (54 LOC) — buttons dispatching `litgraph:add-center` window events.
- Read LitNodeView.tsx (142 LOC) — ReactFlow-style memoized node component using @xyflow/react Handle/Position/NodeProps. NOT MOUNTED anywhere.
- Read LitEdgeView.tsx (97 LOC) — ReactFlow-style memoized edge component using @xyflow/react BaseEdge/EdgeLabelRenderer/getBezierPath. NOT MOUNTED anywhere.
- Read PolerPanel.tsx (785 LOC, full) — Layer F.2 POLER Engine Ψ visualizer with 3 tabs (heatmap/svo/paradoxes), parallel Promise.all of 3 IPC calls.
- Read ReasoningDialog.tsx (first 300 LOC) — Layer E Reasoning Engine UI (deterministic, no LLM) wrapped in ErrorBoundary; consumes reasoningExtractEvents + reasoningGetWorldState + reasoningRunCycle.
- Verified package.json line: `"@xyflow/react": "^12.11.2"` — dependency is real but mostly unused.
- Grepped for selectedChapterText/collectedText — confirmed both exist at Toolbar.tsx lines 97-126 and are passed to 6 dialogs (PolerDialog, NerDialog, CharacterGraphDialog, ConflictGraphDialog, ReasoningDialog use collectedText; PolerPanel uses both selectedChapterText + collectedText).

Atomic Findings:

### F1. Canvas architecture: CUSTOM CANVAS, @xyflow/react is dead weight
- Rendering path: `App.tsx → LitApp → LitCanvas → CanvasRenderer` (pure HTML5 `<canvas>` 2D context).
- `CanvasRenderer.tsx` (862 LOC) implements from scratch: viewport state (`{x, y, zoom}`), DPR-aware canvas sizing, ResizeObserver, dot grid background, viewport transform, Bezier edges with culling, ε-resonance-based edge width (avgEps → 1-5px), node rendering with type-color stripe, ε-heatmap header tint, word-count badge, body text wrapping, selection ring, handle circles, hit-testing (findNodeAt), wheel-zoom-to-cursor, pan (mousedown+move on pane), node drag, background image drag, right-click context menu dispatch via window event.
- `@xyflow/react` is imported in 3 files but **NOT rendered**:
  - `App.tsx:1,6` — `<ReactFlowProvider>` wraps `<LitApp/>` but no `<ReactFlow>` component exists anywhere in the codebase.
  - `LitNodeView.tsx:4` — `Handle, Position, NodeProps, Node` from @xyflow/react; component exported as default but **not imported by any file** (verified by grep).
  - `LitEdgeView.tsx:4` — `BaseEdge, EdgeLabelRenderer, getBezierPath, EdgeProps` from @xyflow/react; component exported but **not imported by any file**.
- Both `LitNodeView` and `LitEdgeView` have custom `areEqual` memo comparators (good practice for ReactFlow), suggesting they were the original renderer and were later replaced by `CanvasRenderer` (canvas-based, faster, supports ε-resonance visualization that ReactFlow can't).
- Architecture smell: `window.__litgraphViewport = viewport` (CanvasRenderer.tsx:140-142) — leaks viewport to window for HTML X-ray export. Comment admits "Не идеально архитектурно". Should live in Zustand.
- Architecture smell: `import("@/lib/litgraph/store").then(...)` dynamic imports at CanvasRenderer.tsx:160, 657 — used in event handlers (add-center, node-drag) to avoid prop drilling. Better solved by closing over `useLitStore.getState()` directly.
- Verdict: Toolbar.tsx:40 comment "Canvas renderer не использует ReactFlow, fitView через window event" confirms intent. Recommendation: **delete LitNodeView.tsx, LitEdgeView.tsx, drop @xyflow/react from package.json, remove ReactFlowProvider from App.tsx** — saves ~240 LOC + 1 dep.

### F2. Toolbar complexity: OVERLOADED — 10 dialog states + 9 dialogs + 9 inline handlers in one 951-LOC file
- 10 dialog open-state vars (Toolbar.tsx:65-86): metaOpen, exportOpen, importMdOpen, aiMode, assistantOpen, polerOpen, polerPanelOpen, nerOpen, charGraphOpen, conflictGraphOpen, reasoningOpen. (bgError and bgImporting are not dialogs, they're transient flags.)
- 9 dialog components mounted inline at bottom (lines 768-948):
  1. `<Dialog>` meta (title/author/description) — inline JSX
  2. `<Dialog>` export (text/markdown preview + copy/download)
  3. `<Dialog>` importMd (paste/upload + parse via Rust+NER pipeline)
  4. `<AIDialog>` (continue-chapter / analyze-plot modes)
  5. `<AssistantDialog>` (chat)
  6. `<PolerDialog>` (spectral k-modes clustering — old)
  7. `<PolerPanel>` (v7.5-LEM canonical — new POLER Ψ)
  8. `<NerDialog>`
  9. `<CharacterGraphDialog>`
  10. `<ConflictGraphDialog>`
  11. `<ReasoningDialog>`
  (that's actually 11 — count includes 3 inline Dialogs + 8 external)
- ~9 inline handler functions (lines 130-397): handleExport, handleDownloadExport, handleExportJson, handleExportHtmlXray, handleImportClick, handleFileChange, handleImportMdClick, handleMdFileChange, handleParseMd (75 LOC async with Tauri/web fallback), handleImportBackgroundClick, handleBgFileChange, processBackgroundFile, handleClearBackground, handleAutoLayout, handleFitView, handleClearAll, handleNewProject.
- 3 hidden `<input type=file>` elements (JSON, MD, Background) — they live in the JSX stream.
- Top-level button inventory in `<header>`: Logo/Brand, Title Input, Search Input, Focus toggle, AI Dropdown, "Спросить AI" quick button, POLER button, POLER Ψ button, NER button, Character Graph button, Conflict Graph button, Reasoning button, Edge Type Dropdown, FitView, AutoLayout, File Dropdown, nodes/edges counter = **17 visible controls** in a single flex-wrap header.
- User-facing redundancy: TWO POLER entry points (POLER → spectral clustering dialog; POLER Ψ → canonical engine panel). TWO AI entry points (AI dropdown + "Спросить AI" button → both can open AssistantDialog).
- Toolbar re-renders on every store change touching title/author/description/nodes.length/edges.length/defaultEdgeKind/searchQuery/focusEnabled/backgroundLayer + the 3 derived selectors (collectedText, selectedChapterText, selectedChapterIndex) which recompute on every nodes array change.

### F3. State management: derived selectors exist post-c143a7f, but inline and un-memoized
- `collectedText` (Toolbar.tsx:97-103): joins all `n.data.fullText` for `n.type === "chapter"` chapters. Returns string. Used by PolerDialog, NerDialog, CharacterGraphDialog, ConflictGraphDialog, ReasoningDialog, PolerPanel (as fullManuscriptText).
- `selectedChapterText` (Toolbar.tsx:108-116): if `selectedNodeId` is a chapter → its fullText; else first chapter's fullText. Used by PolerPanel (as chapterText).
- `selectedChapterIndex` (Toolbar.tsx:118-126): 0-based index of selected chapter among chapters with fullText; 0 if none.
- All 3 selectors are inline `useLitStore((s) => {...})` calls. No `useMemo`/`shallow` equality — recompute on every store mutation (Zustand default behavior).
- Other stores touched by Toolbar: title, author, description, setProjectMeta, nodesCount (via s.nodes.length), edgesCount, defaultEdgeKind, setDefaultEdgeKind, exportProject, loadProject, newProject, setNodes, setEdges, searchQuery, setSearchQuery, focusEnabled, setFocusEnabled, setBackgroundLayer, clearBackgroundLayer, backgroundLayer — **18 distinct store slices** subscribed in one component.
- No `useShallow` from `zustand/react/shallow` is used anywhere in Toolbar → 18 individual `useLitStore` calls = 18 store subscriptions, each triggering re-render on its slice change.

### F4. GUI refactoring opportunities (P0-P2)

**P0 — Extract DialogHost.tsx** (high impact, low risk)
- Create `src/components/litgraph/DialogHost.tsx` that imports all 9 dialog components + owns the 10 open-state vars.
- Toolbar becomes a presentational component receiving `onOpenDialog(dialogName)` callback.
- Reduces Toolbar.tsx from 951 → ~450 LOC. DialogHost.tsx ~250 LOC.
- Bonus: enables single Escape-key handler and stacked-dialog coordination.

**P0 — Extract useDerivedText.ts hook** (low risk)
- Move `collectedText`, `selectedChapterText`, `selectedChapterIndex` to `src/lib/litgraph/hooks/use-derived-text.ts`.
- Wrap in `useMemo` with proper deps (nodes array reference + selectedNodeId).
- Reuse in PolerPanel, PolerDialog, NerDialog, etc. — currently they receive text as prop, but selectors recompute in Toolbar and pass strings down. With a hook, each dialog could subscribe lazily.

**P1 — Split Toolbar into ~6 sub-components**
- `ToolbarBrand` (logo + tagline)
- `ToolbarProjectMeta` (title/author/description inputs + metaOpen dialog trigger)
- `ToolbarSearch` (search input + focus toggle)
- `ToolbarAIActions` (AI dropdown + "Спросить AI" quick button + AIDialog + AssistantDialog)
- `ToolbarAnalysisActions` (POLER + POLER Ψ + NER + CharGraph + ConflictGraph + Reasoning buttons — 6 buttons, all open dialogs)
- `ToolbarCanvasActions` (FitView + AutoLayout + Edge Type dropdown)
- `ToolbarFileMenu` (File dropdown with all import/export actions + 3 hidden file inputs + meta/export/importMd dialogs)

**P1 — Eliminate POLER duplication**
- Two POLER entry points confuse users. Old `PolerDialog` (spectral k-modes clustering, `/lib/poler/`) vs new `PolerPanel` (POLER Engine Ψ v7.5-LEM canonical, Tauri IPC).
- Recommendation: deprecate `PolerDialog.tsx` (or merge as a "Legacy clustering" tab inside PolerPanel) and keep only `PolerPanel` as the single "POLER" button. Old spectral clustering could become a 4th tab.

**P1 — Drop @xyflow/react entirely**
- Delete `LitNodeView.tsx` (142 LOC), `LitEdgeView.tsx` (97 LOC), remove `<ReactFlowProvider>` from `App.tsx`, remove `@xyflow/react` from package.json.
- Saves 240 LOC + 1 npm dependency + ~50 KB bundle.

**P2 — Move window-event bus to store actions**
- Currently 4 window events used as a makeshift event bus:
  - `litgraph:fitview` (Toolbar → CanvasRenderer)
  - `litgraph:add-center` (NodePalette → CanvasRenderer)
  - `litgraph:contextmenu` (CanvasRenderer → NodeActions)
  - `window.__litgraphViewport` (CanvasRenderer → Toolbar HTML export)
- Replace with: `store.fitView()` action that sets a `fitViewNonce` counter the CanvasRenderer watches; `store.addNodeAtViewportCenter(type)` action; `store.setContextMenu({x,y,nodeId})` state field; `store.setViewport()` action.
- Eliminates 4 window event listeners + 1 global variable.

**P2 — Memoize Inspector's per-node derived data**
- `Inspector.tsx:533-538` calls `nodes.find(...)` on every render. Wrap in `useLitStore((s) => s.nodes.find(...))` (Zustand selector) + shallow compare.
- `Inspector.tsx:15-20` (NodeInspector) subscribes to allNodes/allEdges but only uses incoming/outgoing of current node — over-subscribed.

### F5. Layer G UI integration: place "Generate LLM Hypotheses" button in PolerPanel Paradox Feed tab

**Option A (recommended) — Per-paradox "Hypothesize" button + batch button in Paradox Feed tab**
- PolerPanel.tsx:727-762 renders each paradox as a card with kind/character/explanation/origin/manifest.
- Add a `<button>🧪 Hypothesize</button>` to each paradox card that calls `cmd_generate_llm_hypotheses({ paradoxId, paradoxKind, character, originChapter, manifestChapter, explanation, fullManuscriptText, provider: AiProvider })` from a new Layer G Tauri command.
- Add a "Generate All Hypotheses" button at the top of the Paradox Feed tab (next to the "Paradoxes detected" counter, ~line 647) that batch-processes all paradoxes.
- Display returned hypotheses inline below each paradox card (or in a slide-out panel).
- Pros: hypotheses appear next to the paradoxes they explain (UX coherence); preserves PolerPanel's modal flow; no new dialog needed.
- Cons: PolerPanel needs to know about AiProvider (config UI for OpenAI/Ollama endpoint + model + API key) — currently it has no AI config. Need to either reuse existing AIDialog's provider plumbing or add a small provider picker in PolerPanel header.

**Option B — Add a 4th tab "LLM Hypotheses" to PolerPanel**
- Tabs become: heatmap | svo | paradoxes | llm-hypotheses
- The 4th tab lists all paradoxes + their LLM-generated explanations in a unified view.
- Generate-on-open (like the other 3 tabs do parallel IPC on mount) — but Layer G is non-deterministic and slow, so generate-on-button-press is safer than generate-on-tab-open.
- Pros: keeps Paradox Feed pure (no LLM call); explicit separation between deterministic (Layers A-E) and probabilistic (Layer G) outputs.
- Cons: hypotheses are visually divorced from the paradoxes they explain (user must cross-reference two tabs).

**Option C (rejected) — Add Layer G to ReasoningDialog**
- ReasoningDialog is Layer E (deterministic engine: events → world state → cycle report). Mixing LLM-driven Layer G into it conflates two architecturally distinct systems.
- The codebase has TWO parallel reasoning systems already (litgraph-core/src/reasoning Layer E + src-tauri/src/reasoning Wave 5 — see worklog 518-519). Adding Layer G to ReasoningDialog would create a third.
- Reject this option.

**Recommended choice: Option A + small provider picker.** The PolerPanel already imports from `@/lib/tauri-commands` (lines 36-42), so adding `cmdGenerateLlmHypotheses` to that module is a one-line import. The POLER purity invariant (poler.rs:32-37 says "no LLM calls") is preserved because the new command lives in a separate `commands/llm_bridge.rs` module (per worklog 491, 500). The UI button just calls the bridge, not the POLER engine itself.

### File-by-file LOC summary
| File | LOC | Role | Notes |
|---|---|---|---|
| LitApp.tsx | 19 | Root layout | Trivial |
| Toolbar.tsx | 951 | Top toolbar + 11 dialogs | OVERLOADED — see F2, F4 |
| LitCanvas.tsx | 78 | Canvas host + hotkeys | Thin wrapper |
| CanvasRenderer.tsx | 862 | Custom HTML5 canvas renderer | Heart of the app; replace ReactFlow |
| Sidebar.tsx | 240 | 3-tab sidebar | OK |
| NodeEditor.tsx | 415 | Per-node editor dialog | OK |
| NodeActions.tsx | 228 | Context menu + floating actions | OK |
| Inspector.tsx | 576 | Node/Edge/Background inspector | Slightly over-subscribed to store |
| NodePalette.tsx | 54 | Add-node buttons | OK |
| LitNodeView.tsx | 142 | ReactFlow node | DEAD CODE — see F1 |
| LitEdgeView.tsx | 97 | ReactFlow edge | DEAD CODE — see F1 |
| PolerPanel.tsx | 785 | POLER Engine Ψ visualizer | Layer G integration target — see F5 |
| **Total** | **4447** | | |

Stage Summary:
- **Canvas architecture**: Custom HTML5 canvas via CanvasRenderer.tsx (862 LOC). `@xyflow/react` (^12.11.2 in package.json) is imported in 3 files (App.tsx, LitNodeView.tsx, LitEdgeView.tsx) but never rendered — LitNodeView/LitEdgeView are dead code, ReactFlowProvider wraps an empty tree. Recommendation: drop the dependency entirely.
- **Toolbar complexity**: 951 LOC managing 10 dialog open-states and mounting 11 dialogs (3 inline Dialogs + 8 external components) inline. 17 visible controls in one flex-wrap header. Two redundant entry points for both POLER (POLER + POLER Ψ) and AI (dropdown + quick button).
- **State management post-c143a7f**: confirmed `collectedText` (Toolbar.tsx:97-103), `selectedChapterText` (108-116), `selectedChapterIndex` (118-126) all exist as inline `useLitStore` selectors. They are un-memoized, recomputed on every store mutation, and not extracted into a reusable hook.
- **GUI refactoring opportunities (P0-P2)**: (P0) Extract DialogHost.tsx + useDerivedText.ts hook; (P1) Split Toolbar into ~6 sub-components, eliminate POLER duplication, drop @xyflow/react; (P2) Replace 4 window-event-bus calls with Zustand actions, move `window.__litgraphViewport` into store.
- **Layer G UI placement**: Best fit = per-paradox "Hypothesize" button inside PolerPanel's Paradox Feed tab (PolerPanel.tsx:727-762) + a "Generate All Hypotheses" batch button at top of tab. Each button calls new `cmd_generate_llm_hypotheses()` Tauri command (in separate commands/llm_bridge.rs module per worklog 491). PolerPanel needs a small AiProvider picker added to its header (currently has none). Rejected: adding Layer G to ReasoningDialog (would conflate Layer E deterministic engine with Layer G LLM bridge).
- **No code changes made** — this is an Explore report. Implementation recommendations are ready for a downstream Build agent.

Next actions for a Build agent:
1. (P0) Create `src/components/litgraph/DialogHost.tsx` extracting all 11 dialogs + 10 open-state vars from Toolbar.tsx. Toolbar becomes presentational.
2. (P0) Create `src/lib/litgraph/hooks/use-derived-text.ts` exporting `useCollectedText()`, `useSelectedChapterText()`, `useSelectedChapterIndex()`. Memoize with `useMemo` over `[nodes, selectedNodeId]`.
3. (P1) Delete LitNodeView.tsx + LitEdgeView.tsx, remove ReactFlowProvider from App.tsx, remove @xyflow/react from package.json. Run `bun install` + `bun tauri dev` to verify canvas still works.
4. (Layer G prep) Add `cmdGenerateLlmHypotheses` export to `src/lib/tauri-commands.ts` (stub returning typed HypothesisReportDto) — actual Tauri command implementation is subagent 05's scope.
5. (Layer G UI) Add a "🧪 Hypothesize" button to each paradox card in PolerPanel.tsx:727-762 calling `cmdGenerateLlmHypotheses(...)`. Add AiProvider picker to PolerPanel header (lines 204-220) — reuse AIDialog's provider plumbing pattern.
6. (P2) Move `window.__litgraphViewport` and 3 window-event-bus calls into Zustand store actions + state fields.
