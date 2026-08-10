---
Task ID: 12-components-ui-config
Agent: Explore (medium thoroughness)
Scope: shadcn/ui primitives, app entry, config files, design system
Files inspected: 15 (7 ui primitives + components.json + globals.css + App.tsx + main.tsx + types/utif.d.ts + tsconfig.json + tsconfig.node.json + vite.config.ts + package.json) + cross-grep against litgraph/* consumers
Total LOC in scope: ~590 (ui primitives: 230 LOC; config: 145 LOC; entry: 30 LOC; globals.css: 239 LOC)
---

# Atomic Report — shadcn/ui Primitives, App Entry & Design System

## 1. Executive Summary

The frontend design system is a **standard shadcn/ui "new-york" + Tailwind v4 + Radix** setup, mechanically correct and self-consistent at the primitive level. The 7 existing primitives (`badge`, `button`, `dialog`, `dropdown-menu`, `input`, `label`, `textarea`) are textbook shadcn output — function-component form, `data-slot` attributes, `cva` variants, `cn()` helper, latest Tailwind v4 idioms (`dark:bg-input/30`, `field-sizing-content`, `outline-hidden`).

**Three concrete defects** stand out:

1. **Doc/runtime mismatch**: README.md:58 and CHANGELOG.md:82 both claim *"47 shadcn/ui компонентов"*. Reality: **7**. Discrepancy is ~6.7×, not a rounding error. Doc bug, not a code bug.
2. **PolerPanel visual inconsistency**: `src/components/litgraph/PolerPanel.tsx` hardcodes `bg-slate-900 / border-slate-800 / text-slate-200` (dark slate) with **zero** `dark:` prefixes, while `Toolbar.tsx`, `LitApp.tsx`, `NodeEditor.tsx`, `Inspector.tsx` all use `bg-stone-50 / bg-white / border-stone-200` (light theme). PolerPanel always renders dark regardless of theme state.
3. **Dark mode is half-built**: `globals.css` defines a full `.dark { … }` token block (lines 82-114) and `@custom-variant dark (&:is(.dark *))` (line 5), and all 7 primitives include `dark:` variant classes. But **no code anywhere in `src/` ever toggles the `.dark` class** on `<html>`/`<body>` — grep for `setTheme|toggleTheme|document.documentElement.classList|next-themes` returns zero hits. `next-themes` is listed only as a TODO in `docs/PROMPT_PLAN.md:529` ("Тёмная тема (next-themes уже в прототипе)") and is **not** in `package.json`. So the `.dark` token overrides and all `dark:*` utility classes are dead code at runtime.

The 7 primitives are sufficient for what the app currently uses — **there are no broken `@/components/ui/*` imports**. However, several litgraph components hand-roll UI patterns that shadcn primitives would canonicalise (see §5).

---

## 2. shadcn/ui Primitive Inventory (7 files, 230 LOC)

| # | File | LOC | Radix dep | Exports | Notes |
|---|------|-----|-----------|---------|-------|
| 1 | `badge.tsx` | 47 | `@radix-ui/react-slot` (via `asChild`) | `Badge`, `badgeVariants` | 4 variants: default/secondary/destructive/outline. `cva` based. |
| 2 | `button.tsx` | 59 | `@radix-ui/react-slot` | `Button`, `buttonVariants` | 6 variants × 4 sizes (default/sm/lg/icon). Modern shadcn idioms (`has-[>svg]:px-3`, `shadow-xs`). |
| 3 | `dialog.tsx` | 143 | `@radix-ui/react-dialog` | `Dialog`, `DialogTrigger`, `DialogPortal`, `DialogClose`, `DialogOverlay`, `DialogContent`, `DialogHeader`, `DialogFooter`, `DialogTitle`, `DialogDescription` | `DialogContent` adds custom `showCloseButton?: boolean` prop (default true). Uses `XIcon` from lucide-react. |
| 4 | `dropdown-menu.tsx` | 258 | `@radix-ui/react-dropdown-menu` | 16 exports (full set incl. Sub, Checkbox, Radio, Separator, Shortcut) | Most complete primitive; uses `CheckIcon`, `ChevronRightIcon`, `CircleIcon`. |
| 5 | `input.tsx` | 22 | none | `Input` | Plain `<input>`, no Radix. Includes `dark:bg-input/30`, `aria-invalid:*` rings. |
| 6 | `label.tsx` | 25 | `@radix-ui/react-label` | `Label` | Thin wrapper. `peer-disabled:*` + `group-data-[disabled=true]:*` patterns. |
| 7 | `textarea.tsx` | 19 | none | `Textarea` | Plain `<textarea>`. Uses `field-sizing-content` (Tailwind v4 native). |

**All 7 primitives share these properties**:
- Import `cn` from `@/lib/utils` (clsx + tailwind-merge, 6 LOC).
- Use the latest shadcn "new-york" form: function components (not `forwardRef`), `data-slot="…"` attributes, `cva` for variants where applicable.
- Reference design tokens via Tailwind semantic classes (`bg-primary`, `text-muted-foreground`, `border-input`, `ring-ring/50`, etc.) — never raw hex.
- Include `dark:*` variant utilities (e.g. `dark:bg-input/30`, `dark:aria-invalid:ring-destructive/40`) that are currently dead at runtime (see §6).

**Radix dependency mapping** (`package.json`):
- `@radix-ui/react-dialog` ^1.1.4 → used by `dialog.tsx`
- `@radix-ui/react-dropdown-menu` ^2.1.4 → used by `dropdown-menu.tsx`
- `@radix-ui/react-label` ^2.1.1 → used by `label.tsx`
- `@radix-ui/react-slot` ^1.1.1 → used by `button.tsx`, `badge.tsx`
- `badge.tsx`, `input.tsx`, `textarea.tsx` need no Radix (they wrap native elements).

No extra Radix packages are installed — so no Tabs, Select, Tooltip, ScrollArea, Separator, Switch, Checkbox, RadioGroup, Slider, Progress, Skeleton, Popover, HoverCard, Accordion, Collapsible, Avatar, AspectRatio, Sonner/Toast, etc. primitives can exist without `npm install` first.

---

## 3. components.json (shadcn config)

```jsonc
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,                  // ← Not React Server Components (Vite SPA)
  "tsx": true,
  "tailwind": {
    "config": "",                // ← Empty: Tailwind v4 has no JS config
    "css": "src/globals.css",
    "baseColor": "stone",        // ← Stone palette (matches globals.css oklch values)
    "cssVariables": true,
    "prefix": ""
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"           // ← ⚠️ DEAD ALIAS: src/hooks/ does not exist
  },
  "iconLibrary": "lucide"
}
```

**Observations**:
- `style: "new-york"` is the modern shadcn style (vs `default`) — matches the function-component + `data-slot` form observed in the primitives.
- `rsc: false` correctly reflects Vite SPA (no RSC).
- `baseColor: "stone"` matches the light-mode `--background: oklch(1 0 0)` / `--muted: oklch(0.97 0 0)` neutral greige palette.
- `tailwind.config: ""` confirms Tailwind v4 (no `tailwind.config.js/ts` file exists in repo root — verified).
- The `hooks` alias points to `@/hooks` but `src/hooks/` does not exist. If `shadcn add` ever tries to install a primitive that depends on `use-toast` or similar, it will fail. **Cosmetic dead alias.**

---

## 4. Tailwind v4 Setup

### 4.1 Vite plugin wiring (`vite.config.ts`)

```ts
import tailwindcss from "@tailwindcss/vite";
// …
plugins: [react(), tailwindcss()],
```

Tailwind v4 is loaded as a **Vite plugin** (not PostCSS). No `postcss.config.js`, no `tailwind.config.js`. This is the canonical Tailwind v4 + Vite integration.

### 4.2 globals.css entry (`src/globals.css`, 239 LOC)

Imports (lines 1-3):
```css
@import "tailwindcss";
@import "tw-animate-css";                 /* ← shadcn animation utilities (animate-in, fade-in-0, zoom-in-95, …) */
@import "@xyflow/react/dist/style.css";   /* ← React Flow base CSS */
```

Dark variant declaration (line 5):
```css
@custom-variant dark (&:is(.dark *));
```

### 4.3 Custom design tokens (`@theme inline { … }`, lines 7-45)

The `@theme inline` block maps Tailwind v4 colour utilities to CSS variables. ~38 token mappings:

- **Core**: `--color-background`, `--color-foreground`, `--color-card`, `--color-card-foreground`, `--color-popover`, `--color-popover-foreground`, `--color-primary`, `--color-primary-foreground`, `--color-secondary`, `--color-secondary-foreground`, `--color-muted`, `--color-muted-foreground`, `--color-accent`, `--color-accent-foreground`, `--color-destructive`, `--color-border`, `--color-input`, `--color-ring`.
- **Chart palette**: `--color-chart-1` … `--color-chart-5` (5 distinct hues).
- **Sidebar**: `--color-sidebar`, `--color-sidebar-foreground`, `--color-sidebar-primary`, `--color-sidebar-primary-foreground`, `--color-sidebar-accent`, `--color-sidebar-accent-foreground`, `--color-sidebar-border`, `--color-sidebar-ring` (8 sidebar tokens; **not used anywhere in `src/`** — no `<Sidebar>` shadcn primitive exists, the project's `Sidebar.tsx` is a custom component).
- **Fonts**: `--font-sans: var(--font-geist-sans)`, `--font-mono: var(--font-geist-mono)` — **but `--font-geist-sans` / `--font-geist-mono` are never defined anywhere** (no `next/font` import, no `@font-face` rule, no Geist font package in `package.json`). Dead tokens — `--font-sans`/`--font-mono` will resolve to `unset` and the browser will fall back to default sans-serif.
- **Radii**: `--radius: 0.625rem` (10px) in `:root`; `--radius-sm/md/lg/xl` derived via `calc(var(--radius) ∓ Npx)`.

### 4.4 Token values (`:root` light, lines 47-80; `.dark` dark, lines 82-114)

All colours use **oklch** colour space (Tailwind v4 default). Light mode is a stone/neutral greyscale:
- `--background: oklch(1 0 0)` (white)
- `--foreground: oklch(0.145 0 0)` (near-black)
- `--primary: oklch(0.205 0 0)` (near-black, high-contrast)
- `--muted-foreground: oklch(0.556 0 0)` (mid-grey)
- `--border: oklch(0.922 0 0)` (light grey)
- `--destructive: oklch(0.577 0.245 27.325)` (red)
- `--chart-1`..`5`: orange / teal / blue / yellow / amber

Dark mode mirrors with inverted luminance:
- `--background: oklch(0.145 0 0)`, `--foreground: oklch(0.985 0 0)`, etc.
- `--border: oklch(1 0 0 / 10%)` (translucent white)
- `--input: oklch(1 0 0 / 15%)`

### 4.5 Domain-specific CSS (lines 116-238)

After the shadcn token block, `globals.css` carries substantial **app-specific styling** (123 LOC, 52% of file):
- `.lit-canvas-bg` (light + `.dark` variants): parchment background with radial gradients + 45° repeating linear-gradient hatching (lines 125-152). Hex values `#f5efe1` (light parchment) / `#1a1612` (dark parchment) — **raw hex, not oklch tokens** (inconsistent with the rest of the design system, but justified because these are decorative noise layers, not semantic surfaces).
- `.react-flow__*` overrides (lines 154-211): hides attribution, customises edge stroke widths, handle styling, controls button colours (`#5b4636` warm brown, `#faf6ec` parchment), minimap border. Light + `.dark` variants.
- `.lit-scroll` scrollbar (lines 213-227): 8px thumb, warm brown rgba(139, 110, 70, 0.3).
- `@keyframes lit-node-pop` + `.lit-node-enter` (lines 229-238): 0.25s scale 0.85→1.04→1 pop animation for new graph nodes.

