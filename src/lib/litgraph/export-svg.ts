/**
 * export-svg.ts
 * =============
 * Фаза A (Centaur Manifest, Task ID 12):
 *   SVG X-ray экспорт рабочего стола LitGraph.
 *
 * Что делает:
 *   1. Берёт текущее состояние store: nodes, edges, backgroundLayer, viewport.
 *   2. Сериализует всё в один самодостаточный .svg файл.
 *   3. В SVG встраиваются:
 *      - <image> с background (data: URL base64) — самодостаточный файл
 *      - <g data-node-id="..." data-type="..." data-reason="..."> для каждой ноды
 *      - <path data-edge-id="..." data-edge-kind="..." data-j="..."> для каждой связи
 *      - <metadata> с parser_version, source_md5, timestamp
 *
 * Зачем:
 *   Когда пользователь шлёт этот SVG мне (AI), я вижу не только картинку,
 *   но и АЛГОРИТМИЧЕСКУЮ ЛОГИКУ (через data-* атрибуты): почему нода
 *   там оказалась, какой confidence, какие aliases, какой epsilon и т.д.
 *   Это и есть "векторное представление мозга алгоритма".
 *
 * SVG rationale:
 *   - Векторный → бесконечный зум без пикселизации
 *   - Текстовые data-* атрибуты видны через DevTools или в исходнике
 *   - Нативно рендерится браузером, Inkscape, Figma
 *   - В отличие от PNG/TIFF, не теряет метаинформацию
 */

import type { LitNode, LitEdge, BackgroundLayer, EdgeKind } from "./types";
import { NODE_TYPES, EDGE_TYPES } from "./types";

// ====== Константы (должны совпадать с CanvasRenderer.tsx) ======
const NODE_WIDTH = 260;
const NODE_BASE_HEIGHT = 70;
const NODE_MAX_HEIGHT = 140;

function getNodeHeight(node: { type: string; data: { meta?: Record<string, unknown> } }): number {
  if (node.type !== "chapter") return NODE_BASE_HEIGHT;
  const epsilon = (node.data.meta?.epsilon as number) ?? 30;
  return NODE_BASE_HEIGHT + (epsilon / 100) * (NODE_MAX_HEIGHT - NODE_BASE_HEIGHT);
}

// ====== Утилиты ======

/** Экранировать строку для XML attribute value. */
function escapeXmlAttr(s: string): string {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&apos;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    // Контрольные символы — XML их не любит
    .replace(/[\x00-\x08\x0B\x0C\x0E-\x1F]/g, "");
}

/** Экранировать строку для XML text content. */
function escapeXmlText(s: string): string {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/[\x00-\x08\x0B\x0C\x0E-\x1F]/g, "");
}

/** Обрезать длинный текст до N символов с многоточием. */
function truncate(s: string, max: number): string {
  if (s.length <= max) return s;
  // Безопасный срез по char boundary (для эмодзи/кириллицы)
  let end = max;
  while (end > 0) {
    const c = s.codePointAt(end - 1);
    if (c === undefined) break;
    // Суррогаты: если это low surrogate, откатимся
    if (c >= 0xDC00 && c <= 0xDFFF) {
      end -= 1;
      continue;
    }
    break;
  }
  return s.slice(0, end) + "…";
}

/**
 * Безопасно сериализовать value в строку для data-* атрибута.
 * Числа/булевы → как есть; объекты → компактный JSON; строки → как есть.
 */
function toDataAttr(value: unknown): string {
  if (value === null || value === undefined) return "";
  if (typeof value === "string") return value;
  if (typeof value === "number" || typeof value === "boolean") return String(value);
  try {
    return JSON.stringify(value);
  } catch {
    return "[unserializable]";
  }
}

// ====== Сбор данных "почему" (reason) ======

