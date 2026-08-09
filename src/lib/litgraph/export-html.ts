/**
 * export-html.ts
 * ==============
 * Фаза A-v2 (Centaur Manifest, Task ID 13):
 *   Интерактивный HTML X-ray экспорт рабочего стола LitGraph.
 *
 * Чем это отличается от SVG X-ray:
 *   SVG — это "фотка". Ты видишь ветки визуально, но не можешь кликнуть,
 *   не можешь посмотреть meta конкретной ноды, не можешь развернуть SVO.
 *   HTML X-ray — это **мини-программа**: самодостаточный .html файл,
 *   который открывается в любом браузере и работает как GUI рабочего стола:
 *     - Canvas с pan/zoom (drag, wheel)
 *     - Click по ноде → sidebar показывает full meta + reason + SVO + J
 *     - Click по ребру → sidebar показывает edge reason + SVO triples
 *     - Hover → tooltip с краткой информацией
 *     - Search по title/body/tags
 *     - Toggle фона + opacity slider
 *     - Keyboard: F=fit, +/- zoom, arrows pan, Esc deselect
 *
 * Data flow:
 *   1. exportWorkspaceToHtml(nodes, edges, bg, viewport, opts) → string HTML
 *   2. JSON всех данных встраивается в <script type="application/json" id="litgraph-data">
 *   3. Background image — уже data: URL (base64), встраивается как есть в JSON
 *   4. Все JS/CSS inline — один файл, никаких внешних зависимостей
 *
 * Что видит AI/разработчик в файле:
 *   - Открой в браузере → интерактивный GUI
 *   - Открой в текстовом редакторе → видишь JSON со всеми meta/reason/SVO
 *   - Ctrl+U → исходник, читаемый, с комментариями
 *
 * Лицензия: MIT (часть LitGraph).
 */

import type { LitNode, LitEdge, BackgroundLayer, EdgeKind } from "./types";
import { NODE_TYPES, EDGE_TYPES } from "./types";
import { analyzeWorkspace, type NodeDiagnostic } from "./heuristics";

// ====== Константы (используются внутри HTML template как inline JS, см. ниже) ======
// NODE_WIDTH = 260, NODE_BASE_HEIGHT = 70, NODE_MAX_HEIGHT = 140
// nodeHeight() — дублирован внутри HTML template как JS (string), чтобы
// мини-программа была самодостаточной. Здесь константы не нужны.

// ====== Сбор reason-строк (дублирует export-svg.ts, чтобы был независимым модулем) ======

function buildNodeReason(node: LitNode): string {
  const meta = node.data.meta ?? {};
  const parts: string[] = [];
  switch (node.type) {
    case "chapter": {
      if (meta.wordCount !== undefined) parts.push(`words=${meta.wordCount}`);
      if (meta.epsilon !== undefined) parts.push(`ε=${meta.epsilon}`);
      if (meta.emotion !== undefined) parts.push(`emotion=${meta.emotion}`);
      if (meta.uniqueWords !== undefined) parts.push(`unique=${meta.uniqueWords}`);
      if (meta.characters) parts.push(`chars=[${meta.characters}]`);
      if (meta.locations) parts.push(`locs=[${meta.locations}]`);
      if (Array.isArray(node.data.tags) && node.data.tags.length > 0) {
        parts.push(`tags=[${node.data.tags.join(",")}]`);
      }
      return parts.length ? `chapter:${parts.join(";")}` : "chapter:manual";
    }
    case "character": {
      // v0.3.0: если парсер предоставил полный reason (с speech_verb_hits,
      // direct_address_hits, prefix, NOT_IN_STOPLIST) — используем его.
      // Это даёт AI/developer полную трассировку решения парсера.
      if (meta.reason && typeof meta.reason === "string") {
        return meta.reason;
      }
      // Fallback для нод, созданных вручную или старым парсером
      if (meta.mentions !== undefined) parts.push(`freq=${meta.mentions}`);
      if (meta.speechCount !== undefined) parts.push(`speech=${meta.speechCount}`);
      if (meta.directCount !== undefined) parts.push(`direct=${meta.directCount}`);
      if (meta.chapters) parts.push(`in=${meta.chapters}`);
      if (meta.firstChapter) parts.push(`first=${meta.firstChapter}`);
      if (node.data.body) {
        const formsMatch = node.data.body.match(/Формы:\s*([^.]+)/);
        if (formsMatch) parts.push(`aliases=[${formsMatch[1].trim()}]`);
      }
      return parts.length ? `character:${parts.join(";")}` : "character:manual";
    }
    case "location": {
      if (meta.mentions !== undefined) parts.push(`freq=${meta.mentions}`);
      if (meta.chapters) parts.push(`in=${meta.chapters}`);
      if (meta.firstChapter) parts.push(`first=${meta.firstChapter}`);
      return parts.length ? `location:${parts.join(";")}` : "location:manual";
    }
    case "scene":
      return (
        `scene:${meta.pov ? `pov=${meta.pov};` : ""}${meta.mood ? `mood=${meta.mood};` : ""}${
          meta.timeOfDay ? `time=${meta.timeOfDay}` : ""
        }`.replace(/;$/, "") || "scene:manual"
      );
    case "plotpoint":
      return `plotpoint:${meta.importance ? `imp=${meta.importance}` : "imp=?"}`;
    case "conflict":
      return `conflict:${meta.importance ? `imp=${meta.importance}` : "imp=?"}`;
    case "dialogue":
      return "dialogue:manual";
    case "theme":
      return (
        `theme:${meta.importance ? `imp=${meta.importance};` : ""}${
          meta.manifestation ? `manifest=${String(meta.manifestation).slice(0, 40)}` : ""
        }`.replace(/;$/, "") || "theme:manual"
      );
    case "idea":
      return "idea:manual";
    default:
      return "unknown:manual";
  }
}

function buildEdgeReason(
  edge: LitEdge,
  source?: LitNode,
  target?: LitNode,
): string {
  const kind = (edge.data?.kind ?? "flow") as EdgeKind;
  const parts: string[] = [`kind=${kind}`];
  switch (kind) {
    case "flow": {
      const srcEps = (source?.data.meta?.epsilon as number) ?? 30;
      const tgtEps = (target?.data.meta?.epsilon as number) ?? 30;
      const avgEps = (srcEps + tgtEps) / 2;
      parts.push(`avg_ε=${avgEps.toFixed(1)}`);
      parts.push(`src_ε=${srcEps}`);
      parts.push(`tgt_ε=${tgtEps}`);
      parts.push(`reason=sequence_by_chapter_num`);
      break;
    }
    case "cause":
      parts.push(`reason=causal_link`);
      if (edge.data?.note) parts.push(`note="${String(edge.data.note).slice(0, 60)}"`);
      break;
    case "character":
      parts.push(`reason=char_mentioned_in_chapter`);
      if (source?.data.meta?.mentions)
        parts.push(`char_freq=${source.data.meta.mentions}`);
      break;
    case "location":
      parts.push(`reason=loc_appears_in_chapter`);
      if (source?.data.meta?.mentions)
        parts.push(`loc_freq=${source.data.meta.mentions}`);
      break;
    case "conflict":
      parts.push(`reason=SVO_aggregation`);
      if (edge.data?.jValue !== undefined) parts.push(`J=${edge.data.jValue}`);
      if (edge.data?.svoTriples && Array.isArray(edge.data.svoTriples)) {
        parts.push(`svo_count=${edge.data.svoTriples.length}`);
      }
      break;
    case "foreshadow":
      parts.push(`reason=foreshadow_link`);
      break;
    case "reference":
      parts.push(`reason=reference_link`);
      break;
    case "alternative":
      parts.push(`reason=alt_branch`);
      break;
    case "theme":
      parts.push(`reason=theme_in_chapter`);
      break;
  }
  return parts.join(";");
}

// ====== Подготовка данных для встраивания ======

interface ExportHtmlOptions {
  title: string;
  author: string;
  description: string;
  parserVersion?: string;
  sourceMdHash?: string;
  createdAt?: number;
  /** Опционально: snapshot последних анализов (POLER, conflict, NER). */
  analysisSnapshot?: {
    poler?: unknown;
    conflict?: unknown;
    ner?: unknown;
  };
}

interface EmbeddedData {
  schema: "litgraph-xray-html/v1";
  exportedAt: number;
  project: {
    title: string;
    author: string;
    description: string;
    parserVersion: string;
    sourceMdHash: string;
    createdAt: number;
  };
  viewport: { x: number; y: number; zoom: number } | null;
  background: BackgroundLayer | null;
  nodes: LitNode[];
  edges: LitEdge[];
  nodeReasons: Record<string, string>;
  edgeReasons: Record<string, string>;
  nodeHeights: Record<string, number>;
  nodeTypeConfig: typeof NODE_TYPES;
  edgeTypeConfig: typeof EDGE_TYPES;
  counts: {
    nodes: number;
    edges: number;
    byType: Record<string, number>;
    byEdgeKind: Record<string, number>;
  };
  analysis?: {
    poler?: unknown;
    conflict?: unknown;
    ner?: unknown;
  };
  /** Smart X-Ray: диагностика каждой ноды (Map сериализуется как Record). */
  diagnostics?: Record<string, NodeDiagnostic>;
  /** Краткая сводка диагностики для topbar. */
  diagnosticsSummary?: {
    total: number;
    ok: number;
    suspect: number;
    error: number;
    warnings: number;
    suggestions: number;
  };
}