**All domain CSS uses warm-brown/parchment palette** (`#5b4636`, `#f5efe1`, `rgba(180, 140, 80, …)`) — this is the **literary "parchment" theme** identity, distinct from the neutral stone shadcn base. The two palettes coexist without conflict because parchment CSS targets `.lit-*` and `.react-flow__*` selectors, while shadcn tokens target semantic surfaces.

---

## 5. Missing shadcn Components (critical focus #1)

### 5.1 What the README claims vs reality

| Source | Claimed count | Actual count | Discrepancy |
|--------|--------------|--------------|-------------|
| `README.md:58` | "47 shadcn/ui компонентов" | 7 | **−40 (−85%)** |
| `CHANGELOG.md:82` | "47 shadcn/ui компонентов" | 7 | **−40 (−85%)** |
| `src/components/ui/` (ls) | — | 7 | ground truth |

### 5.2 No broken imports

A grep across `src/` for `from "@/components/ui/<x>"` shows **only** imports of the 7 existing primitives. No component imports `@/components/ui/tabs`, `/card`, `/select`, `/tooltip`, `/scroll-area`, `/separator`, `/switch`, `/toast`, `/sonner`, `/progress`, `/skeleton`, `/sheet`, `/popover`, `/accordion`, etc. **The codebase does not break**, it just hand-rolls the missing patterns.