/**
 * Для каждой ноды собрать объяснение алгоритма: почему она тут.
 * Читает meta поля которые парсер уже кладёт в node.data.meta.
 * Это и есть X-ray —.visible reason of the algorithm.
 */
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
      // Если есть tags главы (например "пролог")
      if (Array.isArray(node.data.tags) && node.data.tags.length > 0) {
        parts.push(`tags=[${node.data.tags.join(",")}]`);
      }
      return parts.length ? `chapter:${parts.join(";")}` : "chapter:manual";
    }
    case "character": {
      if (meta.mentions !== undefined) parts.push(`freq=${meta.mentions}`);
      if (meta.chapters) parts.push(`in=${meta.chapters}`);
      if (meta.firstChapter) parts.push(`first=${meta.firstChapter}`);
      // aliases в body часто присутствуют как "Формы: X, Y"
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
      return `scene:${meta.pov ? `pov=${meta.pov};` : ""}${meta.mood ? `mood=${meta.mood};` : ""}${meta.timeOfDay ? `time=${meta.timeOfDay}` : ""}`.replace(/;$/, "") || "scene:manual";
    case "plotpoint":
      return `plotpoint:${meta.importance ? `imp=${meta.importance}` : "imp=?"}`;
    case "conflict":
      return `conflict:${meta.importance ? `imp=${meta.importance}` : "imp=?"}`;
    case "dialogue":
      return "dialogue:manual";
    case "theme":
      return `theme:${meta.importance ? `imp=${meta.importance};` : ""}${meta.manifestation ? `manifest=${truncate(String(meta.manifestation), 40)}` : ""}`.replace(/;$/, "") || "theme:manual";
    case "idea":
      return "idea:manual";
    default:
      return "unknown:manual";
  }
}

/**
 * Для каждой связи собрать объяснение: какие SVO-данные её породили.
 */