function countByType(nodes: LitNode[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const n of nodes) counts[n.type] = (counts[n.type] ?? 0) + 1;
  return counts;
}

function countByEdgeKind(edges: LitEdge[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const e of edges) {
    const k = (e.data?.kind ?? "flow") as string;
    counts[k] = (counts[k] ?? 0) + 1;
  }
  return counts;
}

// ====== HTML template ======
//
// Шаблон интерактивной мини-программы.
// Внутри:
//   1. CSS (inline в <style>) — layout в стиле desktop GUI: topbar, canvas, sidebar, statusbar
//   2. <script type="application/json" id="litgraph-data"> — все данные
//   3. <script> — vanilla JS: canvas rendering, pan/zoom, click handlers, sidebar fill
//
// Переменная __LITGRAPH_DATA_JSON__ заменяется на JSON.stringify(data) на этапе генерации.
// Никаких других плейсхолдеров — всё остальное статично, чтобы пользователь мог
// переиспользовать шаблон и подставить свой JSON вручную.

const HTML_TEMPLATE = `<!DOCTYPE html>
<html lang="ru">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>LitGraph X-ray — __TITLE_PLACEHOLDER__</title>
<style>
/* ===== Reset & base ===== */
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body { height: 100%; overflow: hidden; }
body {
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Noto Sans", "Noto Sans SC", sans-serif;
  font-size: 13px;
  color: #1c1917;
  background: #fafaf9;
  -webkit-font-smoothing: antialiased;
}

/* ===== Layout ===== */
#app {
  display: grid;
  grid-template-rows: auto auto 1fr auto;
  height: 100vh;
}
#topbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 10px 16px;
  background: #1c1917;
  color: #fafaf9;
  border-bottom: 1px solid #292524;
  flex-wrap: wrap;
}
#topbar .brand {
  display: flex;
  align-items: center;
  gap: 8px;
  font-weight: 700;
  font-size: 14px;
  letter-spacing: 0.02em;
}
#topbar .brand .logo {
  width: 22px;
  height: 22px;
  background: linear-gradient(135deg, #6366f1, #14b8a6);
  border-radius: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 800;
  color: #fff;
}
#topbar .title-block {
  display: flex;
  flex-direction: column;
  line-height: 1.2;
}
#topbar .title-block .title { font-size: 13px; font-weight: 600; }
#topbar .title-block .meta { font-size: 11px; opacity: 0.6; }
#topbar .spacer { flex: 1; }
#topbar .controls {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
#topbar .controls input[type="search"] {
  padding: 5px 10px;
  border-radius: 6px;
  border: 1px solid #44403c;
  background: #292524;
  color: #fafaf9;
  font-size: 12px;
  width: 180px;
}
#topbar .controls input[type="search"]:focus {
  outline: none;
  border-color: #6366f1;
}
#topbar .controls button {
  padding: 5px 10px;
  border-radius: 6px;
  border: 1px solid #44403c;
  background: #292524;
  color: #fafaf9;
  font-size: 12px;
  cursor: pointer;
  transition: background 0.15s;
}
#topbar .controls button:hover { background: #44403c; }
#topbar .controls label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  opacity: 0.8;
  cursor: pointer;
  user-select: none;
}
#topbar .controls input[type="range"] { width: 80px; }

/* ===== Main: canvas + sidebar ===== */
main {
  display: grid;
  grid-template-columns: 1fr 360px;
  overflow: hidden;
  position: relative;
}
#canvas-wrap {
  position: relative;
  overflow: hidden;
  background: #fafaf9;
  background-image:
    radial-gradient(circle, #d6cfc0 1px, transparent 1px);
  background-size: 20px 20px;
}
#canvas {
  display: block;
  width: 100%;
  height: 100%;
  cursor: grab;
}
#canvas.dragging { cursor: grabbing; }
#canvas.over-node { cursor: pointer; }
#canvas.over-edge { cursor: pointer; }

#tooltip {
  position: absolute;
  pointer-events: none;
  background: rgba(28, 25, 23, 0.95);
  color: #fafaf9;
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 11px;
  line-height: 1.4;
  max-width: 260px;
  z-index: 100;
  display: none;
  box-shadow: 0 4px 12px rgba(0,0,0,0.3);
  white-space: pre-wrap;
}
#tooltip.visible { display: block; }

/* ===== Smart X-Ray: diagnostic badges & sidebar blocks ===== */
#diag-bar {
  display: flex;
  gap: 8px;
  padding: 6px 12px;
  background: #fff7ed;
  border-bottom: 1px solid #fed7aa;
  font-size: 11px;
  align-items: center;
  flex-wrap: wrap;
}
#diag-bar .diag-pill {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border-radius: 12px;
  font-weight: 600;
}
#diag-bar .diag-pill.ok { background: #d1fae5; color: #065f46; }
#diag-bar .diag-pill.suspect { background: #fef3c7; color: #92400e; }
#diag-bar .diag-pill.error { background: #fee2e2; color: #991b1b; }
#diag-bar .diag-pill.info { background: #dbeafe; color: #1e40af; }
#diag-bar .diag-label { color: #78716c; font-weight: 500; }

.diag-section { margin-top: 4px; }
.diag-section h3 {
  display: flex;
  align-items: center;
  gap: 6px;
}
.diag-confidence {
  font-size: 11px;
  font-weight: 600;
  padding: 1px 6px;
  border-radius: 8px;
  margin-left: auto;
}
.diag-confidence.ok { background: #d1fae5; color: #065f46; }
.diag-confidence.suspect { background: #fef3c7; color: #92400e; }
.diag-confidence.error { background: #fee2e2; color: #991b1b; }

.diag-warning {
  display: flex;
  gap: 8px;
  padding: 8px 10px;
  border-radius: 6px;
  margin-bottom: 6px;
  font-size: 11.5px;
  line-height: 1.45;
  border-left: 3px solid;
}
.diag-warning.error { background: #fef2f2; border-color: #dc2626; }
.diag-warning.warn  { background: #fffbeb; border-color: #f59e0b; }
.diag-warning.info  { background: #eff6ff; border-color: #3b82f6; }
.diag-warning .diag-icon {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
  color: #fff;
  margin-top: 1px;
}
.diag-warning.error .diag-icon { background: #dc2626; }
.diag-warning.warn  .diag-icon { background: #f59e0b; }
.diag-warning.info  .diag-icon { background: #3b82f6; }
.diag-warning .diag-content { flex: 1; }
.diag-warning .diag-code {
  font-family: ui-monospace, monospace;
  font-size: 10px;
  color: #78716c;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 2px;
}
.diag-warning .diag-message { color: #1c1917; }
.diag-warning .diag-detail {
  color: #57534e;
  font-size: 11px;
  margin-top: 4px;
  padding-top: 4px;
  border-top: 1px dashed #e7e5e4;
}

.diag-suggestion {
  display: flex;
  gap: 8px;
  padding: 8px 10px;
  background: #f0fdf4;
  border-left: 3px solid #16a34a;
  border-radius: 6px;
  margin-bottom: 6px;
  font-size: 11.5px;
  line-height: 1.45;
}
.diag-suggestion .diag-icon {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  background: #16a34a;
  color: #fff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  font-weight: 700;
  margin-top: 1px;
}
.diag-suggestion .diag-code {
  font-family: ui-monospace, monospace;
  font-size: 10px;
  color: #166534;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  margin-bottom: 2px;
}

/* ===== Sidebar ===== */
#sidebar {
  background: #fff;
  border-left: 1px solid #e7e5e4;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}
#sidebar .panel-header {
  padding: 12px 16px;
  background: #f5f5f4;
  border-bottom: 1px solid #e7e5e4;
  display: flex;
  align-items: center;
  gap: 8px;
}
#sidebar .panel-header .icon {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
  font-weight: 800;
  color: #fff;
}
#sidebar .panel-header h2 {
  font-size: 13px;
  font-weight: 600;
  flex: 1;
}
#sidebar .panel-header .close {
  background: none;
  border: none;
  cursor: pointer;
  font-size: 16px;
  color: #78716c;
  padding: 2px 6px;
  border-radius: 4px;
}
#sidebar .panel-header .close:hover { background: #e7e5e4; color: #1c1917; }

#sidebar .panel-body { padding: 14px 16px; }
#sidebar .empty {
  padding: 40px 20px;
  text-align: center;
  color: #78716c;
  font-size: 12px;
  line-height: 1.6;
}
#sidebar .empty .hint {
  margin-top: 8px;
  font-size: 11px;
  opacity: 0.7;
}
#sidebar .section {
  margin-bottom: 16px;
}
#sidebar .section h3 {
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: #78716c;
  margin-bottom: 6px;
}
#sidebar .field {
  display: grid;
  grid-template-columns: 110px 1fr;
  gap: 6px 10px;
  margin-bottom: 4px;
  font-size: 12px;
  line-height: 1.5;
}
#sidebar .field .key {
  color: #78716c;
  font-weight: 500;
}
#sidebar .field .val {
  color: #1c1917;
  word-break: break-word;
}
#sidebar .field .val.mono {
  font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
  font-size: 11px;
  background: #f5f5f4;
  padding: 2px 6px;
  border-radius: 3px;
}
#sidebar .body-preview {
  background: #fafaf9;
  border: 1px solid #e7e5e4;
  border-radius: 6px;
  padding: 10px 12px;
  font-size: 12px;
  line-height: 1.6;
  color: #44403c;
  white-space: pre-wrap;
  max-height: 240px;
  overflow-y: auto;
  margin-top: 4px;
}
#sidebar .tag {
  display: inline-block;
  padding: 2px 8px;
  background: #f5f5f4;
  border: 1px solid #e7e5e4;
  border-radius: 10px;
  font-size: 11px;
  color: #57534e;
  margin-right: 4px;
  margin-bottom: 4px;
}
#sidebar .reason-box {
  background: #eef2ff;
  border-left: 3px solid #6366f1;
  padding: 8px 10px;
  border-radius: 4px;
  font-family: ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace;
  font-size: 11px;
  line-height: 1.5;
  color: #312e81;
  word-break: break-all;
}
#sidebar .meta-table {
  width: 100%;
  border-collapse: collapse;
  font-size: 11px;
}
#sidebar .meta-table td {
  padding: 4px 6px;
  border-bottom: 1px solid #f5f5f4;
  vertical-align: top;
}
#sidebar .meta-table td.key {
  color: #78716c;
  font-weight: 500;
  width: 35%;
  font-family: ui-monospace, monospace;
}
#sidebar .meta-table td.val {
  color: #1c1917;
  word-break: break-word;
  font-family: ui-monospace, monospace;
  font-size: 11px;
}
#sidebar .svo-triple {
  background: #fef3c7;
  border-left: 3px solid #d97706;
  padding: 6px 10px;
  border-radius: 4px;
  font-family: ui-monospace, monospace;
  font-size: 11px;
  margin-bottom: 4px;
  color: #78350f;
  word-break: break-word;
}
#sidebar .json-block {
  background: #1c1917;
  color: #d4d4d4;
  padding: 10px 12px;
  border-radius: 6px;
  font-family: ui-monospace, monospace;
  font-size: 11px;
  line-height: 1.5;
  overflow-x: auto;
  white-space: pre;
  max-height: 200px;
  overflow-y: auto;
}

/* ===== Legend ===== */
#legend {
  position: absolute;
  bottom: 12px;
  left: 12px;
  background: rgba(255, 255, 255, 0.96);
  border: 1px solid #e7e5e4;
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 11px;
  max-width: 240px;
  box-shadow: 0 2px 8px rgba(0,0,0,0.08);
}
#legend h4 {
  font-size: 10px;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: #78716c;
  margin-bottom: 6px;
}
#legend .row {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 3px;
}
#legend .swatch {
  width: 12px;
  height: 12px;
  border-radius: 3px;
  flex-shrink: 0;
}
#legend .row.dashed .swatch {
  background: repeating-linear-gradient(90deg, currentColor 0 4px, transparent 4px 8px);
}

/* ===== Statusbar ===== */
#statusbar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 6px 16px;
  background: #f5f5f4;
  border-top: 1px solid #e7e5e4;
  font-size: 11px;
  color: #57534e;
}
#statusbar .stat {
  display: flex;
  align-items: center;
  gap: 4px;
}
#statusbar .stat strong { color: #1c1917; }
#statusbar .spacer { flex: 1; }
#statusbar .kbd-hint {
  font-size: 10px;
  opacity: 0.7;
}
#statusbar .kbd-hint kbd {
  background: #fff;
  border: 1px solid #d6d3d1;
  border-radius: 3px;
  padding: 1px 5px;
  font-family: ui-monospace, monospace;
  font-size: 10px;
}

/* ===== Mobile ===== */
@media (max-width: 768px) {
  main { grid-template-columns: 1fr; grid-template-rows: 1fr 240px; }
  #sidebar { border-left: none; border-top: 1px solid #e7e5e4; }
  #topbar .controls input[type="search"] { width: 100px; }
}
</style>
</head>
<body>
<div id="app">
  <header id="topbar">
    <div class="brand">
      <span class="logo">L</span>
      <span>LitGraph X-ray</span>
    </div>
    <div class="title-block">
      <div class="title" id="proj-title">—</div>
      <div class="meta" id="proj-meta">—</div>
    </div>
    <div class="spacer"></div>
    <div class="controls">
      <input type="search" id="search" placeholder="Поиск (title/body/tags)…" autocomplete="off">
      <button id="btn-fit" title="Fit graph to screen (F)">Fit</button>
      <button id="btn-zoom-out" title="Zoom out (-)">−</button>
      <button id="btn-zoom-in" title="Zoom in (+)">+</button>
      <label title="Show/hide background image">
        <input type="checkbox" id="toggle-bg" checked> Фон
      </label>
      <label title="Background opacity">
        <input type="range" id="bg-opacity" min="0" max="100" value="55">
      </label>
    </div>
  </header>

  <div id="diag-bar" style="display:none">
    <span class="diag-label">Smart X-Ray:</span>
    <span class="diag-pill ok" id="diag-ok">0 ok</span>
    <span class="diag-pill suspect" id="diag-suspect">0 suspect</span>
    <span class="diag-pill error" id="diag-error">0 error</span>
    <span class="diag-pill info" id="diag-sugg">0 suggestions</span>
  </div>

  <main>
    <div id="canvas-wrap">
      <canvas id="canvas"></canvas>
      <div id="tooltip"></div>
      <div id="legend">
        <h4>Типы нод</h4>
        <div id="legend-nodes"></div>
        <h4 style="margin-top:8px">Типы связей</h4>
        <div id="legend-edges"></div>
      </div>
    </div>
    <aside id="sidebar">
      <div class="panel-header">
        <span class="icon" id="side-icon" style="background:#78716c">?</span>
        <h2 id="side-title">Инспектор</h2>
        <button class="close" id="side-close" title="Закрыть (Esc)">×</button>
      </div>
      <div class="panel-body" id="side-body">
        <div class="empty">
          Кликни по ноде или связи,<br>чтобы увидеть X-ray детали.
          <div class="hint">
            Pan: drag · Zoom: wheel · Fit: F · Deselect: Esc
          </div>
        </div>
      </div>
    </aside>
  </main>

  <footer id="statusbar">
    <div class="stat">Нод: <strong id="stat-nodes">0</strong></div>
    <div class="stat">Связей: <strong id="stat-edges">0</strong></div>
    <div class="stat" id="stat-zoom">Zoom: 100%</div>
    <div class="spacer"></div>
    <div class="kbd-hint">
      <kbd>F</kbd> fit · <kbd>+</kbd>/<kbd>−</kbd> zoom · <kbd>↑↓←→</kbd> pan · <kbd>Esc</kbd> deselect
    </div>
  </footer>
</div>

<script type="application/json" id="litgraph-data">
__LITGRAPH_DATA_JSON__
</script>
<script>
"use strict";
// ===== LitGraph X-ray mini-program =====
// Vanilla JS, no dependencies. Renders workspace as interactive canvas.
// Source: src/lib/litgraph/export-html.ts

(function() {
  const dataEl = document.getElementById('litgraph-data');
  const DATA = JSON.parse(dataEl.textContent);

  const NODE_TYPES = DATA.nodeTypeConfig;
  const EDGE_TYPES = DATA.edgeTypeConfig;
  const DIAGNOSTICS = DATA.diagnostics || {};
  const DIAG_SUMMARY = DATA.diagnosticsSummary || null;
  const NODE_WIDTH = 260;
  const NODE_BASE_HEIGHT = 70;

  // ===== Smart X-Ray: диагностическая полоска вверху =====
  if (DIAG_SUMMARY) {
    const bar = document.getElementById('diag-bar');
    bar.style.display = 'flex';
    document.getElementById('diag-ok').textContent = DIAG_SUMMARY.ok + ' ok';
    document.getElementById('diag-suspect').textContent = DIAG_SUMMARY.suspect + ' suspect';
    document.getElementById('diag-error').textContent = DIAG_SUMMARY.error + ' error';
    document.getElementById('diag-sugg').textContent = DIAG_SUMMARY.suggestions + ' suggestions';
    // Скрываем pills с нулевым количеством
    if (DIAG_SUMMARY.suspect === 0) document.getElementById('diag-suspect').style.display = 'none';
    if (DIAG_SUMMARY.error === 0) document.getElementById('diag-error').style.display = 'none';
    if (DIAG_SUMMARY.suggestions === 0) document.getElementById('diag-sugg').style.display = 'none';
  }

  // Helper: получить диагностику ноды
  function getDiagnostic(nodeId) {
    return DIAGNOSTICS[nodeId] || null;
  }

  // Helper: цвет рамки ноды по уровню диагностики
  function diagBorderColor(nodeId) {
    const d = getDiagnostic(nodeId);
    if (!d) return null;
    if (d.level === 'error') return '#dc2626';
    if (d.level === 'suspect') return '#f59e0b';
    return null;
  }

  // Helper: иконка для badge
  function diagBadgeIcon(nodeId) {
    const d = getDiagnostic(nodeId);
    if (!d) return null;
    if (d.level === 'error') return '!';
    if (d.level === 'suspect') return '?';
    return null;
  }

  function nodeHeight(n) {
    if (n.type !== 'chapter') return NODE_BASE_HEIGHT;
    const eps = (n.data.meta && n.data.meta.epsilon) ?? 30;
    return NODE_BASE_HEIGHT + (eps / 100) * (140 - NODE_BASE_HEIGHT);
  }

  // ===== State =====
  const state = {
    viewport: { x: 0, y: 0, zoom: 1 },
    selectedNodeId: null,
    selectedEdgeId: null,
    hoveredNodeId: null,
    hoveredEdgeId: null,
    searchQuery: '',
    showBg: true,
    bgOpacity: 0.55,
    bgImage: null,
    bgImageReady: false,
    isDragging: false,
    dragStart: { x: 0, y: 0 },
    vpStart: { x: 0, y: 0 },
  };

  // Initial viewport from data (if any)
  if (DATA.viewport) {
    state.viewport = { ...DATA.viewport };
  }

  // ===== Canvas setup =====
  const canvas = document.getElementById('canvas');
  const ctx = canvas.getContext('2d');
  const wrap = document.getElementById('canvas-wrap');
  const tooltip = document.getElementById('tooltip');

  let dpr = window.devicePixelRatio || 1;
  function resizeCanvas() {
    const rect = wrap.getBoundingClientRect();
    dpr = window.devicePixelRatio || 1;
    canvas.width = rect.width * dpr;
    canvas.height = rect.height * dpr;
    canvas.style.width = rect.width + 'px';
    canvas.style.height = rect.height + 'px';
    render();
  }
  window.addEventListener('resize', resizeCanvas);

  // ===== Background image =====
  if (DATA.background && DATA.background.src) {
    const img = new Image();
    img.onload = () => {
      state.bgImage = img;
      state.bgImageReady = true;
      state.bgOpacity = DATA.background.opacity ?? 0.55;
      document.getElementById('bg-opacity').value = Math.round(state.bgOpacity * 100);
      render();
    };
    img.onerror = () => {
      console.warn('[LitGraph X-ray] Background image failed to load');
    };
    img.src = DATA.background.src;
  }

  // ===== Helpers: world ↔ screen =====
  function worldToScreen(x, y) {
    return {
      x: x * state.viewport.zoom + state.viewport.x,
      y: y * state.viewport.zoom + state.viewport.y,
    };
  }
  function screenToWorld(x, y) {
    return {
      x: (x - state.viewport.x) / state.viewport.zoom,
      y: (y - state.viewport.y) / state.viewport.zoom,
    };
  }

  // ===== Hit testing =====
  function nodeAt(screenX, screenY) {
    // Iterate in reverse so top-drawn nodes are picked first
    for (let i = DATA.nodes.length - 1; i >= 0; i--) {
      const n = DATA.nodes[i];
      if (!matchesSearch(n)) continue;
      const sp = worldToScreen(n.position.x, n.position.y);
      const w = NODE_WIDTH * state.viewport.zoom;
      const h = nodeHeight(n) * state.viewport.zoom;
      if (screenX >= sp.x && screenX <= sp.x + w &&
          screenY >= sp.y && screenY <= sp.y + h) {
        return n;
      }
    }
    return null;
  }

  function edgeAt(screenX, screenY) {
    // Sample bezier points and check distance
    for (const e of DATA.edges) {
      const s = DATA.nodes.find(n => n.id === e.source);
      const t = DATA.nodes.find(n => n.id === e.target);
      if (!s || !t) continue;
      const sx = s.position.x + NODE_WIDTH;
      const sy = s.position.y + nodeHeight(s) / 2;
      const tx = t.position.x;
      const ty = t.position.y + nodeHeight(t) / 2;
      // Sample 30 points along the bezier
      const threshold = 8;
      for (let i = 0; i <= 30; i++) {
        const u = i / 30;
        // Cubic bezier with cp1 = (sx + dx*0.5, sy), cp2 = (tx - dx*0.5, ty)
        const dx = tx - sx;
        const cp1x = sx + dx * 0.5, cp1y = sy;
        const cp2x = tx - dx * 0.5, cp2y = ty;
        const x = (1-u)**3 * sx + 3*(1-u)**2 * u * cp1x + 3*(1-u) * u**2 * cp2x + u**3 * tx;
        const y = (1-u)**3 * sy + 3*(1-u)**2 * u * cp1y + 3*(1-u) * u**2 * cp2y + u**3 * ty;
        const sp = worldToScreen(x, y);
        const d = Math.hypot(sp.x - screenX, sp.y - screenY);
        if (d <= threshold) return e;
      }
    }
    return null;
  }

  function matchesSearch(n) {
    if (!state.searchQuery) return true;
    const q = state.searchQuery.toLowerCase();
    return (
      (n.data.title || '').toLowerCase().includes(q) ||
      (n.data.body || '').toLowerCase().includes(q) ||
      (n.data.tags || []).some(t => t.toLowerCase().includes(q))
    );
  }

  // ===== Rendering =====
  function render() {
    const W = canvas.width;
    const H = canvas.height;
    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.clearRect(0, 0, W / dpr, H / dpr);

    // CSS pixel size
    const cssW = W / dpr;
    const cssH = H / dpr;

    // Background image (in world coords, transformed by viewport)
    if (state.showBg && state.bgImageReady && DATA.background) {
      const bg = DATA.background;
      const dw = bg.naturalWidth * bg.scale;
      const dh = bg.naturalHeight * bg.scale;
      const sp = worldToScreen(bg.x, bg.y);
      const sw = dw * state.viewport.zoom;
      const sh = dh * state.viewport.zoom;
      ctx.globalAlpha = state.bgOpacity;
      if (bg.rotation) {
        const cx = sp.x + sw / 2;
        const cy = sp.y + sh / 2;
        ctx.translate(cx, cy);
        ctx.rotate(bg.rotation * Math.PI / 180);
        ctx.translate(-cx, -cy);
      }
      ctx.drawImage(state.bgImage, sp.x, sp.y, sw, sh);
      ctx.globalAlpha = 1;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    }

    // Edges
    for (const e of DATA.edges) {
      const s = DATA.nodes.find(n => n.id === e.source);
      const t = DATA.nodes.find(n => n.id === e.target);
      if (!s || !t) continue;
      // Skip edges to filtered-out nodes only if BOTH endpoints are filtered
      if (!matchesSearch(s) && !matchesSearch(t)) continue;

      const kind = (e.data && e.data.kind) || 'flow';
      const cfg = EDGE_TYPES[kind] || EDGE_TYPES.flow;
      const sx = s.position.x + NODE_WIDTH;
      const sy = s.position.y + nodeHeight(s) / 2;
      const tx = t.position.x;
      const ty = t.position.y + nodeHeight(t) / 2;
      const dx = tx - sx;
      const cp1x = sx + dx * 0.5, cp1y = sy;
      const cp2x = tx - dx * 0.5, cp2y = ty;

      const sp1 = worldToScreen(sx, sy);
      const sp2 = worldToScreen(cp1x, cp1y);
      const sp3 = worldToScreen(cp2x, cp2y);
      const sp4 = worldToScreen(tx, ty);

      // Stroke width: flow depends on epsilon
      let strokeWidth = 2;
      if (kind === 'flow') {
        const srcEps = (s.data.meta && s.data.meta.epsilon) ?? 30;
        const tgtEps = (t.data.meta && t.data.meta.epsilon) ?? 30;
        const avgEps = (srcEps + tgtEps) / 2;
        strokeWidth = (1 + (avgEps / 100) * 4) * state.viewport.zoom;
      } else {
        strokeWidth = 2 * state.viewport.zoom;
      }

      // Highlight if selected/hovered
      const isSelected = state.selectedEdgeId === e.id;
      const isHovered = state.hoveredEdgeId === e.id;
      if (isSelected) {
        ctx.strokeStyle = cfg.color;
        ctx.lineWidth = strokeWidth + 3;
        ctx.shadowColor = cfg.color;
        ctx.shadowBlur = 8;
      } else if (isHovered) {
        ctx.strokeStyle = cfg.color;
        ctx.lineWidth = strokeWidth + 1.5;
        ctx.shadowBlur = 0;
      } else {
        ctx.strokeStyle = cfg.color;
        ctx.lineWidth = strokeWidth;
        ctx.shadowBlur = 0;
      }

      ctx.beginPath();
      ctx.moveTo(sp1.x, sp1.y);
      ctx.bezierCurveTo(sp2.x, sp2.y, sp3.x, sp3.y, sp4.x, sp4.y);
      if (cfg.dashed) {
        const dash = 6 * state.viewport.zoom;
        ctx.setLineDash([dash, dash * 0.7]);
      } else {
        ctx.setLineDash([]);
      }
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.shadowBlur = 0;

      // Edge label at midpoint
      const midX = (sp1.x + sp4.x) / 2;
      const midY = (sp1.y + sp4.y) / 2;
      const labelW = cfg.label.length * 6 + 12;
      const labelH = 14;
      ctx.fillStyle = '#fff';
      ctx.strokeStyle = cfg.color + '40';
      ctx.lineWidth = 1;
      ctx.beginPath();
      // manual rounded rect
      const rx = midX - labelW / 2;
      const ry = midY - labelH / 2;
      const r = 7;
      ctx.moveTo(rx + r, ry);
      ctx.lineTo(rx + labelW - r, ry);
      ctx.quadraticCurveTo(rx + labelW, ry, rx + labelW, ry + r);
      ctx.lineTo(rx + labelW, ry + labelH - r);
      ctx.quadraticCurveTo(rx + labelW, ry + labelH, rx + labelW - r, ry + labelH);
      ctx.lineTo(rx + r, ry + labelH);
      ctx.quadraticCurveTo(rx, ry + labelH, rx, ry + labelH - r);
      ctx.lineTo(rx, ry + r);
      ctx.quadraticCurveTo(rx, ry, rx + r, ry);
      ctx.closePath();
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = cfg.color;
      ctx.font = '10px sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(cfg.label, midX, midY);
    }

    // Nodes
    for (const n of DATA.nodes) {
      const visible = matchesSearch(n);
      if (!visible) continue;

      const cfg = NODE_TYPES[n.type] || NODE_TYPES.idea;
      const h = nodeHeight(n);
      const sp = worldToScreen(n.position.x, n.position.y);
      const w = NODE_WIDTH * state.viewport.zoom;
      const ch = h * state.viewport.zoom;
      const x = sp.x;
      const y = sp.y;

      const isSelected = state.selectedNodeId === n.id;
      const isHovered = state.hoveredNodeId === n.id;

      // Dim if focus mode and not selected/hovered (we don't have focus mode here — keep simple)
      let alpha = 1;
      if (state.searchQuery && !matchesSearch(n)) alpha = 0.2;

      ctx.globalAlpha = alpha;

      // Shadow when selected
      if (isSelected) {
        ctx.shadowColor = cfg.color;
        ctx.shadowBlur = 12;
        ctx.shadowOffsetY = 2;
      } else if (isHovered) {
        ctx.shadowColor = 'rgba(0,0,0,0.15)';
        ctx.shadowBlur = 8;
      } else {
        ctx.shadowBlur = 0;
        ctx.shadowOffsetY = 0;
      }

      // Card background
      ctx.fillStyle = '#fff';
      const diagColor = diagBorderColor(n.id);
      if (diagColor) {
        // Подсвечиваем подозрительные ноды цветной рамкой
        ctx.strokeStyle = diagColor;
        ctx.lineWidth = isSelected ? 3 : 2;
      } else {
        ctx.strokeStyle = isSelected ? cfg.color : cfg.color + '40';
        ctx.lineWidth = isSelected ? 2 : 1;
      }
      const radius = 11 * state.viewport.zoom;
      drawRoundedRect(ctx, x, y, w, ch, radius);
      ctx.fill();
      ctx.stroke();
      ctx.shadowBlur = 0;
      ctx.shadowOffsetY = 0;

      // Smart X-Ray badge (если есть диагностика)
      const badgeIcon = diagBadgeIcon(n.id);
      if (badgeIcon) {
        const bx = x + w - 14 * state.viewport.zoom;
        const by = y + 14 * state.viewport.zoom;
        const br = 8 * state.viewport.zoom;
        ctx.fillStyle = diagColor;
        ctx.beginPath();
        ctx.arc(bx, by, br, 0, Math.PI * 2);
        ctx.fill();
        ctx.fillStyle = '#fff';
        ctx.font = 'bold ' + (10 * state.viewport.zoom) + 'px sans-serif';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText(badgeIcon, bx, by);
        ctx.textAlign = 'left';
        ctx.textBaseline = 'alphabetic';
      }

      // Left color bar
      ctx.fillStyle = cfg.color;
      drawRoundedRect(ctx, x, y, 4 * state.viewport.zoom, ch, 2);
      ctx.fill();

      // Header bg
      ctx.fillStyle = cfg.color + '28';
      drawRoundedRect(ctx, x + 4 * state.viewport.zoom, y, w - 4 * state.viewport.zoom, 28 * state.viewport.zoom, 8 * state.viewport.zoom);
      ctx.fill();

      // Icon circle
      const iconR = 9 * state.viewport.zoom;
      const iconCx = x + 20 * state.viewport.zoom;
      const iconCy = y + 14 * state.viewport.zoom;
      ctx.fillStyle = cfg.color;
      ctx.beginPath();
      ctx.arc(iconCx, iconCy, iconR, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = '#fff';
      ctx.font = 'bold ' + (10 * state.viewport.zoom) + 'px sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText((cfg.singular[0] || '?'), iconCx, iconCy);

      // Type label
      ctx.fillStyle = cfg.color;
      ctx.font = '600 ' + (10 * state.viewport.zoom) + 'px sans-serif';
      ctx.textAlign = 'left';
      ctx.fillText(cfg.singular.toUpperCase(), x + 35 * state.viewport.zoom, iconCy);

      // Title
      ctx.fillStyle = '#292524';
      ctx.font = '600 ' + (13 * state.viewport.zoom) + 'px sans-serif';
      ctx.textAlign = 'left';
      ctx.textBaseline = 'alphabetic';
      const titleText = truncate(n.data.title || 'Без названия', 38);
      ctx.fillText(titleText, x + 12 * state.viewport.zoom, y + 34 * state.viewport.zoom);

      // Body preview
      if (n.data.body) {
        ctx.fillStyle = '#78716c';
        ctx.font = (11 * state.viewport.zoom) + 'px sans-serif';
        const bodyText = truncate(n.data.body.replace(/\\s+/g, ' ').trim(), 70);
        ctx.fillText(bodyText, x + 12 * state.viewport.zoom, y + 54 * state.viewport.zoom);
      }

      // Epsilon badge for chapter
      if (n.type === 'chapter' && n.data.meta && n.data.meta.epsilon !== undefined) {
        const eps = Number(n.data.meta.epsilon);
        const epsColor = eps > 70 ? '#dc2626' : eps > 40 ? '#d97706' : '#78716c';
        ctx.fillStyle = epsColor;
        ctx.font = '600 ' + (9 * state.viewport.zoom) + 'px sans-serif';
        ctx.textAlign = 'right';
        ctx.fillText('ε' + Math.round(eps), x + w - 12 * state.viewport.zoom, y + 14 * state.viewport.zoom);

        // Epsilon bar at bottom
        const barW = w - 8 * state.viewport.zoom;
        const barY = y + ch - 6 * state.viewport.zoom;
        ctx.fillStyle = '#f0f0f0';
        ctx.fillRect(x + 4 * state.viewport.zoom, barY, barW, 4 * state.viewport.zoom);
        const barColor = eps > 70 ? '#dc2626' : eps > 40 ? '#f59e0b' : '#65a30d';
        ctx.fillStyle = barColor;
        ctx.fillRect(x + 4 * state.viewport.zoom, barY, barW * (eps / 100), 4 * state.viewport.zoom);
      }

      // Handle circles (decoration)
      ctx.fillStyle = '#fff';
      ctx.strokeStyle = cfg.color;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(x, y + ch / 2, 6 * state.viewport.zoom, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      ctx.beginPath();
      ctx.arc(x + w, y + ch / 2, 6 * state.viewport.zoom, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();

      ctx.globalAlpha = 1;
    }

    ctx.restore();
  }

  function drawRoundedRect(ctx, x, y, w, h, r) {
    if (w < 2 * r) r = w / 2;
    if (h < 2 * r) r = h / 2;
    if (r < 0) r = 0;
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.lineTo(x + w - r, y);
    ctx.quadraticCurveTo(x + w, y, x + w, y + r);
    ctx.lineTo(x + w, y + h - r);
    ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
    ctx.lineTo(x + r, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - r);
    ctx.lineTo(x, y + r);
    ctx.quadraticCurveTo(x, y, x + r, y);
    ctx.closePath();
  }

  function truncate(s, max) {
    if (s.length <= max) return s;
    let end = max;
    while (end > 0) {
      const c = s.codePointAt(end - 1);
      if (c === undefined) break;
      if (c >= 0xDC00 && c <= 0xDFFF) { end -= 1; continue; }
      break;
    }
    return s.slice(0, end) + '…';
  }

  // ===== Fit to screen =====
  function fitView() {
    if (DATA.nodes.length === 0) return;
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    if (DATA.background) {
      const bg = DATA.background;
      const dw = bg.naturalWidth * bg.scale;
      const dh = bg.naturalHeight * bg.scale;
      minX = Math.min(minX, bg.x);
      minY = Math.min(minY, bg.y);
      maxX = Math.max(maxX, bg.x + dw);
      maxY = Math.max(maxY, bg.y + dh);
    }
    for (const n of DATA.nodes) {
      const h = nodeHeight(n);
      minX = Math.min(minX, n.position.x);
      minY = Math.min(minY, n.position.y);
      maxX = Math.max(maxX, n.position.x + NODE_WIDTH);
      maxY = Math.max(maxY, n.position.y + h);
    }
    if (!isFinite(minX)) return;
    const pad = 60;
    minX -= pad; minY -= pad; maxX += pad; maxY += pad;
    const w = maxX - minX;
    const h = maxY - minY;
    const rect = wrap.getBoundingClientRect();
    const zoom = Math.min(rect.width / w, rect.height / h, 1.5);
    state.viewport.zoom = zoom;
    state.viewport.x = rect.width / 2 - (minX + w / 2) * zoom;
    state.viewport.y = rect.height / 2 - (minY + h / 2) * zoom;
    render();
    updateZoomLabel();
  }

  function zoomBy(factor, centerX, centerY) {
    const rect = wrap.getBoundingClientRect();
    if (centerX === undefined) centerX = rect.width / 2;
    if (centerY === undefined) centerY = rect.height / 2;
    const worldBefore = screenToWorld(centerX, centerY);
    state.viewport.zoom = Math.max(0.1, Math.min(5, state.viewport.zoom * factor));
    const worldAfter = screenToWorld(centerX, centerY);
    state.viewport.x += (worldAfter.x - worldBefore.x) * state.viewport.zoom;
    state.viewport.y += (worldAfter.y - worldBefore.y) * state.viewport.zoom;
    render();
    updateZoomLabel();
  }

  function updateZoomLabel() {
    document.getElementById('stat-zoom').textContent = 'Zoom: ' + Math.round(state.viewport.zoom * 100) + '%';
  }

  // ===== Mouse handlers =====
  canvas.addEventListener('mousedown', (e) => {
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const n = nodeAt(x, y);
    const ed = n ? null : edgeAt(x, y);
    if (!n && !ed) {
      state.isDragging = true;
      state.dragStart = { x, y };
      state.vpStart = { x: state.viewport.x, y: state.viewport.y };
      canvas.classList.add('dragging');
    }
  });

  canvas.addEventListener('mousemove', (e) => {
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;

    if (state.isDragging) {
      state.viewport.x = state.vpStart.x + (x - state.dragStart.x);
      state.viewport.y = state.vpStart.y + (y - state.dragStart.y);
      render();
      return;
    }

    const n = nodeAt(x, y);
    const ed = n ? null : edgeAt(x, y);

    if (n && state.hoveredNodeId !== n.id) {
      state.hoveredNodeId = n.id;
      state.hoveredEdgeId = null;
      canvas.classList.add('over-node');
      canvas.classList.remove('over-edge');
      showTooltip(e, nodeTooltip(n));
      render();
    } else if (ed && state.hoveredEdgeId !== ed.id) {
      state.hoveredEdgeId = ed.id;
      state.hoveredNodeId = null;
      canvas.classList.add('over-edge');
      canvas.classList.remove('over-node');
      showTooltip(e, edgeTooltip(ed));
      render();
    } else if (!n && !ed) {
      if (state.hoveredNodeId || state.hoveredEdgeId) {
        state.hoveredNodeId = null;
        state.hoveredEdgeId = null;
        canvas.classList.remove('over-node', 'over-edge');
        hideTooltip();
        render();
      } else {
        hideTooltip();
      }
    } else if (n || ed) {
      moveTooltip(e);
    }
  });

  canvas.addEventListener('mouseup', (e) => {
    if (state.isDragging) {
      state.isDragging = false;
      canvas.classList.remove('dragging');
      return;
    }
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const n = nodeAt(x, y);
    const ed = n ? null : edgeAt(x, y);
    if (n) {
      state.selectedNodeId = n.id;
      state.selectedEdgeId = null;
      fillSidebarForNode(n);
      render();
    } else if (ed) {
      state.selectedEdgeId = ed.id;
      state.selectedNodeId = null;
      fillSidebarForEdge(ed);
      render();
    } else {
      // Click on empty space — deselect
      state.selectedNodeId = null;
      state.selectedEdgeId = null;
      clearSidebar();
      render();
    }
  });

  canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const factor = e.deltaY < 0 ? 1.1 : 0.9;
    zoomBy(factor, x, y);
  }, { passive: false });

  canvas.addEventListener('mouseleave', () => {
    state.hoveredNodeId = null;
    state.hoveredEdgeId = null;
    state.isDragging = false;
    canvas.classList.remove('dragging', 'over-node', 'over-edge');
    hideTooltip();
    render();
  });

  // ===== Tooltip =====
  function showTooltip(e, text) {
    if (!text) return;
    tooltip.textContent = text;
    tooltip.classList.add('visible');
    moveTooltip(e);
  }
  function moveTooltip(e) {
    const rect = wrap.getBoundingClientRect();
    let x = e.clientX - rect.left + 14;
    let y = e.clientY - rect.top + 14;
    // Keep inside wrap
    const tw = tooltip.offsetWidth;
    const th = tooltip.offsetHeight;
    if (x + tw > rect.width) x = e.clientX - rect.left - tw - 8;
    if (y + th > rect.height) y = e.clientY - rect.top - th - 8;
    tooltip.style.left = x + 'px';
    tooltip.style.top = y + 'px';
  }
  function hideTooltip() {
    tooltip.classList.remove('visible');
  }

  function nodeTooltip(n) {
    const cfg = NODE_TYPES[n.type] || NODE_TYPES.idea;
    const reason = DATA.nodeReasons[n.id] || '';
    let s = cfg.singular + ': ' + (n.data.title || 'Без названия');
    if (n.data.body) s += '\\n' + truncate(n.data.body.replace(/\\s+/g, ' ').trim(), 80);
    if (reason) s += '\\n\\n[reason] ' + reason;
    // Smart X-Ray: показываем диагностику в tooltip
    const d = getDiagnostic(n.id);
    if (d && (d.warnings.length > 0 || d.suggestions.length > 0)) {
      s += '\\n\\n⚠ Smart X-Ray (confidence ' + Math.round(d.confidence * 100) + '%, ' + d.level + '):';
      for (const w of d.warnings.slice(0, 2)) {
        s += '\\n  [' + w.level + '] ' + w.code + ': ' + truncate(w.message, 90);
      }
      for (const sug of d.suggestions.slice(0, 1)) {
        s += '\\n  [suggestion] ' + sug.code + ': ' + truncate(sug.message, 90);
      }
    }
    return s;
  }

  function edgeTooltip(e) {
    const kind = (e.data && e.data.kind) || 'flow';
    const cfg = EDGE_TYPES[kind] || EDGE_TYPES.flow;
    const reason = DATA.edgeReasons[e.id] || '';
    const s = e.source + ' → ' + e.target + '\\n[' + cfg.label + ']';
    if (reason) s += '\\n' + reason;
    if (e.data && e.data.jValue !== undefined) s += '\\nJ=' + e.data.jValue;
    return s;
  }

  // ===== Sidebar =====
  function clearSidebar() {
    document.getElementById('side-icon').style.background = '#78716c';
    document.getElementById('side-icon').textContent = '?';
    document.getElementById('side-title').textContent = 'Инспектор';
    document.getElementById('side-body').innerHTML =
      '<div class="empty">Кликни по ноде или связи,<br>чтобы увидеть X-ray детали.' +
      '<div class="hint">Pan: drag · Zoom: wheel · Fit: F · Deselect: Esc</div></div>';
  }

  function escapeHtml(s) {
    if (s === null || s === undefined) return '';
    return String(s)
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function formatValue(v) {
    if (v === null) return 'null';
    if (v === undefined) return 'undefined';
    if (typeof v === 'string') return v;
    if (typeof v === 'number' || typeof v === 'boolean') return String(v);
    try { return JSON.stringify(v); } catch { return '[unserializable]'; }
  }

  function metaTableHtml(meta) {
    if (!meta || Object.keys(meta).length === 0) return '<div style="color:#a8a29e;font-size:11px">— нет meta —</div>';
    let rows = '';
    for (const [k, v] of Object.entries(meta)) {
      rows += '<tr><td class="key">' + escapeHtml(k) + '</td><td class="val">' + escapeHtml(formatValue(v)) + '</td></tr>';
    }
    return '<table class="meta-table">' + rows + '</table>';
  }

  function fillSidebarForNode(n) {
    const cfg = NODE_TYPES[n.type] || NODE_TYPES.idea;
    const reason = DATA.nodeReasons[n.id] || '';
    document.getElementById('side-icon').style.background = cfg.color;
    document.getElementById('side-icon').textContent = cfg.singular[0] || '?';
    document.getElementById('side-title').textContent = cfg.singular + ' — ' + truncate(n.data.title || 'Без названия', 32);

    let html = '';

    // Title section
    html += '<div class="section">';
    html += '<h3>Заголовок</h3>';
    html += '<div class="field"><div class="val" style="font-weight:600">' + escapeHtml(n.data.title || 'Без названия') + '</div></div>';
    html += '</div>';

    // Body
    if (n.data.body) {
      html += '<div class="section">';
      html += '<h3>Содержание</h3>';
      html += '<div class="body-preview">' + escapeHtml(n.data.body) + '</div>';
      html += '</div>';
    }

    // Tags
    if (n.data.tags && n.data.tags.length > 0) {
      html += '<div class="section">';
      html += '<h3>Теги</h3>';
      html += '<div>' + n.data.tags.map(t => '<span class="tag">' + escapeHtml(t) + '</span>').join('') + '</div>';
      html += '</div>';
    }

    // Smart X-Ray: диагностика
    const diag = getDiagnostic(n.id);
    if (diag && (diag.warnings.length > 0 || diag.suggestions.length > 0)) {
      html += '<div class="section diag-section">';
      html += '<h3>Smart X-Ray: Диагностика';
      html += '<span class="diag-confidence ' + diag.level + '">' + Math.round(diag.confidence * 100) + '%</span>';
      html += '</h3>';
      html += '<div style="font-size:11px;color:#78716c;margin-bottom:8px">' + escapeHtml(diag.summary) + '</div>';
      // Warnings
      for (const w of diag.warnings) {
        const iconChar = w.level === 'error' ? '!' : w.level === 'warn' ? '?' : 'i';
        html += '<div class="diag-warning ' + w.level + '">';
        html += '<span class="diag-icon">' + iconChar + '</span>';
        html += '<div class="diag-content">';
        html += '<div class="diag-code">' + escapeHtml(w.code) + '</div>';
        html += '<div class="diag-message">' + escapeHtml(w.message) + '</div>';
        if (w.detail) {
          html += '<div class="diag-detail">' + escapeHtml(w.detail) + '</div>';
        }
        html += '</div>';
        html += '</div>';
      }
      // Suggestions
      for (const sug of diag.suggestions) {
        html += '<div class="diag-suggestion">';
        html += '<span class="diag-icon">→</span>';
        html += '<div class="diag-content">';
        html += '<div class="diag-code">' + escapeHtml(sug.code) + '</div>';
        html += '<div class="diag-message">' + escapeHtml(sug.message) + '</div>';
        if (sug.detail) {
          html += '<div class="diag-detail">' + escapeHtml(sug.detail) + '</div>';
        }
        if (sug.targetNodeId) {
          html += '<div class="diag-detail" style="border-color:#bbf7d0">target: ' + escapeHtml(sug.targetNodeId) + '</div>';
        }
        html += '</div>';
        html += '</div>';
      }
      html += '</div>';
    }

    // Reason (X-ray)
    if (reason) {
      html += '<div class="section">';
      html += '<h3>Algorithm Reason (X-ray)</h3>';
      html += '<div class="reason-box">' + escapeHtml(reason) + '</div>';
      html += '</div>';
    }

    // Meta table
    if (n.data.meta && Object.keys(n.data.meta).length > 0) {
      html += '<div class="section">';
      html += '<h3>Meta поля (полный X-ray)</h3>';
      html += metaTableHtml(n.data.meta);
      html += '</div>';
    }

    // Connected edges
    const connected = DATA.edges.filter(e => e.source === n.id || e.target === n.id);
    if (connected.length > 0) {
      html += '<div class="section">';
      html += '<h3>Связи (' + connected.length + ')</h3>';
      html += '<table class="meta-table">';
      for (const e of connected) {
        const kind = (e.data && e.data.kind) || 'flow';
        const cfgE = EDGE_TYPES[kind] || EDGE_TYPES.flow;
        const otherId = e.source === n.id ? e.target : e.source;
        const other = DATA.nodes.find(x => x.id === otherId);
        const otherTitle = other ? truncate(other.data.title || other.id, 30) : otherId;
        const dir = e.source === n.id ? '→' : '←';
        html += '<tr><td class="key" style="color:' + cfgE.color + '">' + dir + ' ' + cfgE.label + '</td>' +
                '<td class="val">' + escapeHtml(otherTitle) + '</td></tr>';
      }
      html += '</table>';
      html += '</div>';
    }

    // Versions (chapter/scene)
    if (n.data.versions && n.data.versions.length > 0) {
      html += '<div class="section">';
      html += '<h3>Версии (' + n.data.versions.length + ')</h3>';
      html += '<table class="meta-table">';
      for (const v of n.data.versions.slice(0, 10)) {
        const date = new Date(v.timestamp).toLocaleString('ru-RU');
        html += '<tr><td class="key">' + escapeHtml(v.source || 'manual') + '</td>' +
                '<td class="val">' + escapeHtml(date) + ' · ' + v.wordCount + ' слов</td></tr>';
      }
      html += '</table>';
      html += '</div>';
    }

    // Node id
    html += '<div class="section">';
    html += '<h3>ID</h3>';
    html += '<div class="field"><div class="val mono">' + escapeHtml(n.id) + '</div></div>';
    html += '<div class="field"><div class="key">position</div><div class="val mono">x=' + n.position.x.toFixed(0) + ', y=' + n.position.y.toFixed(0) + '</div></div>';
    html += '</div>';

    document.getElementById('side-body').innerHTML = html;
  }

  function fillSidebarForEdge(e) {
    const kind = (e.data && e.data.kind) || 'flow';
    const cfg = EDGE_TYPES[kind] || EDGE_TYPES.flow;
    const reason = DATA.edgeReasons[e.id] || '';
    const source = DATA.nodes.find(n => n.id === e.source);
    const target = DATA.nodes.find(n => n.id === e.target);

    document.getElementById('side-icon').style.background = cfg.color;
    document.getElementById('side-icon').textContent = cfg.label[0] || '?';
    document.getElementById('side-title').textContent = cfg.label;

    let html = '';

    html += '<div class="section">';
    html += '<h3>Описание типа</h3>';
    html += '<div class="field"><div class="val">' + escapeHtml(cfg.description) + '</div></div>';
    html += '</div>';

    html += '<div class="section">';
    html += '<h3>Концы связи</h3>';
    html += '<div class="field"><div class="key">source</div><div class="val">' + escapeHtml(source ? (source.data.title || source.id) : e.source) + ' <span style="opacity:0.5">(' + escapeHtml(e.source) + ')</span></div></div>';
    html += '<div class="field"><div class="key">target</div><div class="val">' + escapeHtml(target ? (target.data.title || target.id) : e.target) + ' <span style="opacity:0.5">(' + escapeHtml(e.target) + ')</span></div></div>';
    html += '</div>';

    if (reason) {
      html += '<div class="section">';
      html += '<h3>Algorithm Reason (X-ray)</h3>';
      html += '<div class="reason-box">' + escapeHtml(reason) + '</div>';
      html += '</div>';
    }

    if (e.data) {
      html += '<div class="section">';
      html += '<h3>Edge data (полный X-ray)</h3>';
      html += metaTableHtml(e.data);
      html += '</div>';
    }

    // SVO triples
    if (e.data && e.data.svoTriples && Array.isArray(e.data.svoTriples) && e.data.svoTriples.length > 0) {
      html += '<div class="section">';
      html += '<h3>SVO Triples (' + e.data.svoTriples.length + ')</h3>';
      for (const svo of e.data.svoTriples.slice(0, 20)) {
        html += '<div class="svo-triple">' + escapeHtml(formatValue(svo)) + '</div>';
      }
      html += '</div>';
    }

    // J-value (highlight)
    if (e.data && e.data.jValue !== undefined) {
      html += '<div class="section">';
      html += '<h3>J-value (POLER)</h3>';
      html += '<div class="field"><div class="val mono" style="font-size:14px;color:#9333EA;font-weight:600">J = ' + escapeHtml(formatValue(e.data.jValue)) + '</div></div>';
      html += '</div>';
    }

    html += '<div class="section">';
    html += '<h3>ID</h3>';
    html += '<div class="field"><div class="val mono">' + escapeHtml(e.id) + '</div></div>';
    html += '</div>';

    document.getElementById('side-body').innerHTML = html;
  }

  // ===== Controls =====
  document.getElementById('btn-fit').addEventListener('click', fitView);
  document.getElementById('btn-zoom-in').addEventListener('click', () => zoomBy(1.2));
  document.getElementById('btn-zoom-out').addEventListener('click', () => zoomBy(1 / 1.2));

  document.getElementById('search').addEventListener('input', (e) => {
    state.searchQuery = e.target.value;
    render();
  });

  document.getElementById('toggle-bg').addEventListener('change', (e) => {
    state.showBg = e.target.checked;
    render();
  });

  document.getElementById('bg-opacity').addEventListener('input', (e) => {
    state.bgOpacity = parseInt(e.target.value, 10) / 100;
    render();
  });

  document.getElementById('side-close').addEventListener('click', () => {
    state.selectedNodeId = null;
    state.selectedEdgeId = null;
    clearSidebar();
    render();
  });

  // ===== Keyboard =====
  document.addEventListener('keydown', (e) => {
    // Don't interfere with input fields
    if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA') return;
    const step = 40;
    switch (e.key) {
      case 'f': case 'F': fitView(); break;
      case '+': case '=': zoomBy(1.2); break;
      case '-': case '_': zoomBy(1 / 1.2); break;
      case 'ArrowUp': state.viewport.y += step; render(); break;
      case 'ArrowDown': state.viewport.y -= step; render(); break;
      case 'ArrowLeft': state.viewport.x += step; render(); break;
      case 'ArrowRight': state.viewport.x -= step; render(); break;
      case 'Escape':
        state.selectedNodeId = null;
        state.selectedEdgeId = null;
        clearSidebar();
        render();
        break;
    }
  });

  // ===== Topbar / statusbar / legend fill =====
  function fillHeader() {
    document.getElementById('proj-title').textContent = DATA.project.title || 'Untitled';
    const date = new Date(DATA.exportedAt).toLocaleString('ru-RU');
    document.getElementById('proj-meta').textContent =
      'by ' + (DATA.project.author || '—') + ' · exported ' + date +
      ' · parser v' + DATA.project.parserVersion;
    document.getElementById('stat-nodes').textContent = DATA.counts.nodes;
    document.getElementById('stat-edges').textContent = DATA.counts.edges;
    updateZoomLabel();
  }

  function fillLegend() {
    const nodesBox = document.getElementById('legend-nodes');
    let html = '';
    const order = ['chapter','scene','plotpoint','conflict','character','dialogue','location','theme','idea'];
    for (const t of order) {
      const cfg = NODE_TYPES[t];
      if (!cfg) continue;
      const cnt = DATA.counts.byType[t] || 0;
      if (cnt === 0) continue;
      html += '<div class="row"><span class="swatch" style="background:' + cfg.color + '"></span>' +
              cfg.singular + ' (' + cnt + ')</div>';
    }
    nodesBox.innerHTML = html;

    const edgesBox = document.getElementById('legend-edges');
    let ehtml = '';
    const eorder = ['flow','cause','character','location','conflict','foreshadow','reference','alternative','theme'];
    for (const k of eorder) {
      const cfg = EDGE_TYPES[k];
      if (!cfg) continue;
      const cnt = DATA.counts.byEdgeKind[k] || 0;
      if (cnt === 0) continue;
      ehtml += '<div class="row' + (cfg.dashed ? ' dashed' : '') + '" style="color:' + cfg.color + '">' +
               '<span class="swatch" style="background:' + (cfg.dashed ? 'transparent' : cfg.color) + ';border:1px solid ' + cfg.color + '"></span>' +
               '<span style="color:#1c1917">' + cfg.label + ' (' + cnt + ')</span></div>';
    }
    edgesBox.innerHTML = ehtml;
  }

  // ===== Init =====
  fillHeader();
  fillLegend();
  resizeCanvas();
  // Initial fit only if no viewport was provided
  if (!DATA.viewport) {
    setTimeout(fitView, 50);
  } else {
    updateZoomLabel();
  }
})();
</script>
</body>
</html>
`;