### 5.3 Hand-rolled patterns that should be shadcn primitives

These are the **highest-value missing primitives**, ranked by how much hand-rolled duplication they would replace:

| # | Primitive | Where hand-rolled | Lines | What it would replace |
|---|-----------|-------------------|-------|------------------------|
| 1 | **`tabs`** | `PolerPanel.tsx:223-260`, `PolerPanel.tsx:543` | ~40 LOC | Two manual tab bars (`flex bg-slate-800/80 p-1 rounded-lg` + per-tab active/inactive class toggle). Currently uses local `useState` + `className={… ? active : "text-slate-400 hover:text-slate-200"}`. |
| 2 | **`card`** | `PolerPanel.tsx:298-401` (6 instances), `Inspector.tsx`, `NodeEditor.tsx:203`, `ReasoningDialog.tsx:135` | ~80 LOC | Repeated `<div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">` pattern (dark) and `rounded-md border bg-white p-2.5` (light). CardHeader/CardContent/CardTitle would canonicalise. |
| 3 | **`select`** | `AIDialog.tsx:196-205` | ~10 LOC | Native `<select className="w-full h-9 rounded-md border border-stone-200 bg-white px-2 text-sm">` with 4 `<option>`s (Полный анализ / Только сюжет / Только персонажи / Только темп и ритм). Inconsistent with the styled DropdownMenu used elsewhere. |
| 4 | **`separator`** | `Toolbar.tsx:419`, `Toolbar.tsx:617`, `NodeActions.tsx:90`, `NodeActions.tsx:124` | ~8 LOC | Hand-rolled `<div className="h-6 w-px bg-stone-200 mx-1 hidden sm:block" />` vertical separators and `<div className="border-t border-stone-100 my-1" />` horizontal separators. |
| 5 | **`tooltip`** | `Toolbar.tsx` (entire file, no tooltips on icon-only buttons) | — | Icon-only Buttons (`variant="ghost" size="icon"`) have no accessible label hover. Tooltip primitive would fix UX + a11y. |
| 6 | **`progress`** | `TextMomentsDialog.tsx:338`, `PolerPanel.tsx:416-440`, `PolerPanel.tsx:621` | ~15 LOC | Hand-rolled `<div className="w-16 h-1.5 rounded-full bg-stone-100 overflow-hidden"><div style={{width: `${pct}%`}} className="bg-violet-500 h-full" /></div>`. |
| 7 | **`scroll-area`** | `globals.css:213-227` (`.lit-scroll`), used in `NodeEditor.tsx:203`, `Inspector.tsx`, `PolerPanel.tsx:271` | ~15 LOC | Custom scrollbar CSS + `overflow-y-auto lit-scroll` utility. Could be replaced by Radix ScrollArea for cross-browser consistency. |
| 8 | **`sonner` (toast)** | (absent) | — | No toast/notification system. AI command failures, project save success, export completion etc. have **no user feedback channel** except console.log. Adding `sonner` would close this gap. |
| 9 | **`skeleton`** | (absent) | — | Loading states use `<div className="w-5 h-5 border-2 border-purple-500 border-t-transparent rounded-full animate-spin" />` (PolerPanel.tsx:287). No skeleton placeholders for async content. |