function buildEdgeReason(edge: LitEdge, source?: LitNode, target?: LitNode): string {
  const kind = edge.data?.kind ?? "flow";
  const parts: string[] = [`kind=${kind}`];

  switch (kind) {
    case "flow": {
      // flow-связи между главами: толщина зависит от epsilon
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
      if (edge.data?.note) parts.push(`note="${truncate(edge.data.note, 60)}"`);
      break;
    case "character":
      parts.push(`reason=char_mentioned_in_chapter`);
      if (source?.data.meta?.mentions) parts.push(`char_freq=${source.data.meta.mentions}`);
      break;
    case "location":
      parts.push(`reason=loc_appears_in_chapter`);
      if (source?.data.meta?.mentions) parts.push(`loc_freq=${source.data.meta.mentions}`);
      break;
    case "conflict":
      // SVO → J-матрица
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

// ====== SVG builders ======

interface SvgContext {
  nodes: LitNode[];
  edges: LitEdge[];
  background: BackgroundLayer | null;
  viewport: { x: number; y: number; zoom: number } | null;
  projectMeta: {
    title: string;
    author: string;
    description: string;
    parserVersion: string;
    sourceMdHash?: string;
    createdAt: number;
    exportedAt: number;
  };
  /** Опционально: данные последних анализов (POLER, SVO, NER) — если есть. */
  analysisSnapshot?: {
    poler?: unknown;
    conflict?: unknown;
    ner?: unknown;
  };
}

/** Вычислить bbox всего графа + background. */
function computeBounds(ctx: SvgContext): { minX: number; minY: number; maxX: number; maxY: number } {
  let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;

  if (ctx.background) {
    const bg = ctx.background;
    const dw = bg.naturalWidth * bg.scale;
    const dh = bg.naturalHeight * bg.scale;
    minX = Math.min(minX, bg.x);
    minY = Math.min(minY, bg.y);
    maxX = Math.max(maxX, bg.x + dw);
    maxY = Math.max(maxY, bg.y + dh);
  }

  for (const n of ctx.nodes) {
    const h = getNodeHeight(n);
    minX = Math.min(minX, n.position.x);
    minY = Math.min(minY, n.position.y);
    maxX = Math.max(maxX, n.position.x + NODE_WIDTH);
    maxY = Math.max(maxY, n.position.y + h);
  }

  if (!isFinite(minX)) {
    // Пустой граф — дефолт
    return { minX: 0, minY: 0, maxX: 1200, maxY: 800 };
  }

  // Padding
  const pad = 60;
  return { minX: minX - pad, minY: minY - pad, maxX: maxX + pad, maxY: maxY + pad };
}

function buildBackgroundSvg(bg: BackgroundLayer): string {
  const dw = bg.naturalWidth * bg.scale;
  const dh = bg.naturalHeight * bg.scale;

  // Если фон уже data: URL — берём как есть.
  // Если object URL — не сможем встроить (нужен fetch + base64).
  // Но мы в importBackgroundImage всегда храним data: URL.
  const href = bg.src.startsWith("data:") ? bg.src : "";

  const transform = bg.rotation
    ? ` transform="rotate(${bg.rotation} ${bg.x + dw / 2} ${bg.y + dh / 2})"`
    : "";

  const opacityAttr = bg.opacity !== 1 ? ` opacity="${bg.opacity}"` : "";

  const imageEl = href
    ? `<image href="${escapeXmlAttr(href)}" x="${bg.x}" y="${bg.y}" width="${dw}" height="${dh}"${opacityAttr}${transform} preserveAspectRatio="none"/>`
    : `<rect x="${bg.x}" y="${bg.y}" width="${dw}" height="${dh}" fill="#eee" stroke="#888" stroke-dasharray="8 4"${opacityAttr}${transform}/>`;

  return `  <g id="background" data-bg-id="${escapeXmlAttr(bg.id)}" data-bg-name="${escapeXmlAttr(bg.name)}" data-bg-format="${bg.format}" data-bg-natural-w="${bg.naturalWidth}" data-bg-natural-h="${bg.naturalHeight}" data-bg-scale="${bg.scale}" data-bg-locked="${bg.locked}">
    ${imageEl}
  </g>
`;
}

function buildNodeSvg(node: LitNode): string {
  const cfg = NODE_TYPES[node.type as keyof typeof NODE_TYPES] || NODE_TYPES.idea;
  const h = getNodeHeight(node);
  const x = node.position.x;
  const y = node.position.y;

  const reason = buildNodeReason(node);
  const meta = node.data.meta ?? {};

  // Собрать data-* атрибуты из meta (для X-ray анализа)
  const metaDataAttrs: string[] = [];
  for (const [k, v] of Object.entries(meta)) {
    const val = toDataAttr(v);
    if (val && val.length < 500) {
      metaDataAttrs.push(`data-meta-${escapeXmlAttr(k)}="${escapeXmlAttr(val)}"`);
    }
  }

  // Если есть tags — тоже в data
  const tagsAttr = (node.data.tags && node.data.tags.length > 0)
    ? ` data-tags="${escapeXmlAttr(node.data.tags.join(","))}"`
    : "";

  // Иконка — круг с буквой
  const iconLetter = cfg.singular[0] || "?";
  const iconColor = cfg.color;

  // Epsilon-бейдж для глав
  let epsilonBadge = "";
  let epsilonBar = "";
  if (node.type === "chapter" && meta.epsilon !== undefined) {
    const eps = Number(meta.epsilon);
    const epsColor = eps > 70 ? "#dc2626" : eps > 40 ? "#d97706" : "#78716c";
    const epsBarColor = eps > 70 ? "#dc2626" : eps > 40 ? "#f59e0b" : "#65a30d";
    const barW = NODE_WIDTH - 8;
    epsilonBadge = `<text x="${x + NODE_WIDTH - 12}" y="${y + 14}" font-size="9" font-weight="600" fill="${epsColor}" text-anchor="end" dominant-baseline="middle">ε${Math.round(eps)}</text>`;
    epsilonBar = `<rect x="${x + 4}" y="${y + h - 6}" width="${barW}" height="4" fill="#f0f0f0"/><rect x="${x + 4}" y="${y + h - 6}" width="${(barW * eps) / 100}" height="4" fill="${epsBarColor}"/>`;
  }

  // Цветная полоса слева
  const leftBar = `<rect x="${x}" y="${y}" width="4" height="${h}" fill="${iconColor}" rx="2"/>`;

  // Шапка
  const headerBg = `<rect x="${x + 4}" y="${y}" width="${NODE_WIDTH - 4}" height="28" fill="${iconColor}28" rx="8"/>`;

  // Круг с буквой
  const iconCircle = `<circle cx="${x + 20}" cy="${y + 14}" r="9" fill="${iconColor}"/><text x="${x + 20}" y="${y + 14}" font-size="10" font-weight="bold" fill="#fff" text-anchor="middle" dominant-baseline="middle">${escapeXmlText(iconLetter)}</text>`;

  // Тип ноды
  const typeLabel = `<text x="${x + 35}" y="${y + 14}" font-size="10" font-weight="600" fill="${iconColor}" dominant-baseline="middle">${escapeXmlText(cfg.singular.toUpperCase())}</text>`;

  // Заголовок
  const title = truncate(node.data.title || "Без названия", 60);
  const titleEl = `<text x="${x + 12}" y="${y + 34}" font-size="13" font-weight="600" fill="#292524">${escapeXmlText(title)}</text>`;

  // Тело (превью, 2 строки)
  let bodyEl = "";
  if (node.data.body) {
    const bodyText = truncate(node.data.body.replace(/\s+/g, " ").trim(), 120);
    bodyEl = `<text x="${x + 12}" y="${y + 54}" font-size="11" fill="#78716c">${escapeXmlText(bodyText)}</text>`;
  }

  // Handle circles (как в canvas)
  const handles = `
    <circle cx="${x}" cy="${y + h / 2}" r="6" fill="#fff" stroke="${iconColor}" stroke-width="2"/>
    <circle cx="${x + NODE_WIDTH}" cy="${y + h / 2}" r="6" fill="#fff" stroke="${iconColor}" stroke-width="2"/>`;

  return `  <g class="litgraph-node" data-node-id="${escapeXmlAttr(node.id)}" data-node-type="${escapeXmlAttr(node.type)}" data-node-title="${escapeXmlAttr(node.data.title || "")}" data-reason="${escapeXmlAttr(reason)}"${tagsAttr}${metaDataAttrs.length ? " " + metaDataAttrs.join(" ") : ""}>
    <rect x="${x}" y="${y}" width="${NODE_WIDTH}" height="${h}" fill="#fff" stroke="${iconColor}40" stroke-width="1" rx="11"/>
    ${leftBar}
    ${headerBg}
    ${iconCircle}
    ${typeLabel}
    ${epsilonBadge}
    ${titleEl}
    ${bodyEl}
    ${epsilonBar}
    ${handles}
  </g>
`;
}

function buildEdgeSvg(edge: LitEdge, nodes: LitNode[]): string {
  const source = nodes.find((n) => n.id === edge.source);
  const target = nodes.find((n) => n.id === edge.target);
  if (!source || !target) return "";

  const kind = (edge.data?.kind ?? "flow") as EdgeKind;
  const cfg = EDGE_TYPES[kind] || EDGE_TYPES.flow;

  const sx = source.position.x + NODE_WIDTH;
  const sy = source.position.y + getNodeHeight(source) / 2;
  const tx = target.position.x;
  const ty = target.position.y + getNodeHeight(target) / 2;

  // Bezier control points (как в CanvasRenderer)
  const dx = tx - sx;
  const cp1x = sx + dx * 0.5;
  const cp1y = sy;
  const cp2x = tx - dx * 0.5;
  const cp2y = ty;

  const path = `M ${sx} ${sy} C ${cp1x} ${cp1y}, ${cp2x} ${cp2y}, ${tx} ${ty}`;

  // Резонанс: толщина flow-связи зависит от epsilon
  let strokeWidth = 2;
  if (kind === "flow") {
    const srcEps = (source.data.meta?.epsilon as number) ?? 30;
    const tgtEps = (target.data.meta?.epsilon as number) ?? 30;
    const avgEps = (srcEps + tgtEps) / 2;
    strokeWidth = 1 + (avgEps / 100) * 4;
  }

  const dashArray = cfg.dashed ? ` stroke-dasharray="6 4"` : "";
  const reason = buildEdgeReason(edge, source, target);

  // J-значение если есть
  const jAttr = edge.data?.jValue !== undefined ? ` data-j-value="${edge.data.jValue}"` : "";
  const svoAttr = edge.data?.svoTriples && Array.isArray(edge.data.svoTriples)
    ? ` data-svo-triples="${escapeXmlAttr(JSON.stringify(edge.data.svoTriples))}"`
    : "";
  const noteAttr = edge.data?.note ? ` data-note="${escapeXmlAttr(truncate(edge.data.note, 200))}"` : "";

  // Лейбл связи (в центре)
  const labelX = (sx + tx) / 2;
  const labelY = (sy + ty) / 2;
  const labelW = cfg.label.length * 6 + 12;
  const labelH = 14;
  const labelEl = `
    <g class="litgraph-edge-label">
      <rect x="${labelX - labelW / 2}" y="${labelY - labelH / 2}" width="${labelW}" height="${labelH}" fill="#fff" stroke="${cfg.color}40" rx="7"/>
      <text x="${labelX}" y="${labelY}" font-size="10" fill="${cfg.color}" text-anchor="middle" dominant-baseline="middle">${escapeXmlText(cfg.label)}</text>
    </g>`;

  return `  <g class="litgraph-edge" data-edge-id="${escapeXmlAttr(edge.id)}" data-edge-kind="${escapeXmlAttr(kind)}" data-source="${escapeXmlAttr(edge.source)}" data-target="${escapeXmlAttr(edge.target)}" data-reason="${escapeXmlAttr(reason)}"${jAttr}${svoAttr}${noteAttr}>
    <path d="${path}" fill="none" stroke="${cfg.color}" stroke-width="${strokeWidth}"${dashArray}/>
    ${labelEl}
  </g>
`;
}

function buildMetadata(ctx: SvgContext): string {
  const meta = ctx.projectMeta;
  const analysis = ctx.analysisSnapshot ?? {};

  // Встраиваем snapshot последних анализов как CDATA (для полной картины)
  let polerJson = "";
  let conflictJson = "";
  let nerJson = "";
  try {
    if (analysis.poler) polerJson = JSON.stringify(analysis.poler, null, 2);
    if (analysis.conflict) conflictJson = JSON.stringify(analysis.conflict, null, 2);
    if (analysis.ner) nerJson = JSON.stringify(analysis.ner, null, 2);
  } catch {
    // ignore
  }

  const counts = {
    nodes: ctx.nodes.length,
    edges: ctx.edges.length,
    byType: countByType(ctx.nodes),
    byEdgeKind: countByEdgeKind(ctx.edges),
  };

  return `  <metadata xmlns:litgraph="https://litgraph.dev/ns">
    <litgraph:project>
      <litgraph:title>${escapeXmlText(meta.title)}</litgraph:title>
      <litgraph:author>${escapeXmlText(meta.author)}</litgraph:author>
      <litgraph:description>${escapeXmlText(truncate(meta.description, 500))}</litgraph:description>
      <litgraph:parserVersion>${escapeXmlText(meta.parserVersion)}</litgraph:parserVersion>
      <litgraph:sourceMdHash>${escapeXmlText(meta.sourceMdHash ?? "")}</litgraph:sourceMdHash>
      <litgraph:createdAt>${meta.createdAt}</litgraph:createdAt>
      <litgraph:exportedAt>${meta.exportedAt}</litgraph:exportedAt>
    </litgraph:project>
    <litgraph:counts>
      <litgraph:nodes total="${counts.nodes}"/>
      <litgraph:edges total="${counts.edges}"/>
      <litgraph:byType>${escapeXmlText(JSON.stringify(counts.byType))}</litgraph:byType>
      <litgraph:byEdgeKind>${escapeXmlText(JSON.stringify(counts.byEdgeKind))}</litgraph:byEdgeKind>
    </litgraph:counts>
    <litgraph:viewport>
      <litgraph:x>${ctx.viewport?.x ?? 0}</litgraph:x>
      <litgraph:y>${ctx.viewport?.y ?? 0}</litgraph:y>
      <litgraph:zoom>${ctx.viewport?.zoom ?? 1}</litgraph:zoom>
    </litgraph:viewport>
    ${polerJson ? `<litgraph:polerSnapshot><![CDATA[${polerJson}]]></litgraph:polerSnapshot>` : ""}
    ${conflictJson ? `<litgraph:conflictSnapshot><![CDATA[${conflictJson}]]></litgraph:conflictSnapshot>` : ""}
    ${nerJson ? `<litgraph:nerSnapshot><![CDATA[${nerJson}]]></litgraph:nerSnapshot>` : ""}
  </metadata>
`;
}

function countByType(nodes: LitNode[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const n of nodes) {
    counts[n.type] = (counts[n.type] ?? 0) + 1;
  }
  return counts;
}

function countByEdgeKind(edges: LitEdge[]): Record<string, number> {
  const counts: Record<string, number> = {};
  for (const e of edges) {
    const k = e.data?.kind ?? "flow";
    counts[k] = (counts[k] ?? 0) + 1;
  }
  return counts;
}

// ====== Главная функция ======

export interface ExportSvgOptions {
  /** Заголовок проекта. */
  title: string;
  author: string;
  description: string;
  /** Версия парсера (для воспроизводимости). */
  parserVersion?: string;
  /** Хеш исходного .md (если есть). */
  sourceMdHash?: string;
  /** Timestamp создания проекта. */
  createdAt?: number;
  /** Опционально: snapshot последних анализов. */
  analysisSnapshot?: SvgContext["analysisSnapshot"];
}

/**
 * Сериализовать текущее состояние рабочего стола в SVG-строку.
 *
 * Возвращает готовый .svg контент (string) — самодостаточный файл,
 * который можно сохранить через Tauri fs или downloadFile.
 */
export function exportWorkspaceToSvg(
  nodes: LitNode[],
  edges: LitEdge[],
  background: BackgroundLayer | null,
  viewport: { x: number; y: number; zoom: number } | null,
  opts: ExportSvgOptions,
): string {
  const ctx: SvgContext = {
    nodes,
    edges,
    background: background,
    viewport,
    projectMeta: {
      title: opts.title || "Untitled",
      author: opts.author || "",
      description: opts.description || "",
      parserVersion: opts.parserVersion ?? "0.2.2",
      sourceMdHash: opts.sourceMdHash,
      createdAt: opts.createdAt ?? Date.now(),
      exportedAt: Date.now(),
    },
    analysisSnapshot: opts.analysisSnapshot,
  };

  const bounds = computeBounds(ctx);
  const width = bounds.maxX - bounds.minX;
  const height = bounds.maxY - bounds.minY;

  // Header
  const xmlDecl = `<?xml version="1.0" encoding="UTF-8"?>\n`;
  const svgOpen = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${width}" height="${height}" viewBox="${bounds.minX} ${bounds.minY} ${width} ${height}" font-family="sans-serif">
  <!-- LitGraph Workspace X-ray export. Generated at ${new Date(ctx.projectMeta.exportedAt).toISOString()}. -->
  <!-- Каждый <g class="litgraph-node"> и <g class="litgraph-edge"> содержит data-* атрибуты -->
  <!-- с алгоритмической логикой (reason, confidence, epsilon, aliases, j-value, SVO triples). -->
  <!-- Открой этот файл в текстовом редакторе или Inkscape — все данные видны. -->
`;

  // Background grid (точки как в CanvasRenderer)
  const dotGap = 20;
  const dotXStart = Math.floor(bounds.minX / dotGap) * dotGap;
  const dotYStart = Math.floor(bounds.minY / dotGap) * dotGap;
  let dotsSvg = `  <g class="litgraph-grid" opacity="0.4">\n`;
  for (let x = dotXStart; x < bounds.maxX; x += dotGap) {
    for (let y = dotYStart; y < bounds.maxY; y += dotGap) {
      dotsSvg += `    <rect x="${x}" y="${y}" width="1.5" height="1.5" fill="#B8A88C"/>\n`;
    }
  }
  dotsSvg += `  </g>\n`;

  // Background layer
  const bgSvg = ctx.background ? buildBackgroundSvg(ctx.background) : "";

  // Edges (под нодами)
  const edgesSvg = ctx.edges
    .map((e) => buildEdgeSvg(e, ctx.nodes))
    .filter(Boolean)
    .join("\n");

  // Nodes
  const nodesSvg = ctx.nodes.map((n) => buildNodeSvg(n)).join("\n");

  // Metadata
  const metadataSvg = buildMetadata(ctx);

  const svgClose = `</svg>\n`;

  return [
    xmlDecl,
    svgOpen,
    metadataSvg,
    dotsSvg,
    bgSvg,
    `  <g class="litgraph-edges">\n`,
    edgesSvg,
    `  </g>\n`,
    `  <g class="litgraph-nodes">\n`,
    nodesSvg,
    `  </g>\n`,
    svgClose,
  ].join("");
}

// ====== Сохранение через Tauri dialog ======

/**
 * Открыть системный диалог "Сохранить как…" и записать SVG файл.
 *
 * Использует @tauri-apps/plugin-dialog (save) и @tauri-apps/plugin-fs (writeTextFile).
 * Возвращает true если файл сохранён, false если отменён.
 *
 * В браузере (без Tauri) — fallback через downloadFile (Blob + a[download]).
 */
export async function saveSvgViaDialog(svgContent: string, suggestedName: string): Promise<boolean> {
  const isTauri =
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

  if (isTauri) {
    try {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");

      const filePath = await save({
        defaultPath: suggestedName.endsWith(".svg") ? suggestedName : `${suggestedName}.svg`,
        filters: [
          { name: "SVG (X-ray)", extensions: ["svg"] },
        ],
      });

      if (!filePath) return false; // пользователь отменил

      await writeTextFile(filePath, svgContent);
      return true;
    } catch (err) {
      console.error("[LitGraph] Tauri save failed, falling back to download:", err);
      // проваливаемся в браузерный fallback
    }
  }

  // Браузерный fallback
  try {
    const blob = new Blob([svgContent], { type: "image/svg+xml" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = suggestedName.endsWith(".svg") ? suggestedName : `${suggestedName}.svg`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    setTimeout(() => URL.revokeObjectURL(url), 1000);
    return true;
  } catch (err) {
    console.error("[LitGraph] Browser download failed:", err);
    return false;
  }
}