// ===== Главная функция =====

/**
 * Сериализовать текущее состояние рабочего стола в самодостаточный HTML файл.
 *
 * Возвращает строку — готовый .html контент, который можно:
 *   - сохранить через Tauri fs / browser download
 *   - открыть в любом браузере (double-click)
 *   - прислать мне (AI) — я открою и смогу интерактивно изучить граф
 */
export function exportWorkspaceToHtml(
  nodes: LitNode[],
  edges: LitEdge[],
  background: BackgroundLayer | null,
  viewport: { x: number; y: number; zoom: number } | null,
  opts: ExportHtmlOptions,
): string {
  // Build reasons map
  const nodeReasons: Record<string, string> = {};
  for (const n of nodes) nodeReasons[n.id] = buildNodeReason(n);

  const edgeReasons: Record<string, string> = {};
  for (const e of edges) {
    const s = nodes.find((n) => n.id === e.source);
    const t = nodes.find((n) => n.id === e.target);
    edgeReasons[e.id] = buildEdgeReason(e, s, t);
  }

  // Smart X-Ray: прогон всех нод через эвристики.
  // Это пост-процессинг — НЕ меняем сами ноды, только добавляем диагностику.
  const diagMap = analyzeWorkspace(nodes, edges);
  const diagnostics: Record<string, NodeDiagnostic> = {};
  let okCount = 0, suspectCount = 0, errorCount = 0, totalWarn = 0, totalSug = 0;
  for (const [nid, d] of diagMap) {
    diagnostics[nid] = d;
    if (d.level === "ok") okCount++;
    else if (d.level === "suspect") suspectCount++;
    else errorCount++;
    totalWarn += d.warnings.length;
    totalSug += d.suggestions.length;
  }
  const diagnosticsSummary = {
    total: nodes.length,
    ok: okCount,
    suspect: suspectCount,
    error: errorCount,
    warnings: totalWarn,
    suggestions: totalSug,
  };

  const data: EmbeddedData = {
    schema: "litgraph-xray-html/v1",
    exportedAt: Date.now(),
    project: {
      title: opts.title || "Untitled",
      author: opts.author || "",
      description: opts.description || "",
      parserVersion: opts.parserVersion ?? "0.2.2",
      sourceMdHash: opts.sourceMdHash ?? "",
      createdAt: opts.createdAt ?? Date.now(),
    },
    viewport,
    background,
    nodes,
    edges,
    nodeReasons,
    edgeReasons,
    nodeHeights: {},
    nodeTypeConfig: NODE_TYPES,
    edgeTypeConfig: EDGE_TYPES,
    counts: {
      nodes: nodes.length,
      edges: edges.length,
      byType: countByType(nodes),
      byEdgeKind: countByEdgeKind(edges),
    },
    analysis: opts.analysisSnapshot,
    diagnostics,
    diagnosticsSummary,
  };

  // Stringify with safety: avoid </script> in data breaking out
  const jsonStr = JSON.stringify(data, null, 2).replace(/<\//g, "<\\/");

  const titleForPlaceholder = (opts.title || "Untitled").replace(/[<>&"]/g, "");

  return HTML_TEMPLATE
    .replace("__LITGRAPH_DATA_JSON__", jsonStr)
    .replace("__TITLE_PLACEHOLDER__", titleForPlaceholder);
}

// ===== Сохранение через Tauri dialog =====

/**
 * Открыть системный диалог "Сохранить как…" и записать HTML файл.
 *
 * Tauri: dialog:save + fs:writeTextFile.
 * Browser fallback: Blob + a[download].
 *
 * Возвращает true если файл сохранён, false если отменён.
 */
export async function saveHtmlViaDialog(
  htmlContent: string,
  suggestedName: string,
): Promise<boolean> {
  const isTauri =
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

  if (isTauri) {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");

      const filePath = await save({
        defaultPath: suggestedName.endsWith(".html")
          ? suggestedName
          : `${suggestedName}.html`,
        filters: [{ name: "HTML (X-ray mini-program)", extensions: ["html"] }],
      });

      if (!filePath) return false;

      await writeTextFile(filePath, htmlContent);
      return true;
    } catch (err) {
      console.error(
        "[LitGraph] Tauri save HTML failed, falling back to download:",
        err,
      );
      // проваливаемся в браузерный fallback
    }
  }

  // Браузерный fallback
  try {
    const blob = new Blob([htmlContent], { type: "text/html" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = suggestedName.endsWith(".html")
      ? suggestedName
      : `${suggestedName}.html`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return true;
  } catch (err) {
    console.error("[LitGraph] Browser download HTML failed:", err);
    return false;
  }
}