**Lower-priority missing primitives** (no current hand-rolled equivalent, but would be useful if features expand):
- `switch` — no toggle UI exists (could be used for dark-mode toggle if implemented)
- `checkbox` / `radio-group` — no forms using these
- `slider` — no range inputs
- `accordion` — no collapsible sections
- `popover` / `hover-card` — could replace some Dialogs that are read-only
- `sheet` — could replace Dialog for full-height side panels (`Inspector.tsx`, `Sidebar.tsx`)
- `alert` / `alert-dialog` — no destructive-action confirmation dialogs (deleting nodes happens via `NodeActions.tsx` without confirmation)

### 5.4 The `47` claim — what might it have meant?

The README/CHANGELOG claim of "47" does not correspond to any verifiable artefact:
- 7 shadcn primitives in `src/components/ui/`
- 23 components in `src/components/litgraph/`
- 7 + 23 = 30 total `.tsx` components (still not 47)
- Adding all `src/lib/*.ts` modules (14 files) → 44 (close to 47 but they are not "components")
- Possibly the author counted planned/imagined shadcn primitives that were never generated.

**Recommendation**: Update README.md:58 and CHANGELOG.md:82 from "47 shadcn/ui компонентов" to "7 shadcn/ui примитивов + 23 кастомных компонентов" (or whatever the team considers accurate). Doc fix only, no code impact.

---

## 6. Dark Mode (critical focus #3)

### 6.1 Plumbing exists, switching logic absent

| Layer | Status | Evidence |
|-------|--------|----------|
| CSS variant | ✅ Defined | `globals.css:5` — `@custom-variant dark (&:is(.dark *))` |
| Dark tokens | ✅ Defined | `globals.css:82-114` — full `.dark { … }` block mirroring `:root` |
| Primitive `dark:*` utilities | ✅ Present | All 7 primitives include `dark:bg-input/30`, `dark:aria-invalid:ring-destructive/40`, etc. |
| Domain CSS `.dark` overrides | ✅ Present | `globals.css:140-152` (`.dark .lit-canvas-bg`), `globals.css:204-211` (`.dark .react-flow__controls-button`) |
| Theme toggle (UI button) | ❌ Missing | No `<Button onClick={toggleTheme}>` in `Toolbar.tsx` or anywhere |
| Theme state (useState/store) | ❌ Missing | No `theme` field in `src/lib/litgraph/store.ts` (verified by absence of grep hits) |
| Class application | ❌ Missing | No `document.documentElement.classList.add("dark")` / `remove("dark")` anywhere |
| Theme package | ❌ Missing | `next-themes` is **not** in `package.json` deps (despite `docs/PROMPT_PLAN.md:529` saying "next-themes уже в прототипе") |

**Conclusion**: Dark mode is **declaratively complete** (all CSS is in place) but **imperatively unreachable** (no toggle, no state, no class application). At runtime the app is **light-only**. The `dark:*` utilities in primitives are dead code; the `.dark` token block in globals.css is dead code; the `.dark .lit-canvas-bg` and `.dark .react-flow__*` overrides are dead code.

### 6.2 PolerPanel visual inconsistency

`PolerPanel.tsx` is the **only** component in the codebase that renders a dark UI in light mode:

| Component | Theme | Sample className |
|-----------|-------|------------------|
| `LitApp.tsx:10` | light | `bg-stone-50` |
| `Toolbar.tsx:407` | light | `bg-white border-b border-stone-200` |
| `LitNodeView.tsx:38` | light | `bg-white shadow-md` |
| `NodeEditor.tsx:203` | light | `bg-white border border-stone-200` |
| `Inspector.tsx` | light | (uses `bg-white` via shadcn tokens) |
| `NodeActions.tsx:86` | light | `bg-white rounded-lg shadow-xl border border-stone-200` |
| `NerDialog.tsx:249`, `312` | light | `bg-white rounded border-l-2` |
| `TextMomentsDialog.tsx:308` | light | `border border-stone-200 bg-white` |
| `PolerPanel.tsx:202` | **DARK (hardcoded)** | `bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl` |
| `PolerPanel.tsx:204` | dark | `bg-slate-900/50` (header) |
| `PolerPanel.tsx:271` | dark | `bg-slate-950/40` (body) |
| `PolerPanel.tsx:298` (×6) | dark | `bg-slate-900/80 border border-slate-800` (cards) |
| `PolerPanel.tsx:771` | dark | `bg-slate-900/50 border-t border-slate-800` (footer) |

**Total slate-* hardcoded classes in PolerPanel.tsx**: ~70 instances (verified by grep).

**Why this is a problem**:
1. **Visual inconsistency**: PolerPanel opens as a modal that looks like a different application. User perception: "Why does this one panel look like a dark IDE while everything else is light parchment?"
2. **No `dark:` prefix**: The slate palette is applied unconditionally, so even if a future dark-mode toggle is wired up, PolerPanel **will not respond** to it — it would stay slate-900 in dark mode (which is correct for dark, but it would also be slate-900 in light mode, which is the current bug).
3. **Token divergence**: PolerPanel uses `slate` (blue-grey, oklch hue ~250) while the rest of the app uses `stone` (warm neutral, oklch hue 0). The two palettes are perceptually different — slate reads as "tech/IDE", stone reads as "literary/parchment".
4. **Inconsistent with shadcn tokens**: Instead of `bg-popover` / `bg-card` / `text-muted-foreground` (which would automatically adapt to dark mode), PolerPanel uses raw `bg-slate-900` / `text-slate-400` etc., bypassing the entire token system.

**Recommendation**: Refactor `PolerPanel.tsx` to use shadcn tokens (`bg-background`, `bg-card`, `text-muted-foreground`, `border-border`) instead of raw `slate-*` classes. This (a) fixes the visual inconsistency in light mode, (b) makes PolerPanel respond correctly if/when dark mode is wired up, (c) reduces ~70 hardcoded class references to ~10 token references.

---

## 7. Build Config (critical focus #4)

### 7.1 `vite.config.ts` (32 LOC)

```ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

// @ts-expect-error process is node
const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),   // ← Path alias synced with tsconfig
    },
  },
  clearScreen: false,
  server: {
    port: 1420,                                // ← Tauri's expected dev port
    strictPort: true,                          // ← Fails if 1420 is taken (Tauri requirement)
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],            // ← Avoid Rust-triggered HMR loops
    },
  },
}));
```

**Aliases**: `@` → `./src` (only one). Matches `tsconfig.json` `paths`. ✅ Synced.

**Tauri-specific wiring**:
- `TAURI_DEV_HOST` env var support for mobile/remote dev (line 7, 20-26).
- Port 1420 + `strictPort: true` — Tauri's `tauri dev` expects Vite on exactly this port.
- HMR on port 1421 (WS protocol) when `TAURI_DEV_HOST` is set.
- `clearScreen: false` — Tauri CLI controls the terminal.
- `watch.ignored: ["**/src-tauri/**"]` — Rust changes go through cargo, not Vite.

### 7.2 `tsconfig.json` (25 LOC)

```jsonc
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "baseUrl": ".",
    "paths": { "@/*": ["./src/*"] }
  },
  "include": ["src"],
  "references": [{ "path": "./tsconfig.node.json" }]
}
```

**Strictness flags**: `strict`, `noUnusedLocals`, `noUnusedParameters`, `noFallthroughCasesInSwitch` — all enabled. **No `any` types permitted implicitly**, no unused imports allowed. This is stricter than the typical shadcn starter.

**`allowImportingTsExtensions: true`** + **`noEmit: true`** — permits `import x from "./foo.ts"` (with explicit `.ts` extension). Not used by the shadcn primitives (they use extensionless imports), but enabled for flexibility.

**`moduleResolution: "bundler"`** — Vite-compatible, allows `package.json` "exports" resolution.

### 7.3 `tsconfig.node.json` (10 LOC)

Composite project for `vite.config.ts` only:
```jsonc
{
  "compilerOptions": {
    "composite": true,
    "skipLibCheck": true,
    "module": "ESNext",
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true
  },
  "include": ["vite.config.ts"]
}
```

Standard Vite + TS project-references pattern. ✅ Correct.

### 7.4 `package.json` (49 LOC)

**Notable dependencies**:
- `react` / `react-dom` `^19.0.0` — React 19 (latest stable, concurrent renderer).
- `@xyflow/react` `^12.11.2` — React Flow v12 (graph canvas).
- `zustand` `^5.0.6` — state management (store at `src/lib/litgraph/store.ts`).
- `lucide-react` `^0.525.0` — icon library (matches `components.json` `"iconLibrary": "lucide"`).
- `class-variance-authority` `^0.7.1` — used by `button.tsx`, `badge.tsx` for `cva`.
- `clsx` `^2.1.1` + `tailwind-merge` `^3.3.1` — used by `src/lib/utils.ts` `cn()`.
- `html-to-image` `^1.11.13` + `jspdf` `^4.2.1` — PDF/PNG export pipeline.
- `utif` `^3.1.0` — TIFF decoder (type shim in `src/types/utif.d.ts`).
- 4 Tauri packages: `@tauri-apps/api`, `plugin-dialog`, `plugin-fs`, `plugin-store`.

**Dev dependencies**:
- `tailwindcss` `^4`, `@tailwindcss/vite` `^4` — Tailwind v4.
- `tw-animate-css` `^1.3.5` — shadcn animation utility pack (imported in globals.css:2).
- `vite` `^6.0.0`, `@vitejs/plugin-react` `^4.3.0`.
- `typescript` `^5`, `eslint` `^9`.
- `@tauri-apps/cli` `^2.11.4` — Tauri build CLI.

**Scripts**: `dev`, `build` (`tsc && vite build`), `preview`, `tauri`, `lint`.

**Missing deps if missing primitives are added**:
- `tabs` → needs `@radix-ui/react-tabs`
- `select` → needs `@radix-ui/react-select`
- `tooltip` → needs `@radix-ui/react-tooltip`
- `scroll-area` → needs `@radix-ui/react-scroll-area`
- `separator` → needs `@radix-ui/react-separator`
- `progress` → needs `@radix-ui/react-progress`
- `skeleton` → no Radix dep (pure CSS)
- `sonner` → needs `sonner` package + (optionally) `next-themes`
- `card` → no Radix dep (pure div composition)

---

## 8. App Entry (critical focus #5)

### 8.1 Entry chain

```
index.html  ──┐
              ↓
src/main.tsx  ──→  ReactDOM.createRoot(...).render(
                     <React.StrictMode>
                       <App />          ← imports "./globals.css" here
                     </React.StrictMode>
                   )
              ↓
src/App.tsx   ──→  function App() {
                     return (
                       <ReactFlowProvider>   ← from @xyflow/react
                         <LitApp />          ← the real UI shell
                       </ReactFlowProvider>
                     );
                   }
              ↓
src/components/litgraph/LitApp.tsx  ──→  function LitApp() {
                     return (
                       <div className="h-screen w-screen flex flex-col overflow-hidden bg-stone-50">
                         <Toolbar />
                         <div className="flex-1 flex overflow-hidden">
                           <LitCanvas />
                           <Sidebar />
                         </div>
                         <NodeEditor />
                       </div>
                     );
                   }
```

### 8.2 Why the split?

- **`main.tsx`** (10 LOC): Standard React 19 entry. Mounts `<App/>` into `#root`, wraps in `<React.StrictMode>`. Imports `./globals.css` once globally so Tailwind processes it.
- **`App.tsx`** (12 LOC): Thin wrapper whose **only job** is to install the `<ReactFlowProvider>` context. React Flow v12 requires a provider ancestor for any `<ReactFlow>` component (used in `LitCanvas.tsx`). Keeping this in a separate file from `LitApp.tsx` is a clean separation: provider vs layout.
- **`LitApp.tsx`** (19 LOC): The real application shell. Vertical flexbox: `<Toolbar/>` (top), `<LitCanvas/> + <Sidebar/>` (middle, horizontal flex), `<NodeEditor/>` (modal, rendered last but positioned fixed). Root `<div>` is `h-screen w-screen overflow-hidden bg-stone-50`.

**Naming clarity**: `App.tsx` vs `LitApp.tsx` is a deliberate convention — `App` is the framework entry (provider wiring), `LitApp` is the domain entry (the actual LitGraph layout). This is a reasonable pattern; no rename needed.

### 8.3 Type definitions folder (`src/types/`)

Contains exactly **one file**: `utif.d.ts` (32 LOC). It is a hand-written ambient module declaration for the `utif` TIFF decoder package (which ships without TypeScript types). Declares `IFD` interface, `decode()`, `decodeImage()`, `toRGBA8()` functions, and a default export. **Not a design-system concern** — included only for completeness. No other `*.d.ts` files in `src/`.

---

## 9. Atomic Findings (consolidated)

### F1 — shadcn/ui count discrepancy (doc bug, low severity)
**Where**: `README.md:58`, `CHANGELOG.md:82` claim "47 shadcn/ui компонентов".
**Reality**: 7 primitives in `src/components/ui/`.
**Impact**: Documentation misleads users/contributors about the maturity of the UI layer.
**Fix**: Replace "47" with "7" (or "7 + 23 кастомных") in both files. Pure doc fix.
**Effort**: 2 lines.

### F2 — PolerPanel palette inconsistency (visual bug, medium severity)
**Where**: `src/components/litgraph/PolerPanel.tsx` (~70 `slate-*` class references).
**Reality**: PolerPanel hardcodes a dark slate palette unconditionally; all other components use light stone tokens.
**Impact**: PolerPanel visually breaks the light parchment theme. Will also fail to respond to a future dark-mode toggle.
**Fix**: Replace `bg-slate-900/800/950`, `border-slate-700/800`, `text-slate-200/300/400/500` with `bg-background`, `bg-card`, `border-border`, `text-foreground`, `text-muted-foreground`, `text-muted-foreground/70`, etc. (~70 replacements).
**Effort**: ~30 min, low risk (token swap, no logic change).

### F3 — Dark mode is plumbed but never toggled (dead code, medium severity)
**Where**: `globals.css:5` (`@custom-variant dark`), `globals.css:82-114` (`.dark` tokens), `globals.css:140-152` (`.dark .lit-canvas-bg`), `globals.css:204-211` (`.dark .react-flow__*`), and `dark:*` utilities in all 7 primitives.
**Reality**: No `setTheme`/`toggleTheme`, no `document.documentElement.classList.toggle("dark")`, no `next-themes` install. `docs/PROMPT_PLAN.md:529` lists dark mode as a TODO.
**Impact**: ~150 LOC of dark-mode CSS is dead at runtime. Users cannot switch themes.
**Fix (minimum)**: Add a theme toggle in `Toolbar.tsx` that calls `document.documentElement.classList.toggle("dark")` and persists to `localStorage`. (~15 LOC)
**Fix (proper)**: Install `next-themes`, wrap `<App/>` in `<ThemeProvider attribute="class">`, add toggle button. (~30 LOC + 1 dep)
**Effort**: 30 min (minimum) / 1 hour (proper).

### F4 — Dead `hooks` alias in components.json (cosmetic, low severity)
**Where**: `components.json` `aliases.hooks: "@/hooks"`.
**Reality**: `src/hooks/` directory does not exist.
**Impact**: If `npx shadcn@latest add` is used to install a primitive that depends on a `use-toast` hook, generation will fail. No current code uses this alias.
**Fix**: Either remove the `hooks` alias from `components.json`, or create `src/hooks/` with a placeholder. Trivial.
**Effort**: 1 line.

### F5 — Dead Geist font tokens (cosmetic, low severity)
**Where**: `globals.css:10-11` — `--font-sans: var(--font-geist-sans)`, `--font-mono: var(--font-geist-mono)`.
**Reality**: `--font-geist-sans` / `--font-geist-mono` are never defined (no `next/font` import, no `@font-face`, no `geist` package in `package.json`).
**Impact**: `--font-sans` and `--font-mono` resolve to `unset`; the app falls back to the browser default sans-serif. No visible bug, but the intent (Geist font) is silently lost.
**Fix**: Either (a) install `geist` package and import it, or (b) replace `var(--font-geist-sans)` with a concrete font stack like `ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, sans-serif`.
**Effort**: 5 min.

### F6 — Unused sidebar token block (dead code, low severity)
**Where**: `globals.css:12-19` (8 `--color-sidebar-*` mappings) + `globals.css:72-79` (`:root` sidebar values) + `globals.css:106-113` (`.dark` sidebar values) = ~30 LOC.
**Reality**: The project's `Sidebar.tsx` is a custom component (not the shadcn `sidebar` primitive) and does not use `bg-sidebar`, `text-sidebar-foreground`, etc. tokens.
**Impact**: ~30 LOC of unused CSS. No runtime cost (CSS variables cost ~0 bytes when unused).
**Fix**: Optional — remove the sidebar token block. Or keep it for future use if a shadcn `sidebar` primitive is planned.
**Effort**: 5 min if removing.

### F7 — Native `<select>` in AIDialog (UX inconsistency, low severity)
**Where**: `AIDialog.tsx:196-205` — `<select className="w-full h-9 rounded-md border border-stone-200 bg-white px-2 text-sm">` with 4 `<option>`s.
**Reality**: All other dropdown UI uses the styled `DropdownMenu` primitive. The native `<select>` looks visually inconsistent (browser-default styling on Windows/Linux can render a non-parchment dropdown list).
**Fix**: Replace with shadcn `Select` primitive (`@radix-ui/react-select`), or convert to a `DropdownMenu` with radio items.
**Effort**: 15 min + 1 Radix dep.

### F8 — Hand-rolled Tab bars in PolerPanel (technical debt, low severity)
**Where**: `PolerPanel.tsx:223-260` (analytics tabs), `PolerPanel.tsx:543` (sub-tabs).
**Reality**: Two manual tab implementations using `useState` + conditional `className`. Not keyboard-accessible (no arrow-key navigation, no `role="tab"`).
**Fix**: Replace with shadcn `Tabs` primitive (`@radix-ui/react-tabs`). Adds a11y for free.
**Effort**: 20 min + 1 Radix dep.

### F9 — Hand-rolled Progress bars (technical debt, low severity)
**Where**: `TextMomentsDialog.tsx:338`, `PolerPanel.tsx:416-440`, `PolerPanel.tsx:621`.
**Reality**: 3 inline progress-bar implementations with different colours (`bg-violet-500`, `bg-amber-400`, `bg-slate-700`).
**Fix**: Replace with shadcn `Progress` primitive (`@radix-ui/react-progress`).
**Effort**: 15 min + 1 Radix dep.

### F10 — Hand-rolled card divs (technical debt, low severity)
**Where**: `PolerPanel.tsx:298-401` (6 cards), `Inspector.tsx`, `NodeEditor.tsx:203`, `ReasoningDialog.tsx:135`.
**Reality**: Repeated `<div className="bg-white border border-stone-200 p-4 rounded-lg">` (light) and `<div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">` (dark, PolerPanel).
**Fix**: Replace with shadcn `Card` / `CardHeader` / `CardContent` / `CardTitle` (no Radix dep needed; pure div composition).
**Effort**: 30 min.

### F11 — Hand-rolled separators (technical debt, trivial)
**Where**: `Toolbar.tsx:419`, `Toolbar.tsx:617`, `NodeActions.tsx:90`, `NodeActions.tsx:124`.
**Reality**: 4 inline `<div className="h-6 w-px bg-stone-200">` or `<div className="border-t border-stone-100">` separators.
**Fix**: Replace with shadcn `Separator` primitive (`@radix-ui/react-separator`).
**Effort**: 10 min + 1 Radix dep.

### F12 — No toast/notification system (UX gap, medium severity)
**Where**: Absent.
**Reality**: No way to give the user non-blocking feedback. AI command failures, project save success, export completion, parse errors all silently log to console. The only feedback channel is the `ReasoningDialog.tsx` which is blocking and verbose.
**Fix**: Add `sonner` package (`npm i sonner`), add `<Toaster/>` in `App.tsx`, call `toast.success(...)` / `toast.error(...)` from command handlers.
**Effort**: 30 min + 1 dep.

---

## 10. Dependency Graph & Blockers

```
README/CHANGELOG doc fix (F1) ──────► no blocker, pure text edit
PolerPanel token refactor (F2) ─────► no blocker, but should be done BEFORE dark-mode toggle (F3) so PolerPanel responds to dark mode
Dark mode toggle (F3) ──────────────► blocked by F2 (PolerPanel would stay dark in light mode if toggle added first... actually F2 + F3 are independent, but doing F2 first ensures PolerPanel benefits from F3)
Native <select> → ui/select (F7) ───► blocked by `npm i @radix-ui/react-select`
Hand-rolled tabs → ui/tabs (F8) ─────► blocked by `npm i @radix-ui/react-tabs`
Hand-rolled progress → ui/progress (F9) ► blocked by `npm i @radix-ui/react-progress`
Hand-rolled cards → ui/card (F10) ──► no blocker (pure composition, no Radix)
Hand-rolled separators → ui/separator (F11) ► blocked by `npm i @radix-ui/react-separator`
Toast system (F12) ────────────────► blocked by `npm i sonner`
Geist font (F5) ──────────────────► blocked by `npm i geist` OR replace with system font stack
hooks alias (F4) ─────────────────► no blocker, pure config edit
sidebar tokens (F6) ──────────────► no blocker, pure CSS edit
```

---

## 11. Recommended Next Actions (prioritised)

### P0 — Doc accuracy (do today)
1. **Fix README.md:58 and CHANGELOG.md:82** — change "47 shadcn/ui компонентов" to "7 shadcn/ui примитивов + 23 кастомных компонента" (or accurate count). [F1, 2 lines]

### P1 — Visual consistency (do this week)
2. **Refactor `PolerPanel.tsx`** to use shadcn tokens instead of raw `slate-*` classes. ~70 class replacements. [F2, ~30 min]
3. **Fix Geist font dead tokens** in `globals.css:10-11` — either install `geist` package or replace `var(--font-geist-sans)` with a concrete system font stack. [F5, 5 min]

### P2 — Dark mode (do this sprint)
4. **Install `next-themes`**, wrap `<App/>` in `<ThemeProvider attribute="class">`, add a theme-toggle button in `Toolbar.tsx`. This activates the existing `dark:*` CSS (150 LOC currently dead). [F3, ~1 hour]
5. **Remove dead `hooks` alias** from `components.json` OR create `src/hooks/` placeholder. [F4, 1 line]

### P3 — Missing primitives (do as needed)
6. **Add `ui/separator`** — lowest-cost primitive (no Radix dep beyond `@radix-ui/react-separator`), replaces 4 hand-rolled separators. [F11]
7. **Add `ui/card`** — pure div composition, no Radix dep, replaces ~10 hand-rolled card divs. [F10]
8. **Add `ui/select`** — replaces native `<select>` in `AIDialog.tsx`. [F7]
9. **Add `ui/tabs`** — replaces 2 hand-rolled tab bars in `PolerPanel.tsx`, adds a11y. [F8]
10. **Add `ui/progress`** — replaces 3 hand-rolled progress bars. [F9]
11. **Add `ui/sonner` (toast)** — closes the user-feedback gap. [F12]

### P4 — Optional cleanup
12. **Remove unused sidebar token block** in `globals.css` (~30 LOC) if no shadcn `sidebar` primitive is planned. [F6]

---

## 12. Cross-References to Other Subagents

- **Subagent 1 (analysis)**: Already noted the "47→7" discrepancy in the master analysis report (worklog.md line 18). This report confirms and quantifies it (F1).
- **Subagent on PolerPanel/Toolbar/AIDialog (consumer subagents)**: F2, F7, F8 are actionable from those components' perspectives — they should be flagged in their respective reports.
- **Subagent on store/state**: Confirmed no `theme` field in `src/lib/litgraph/store.ts` (F3 blocker). If dark mode is added, the store should hold `theme: "light" | "dark"`.
- **Subagent on Tauri commands**: F12 (toast) is most impactful for Tauri command handlers — they currently have no user feedback channel on success/failure.

---

## 13. Summary Table

| Area | Status | Score |
|------|--------|-------|
| shadcn primitive quality | ✅ Modern, idiomatic, latest Tailwind v4 patterns | 9/10 |
| Tailwind v4 setup | ✅ Correct (@tailwindcss/vite + @theme inline + oklch tokens) | 9/10 |
| Path aliases | ✅ Synced between tsconfig and vite.config | 10/10 |
| App entry structure | ✅ Clean separation (App=provider, LitApp=layout) | 9/10 |
| Dark mode plumbing | ⚠️ CSS complete, toggle missing | 4/10 |
| PolerPanel visual consistency | ❌ Hardcoded dark slate in light app | 2/10 |
| Documentation accuracy | ❌ "47 components" claim is 6.7× inflated | 2/10 |
| Missing primitives | ⚠️ 9 hand-rolled patterns could be canonicalised | 5/10 |
| Token system coherence | ⚠️ Geist font tokens dead, sidebar tokens unused | 6/10 |

**Overall design-system health**: 6.2/10. Mechanically sound primitives, but the design system is **incompletely applied** — PolerPanel bypasses it, dark mode is half-built, several patterns are hand-rolled instead of using shadcn primitives, and the documentation overstates coverage by 6.7×.
