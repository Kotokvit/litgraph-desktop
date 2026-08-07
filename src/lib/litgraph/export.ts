import type { LitNode, LitProject } from "./types";
import { NODE_TYPES, EDGE_TYPES } from "./types";

// ====== Экспорт в обычный текст (читаемый сценарий) ======

export function exportToText(project: LitProject): string {
  const { title, author, description, nodes, edges } = project;

  const lines: string[] = [];
  lines.push(`${title}`);
  lines.push(`Автор: ${author}`);
  if (description) lines.push(``);
  if (description) lines.push(description);
  lines.push("");
  lines.push("========================================");
  lines.push("");

  // Группируем ноды по типам
  const grouped: Record<string, LitNode[]> = {};
  for (const n of nodes) {
    if (!grouped[n.type]) grouped[n.type] = [];
    grouped[n.type].push(n);
  }

  const typeOrder = [
    "chapter",
    "scene",
    "plotpoint",
    "conflict",
    "character",
    "dialogue",
    "location",
    "idea",
  ];

  for (const t of typeOrder) {
    if (!grouped[t] || grouped[t].length === 0) continue;
    const cfg = NODE_TYPES[t as keyof typeof NODE_TYPES];
    lines.push(`◆ ${cfg.plural.toUpperCase()} (${grouped[t].length})`);
    lines.push("────────────────────────────────────────");
    grouped[t].forEach((n, i) => {
      lines.push(`${i + 1}. ${n.data.title}`);
      if (n.data.body) lines.push(`   ${n.data.body.replace(/\n/g, "\n   ")}`);
      const meta = n.data.meta ?? {};
      const metaEntries = Object.entries(meta).filter(([, v]) => v);
      if (metaEntries.length) {
        lines.push(`   [метаданные]`);
        for (const [k, v] of metaEntries) {
          lines.push(`   • ${k}: ${v}`);
        }
      }
      if (n.data.tags?.length) {
        lines.push(`   #${n.data.tags.join(" #")}`);
      }
      lines.push("");
    });
    lines.push("");
  }

  // Связи
  if (edges.length > 0) {
    lines.push("◆ СВЯЗИ");
    lines.push("────────────────────────────────────────");
    edges.forEach((e, i) => {
      const src = nodes.find((n) => n.id === e.source);
      const tgt = nodes.find((n) => n.id === e.target);
      if (!src || !tgt) return;
      const kindCfg = e.data?.kind ? EDGE_TYPES[e.data.kind] : null;
      const kindLabel = kindCfg ? kindCfg.label : "связь";
      lines.push(
        `${i + 1}. [${kindLabel}] ${src.data.title} → ${tgt.data.title}`
      );
      if (e.data?.note) lines.push(`   ${e.data.note}`);
    });
    lines.push("");
  }

  // Сценарий по потоку: идём от глав и сцен по flow-рёбрам
  const flowEdges = edges.filter((e) => e.data?.kind === "flow" || !e.data?.kind);
  if (flowEdges.length > 0) {
    lines.push("========================================");
    lines.push("ПОСЛЕДОВАТЕЛЬНОСТЬ СЦЕН (по потоку)");
    lines.push("========================================");
    lines.push("");

    // Находим стартовые ноды (у которых нет входящих flow-рёбер, но есть хотя бы одна сцена/глава)
    const targetsWithFlow = new Set(flowEdges.map((e) => e.target));
    const startNodes = nodes.filter(
      (n) =>
        (n.type === "chapter" || n.type === "scene") &&
        !targetsWithFlow.has(n.id)
    );

    const visited = new Set<string>();
    const ordered: LitNode[] = [];

    function walk(id: string) {
      if (visited.has(id)) return;
      visited.add(id);
      const node = nodes.find((n) => n.id === id);
      if (node) ordered.push(node);
      const next = flowEdges
        .filter((e) => e.source === id)
        .map((e) => e.target);
      for (const nx of next) walk(nx);
    }

    if (startNodes.length === 0) {
      // если нет стартовых — берём первую сцену
      const firstScene = nodes.find((n) => n.type === "scene" || n.type === "chapter");
      if (firstScene) walk(firstScene.id);
    } else {
      for (const s of startNodes) walk(s.id);
    }

    ordered.forEach((n, i) => {
      const cfg = NODE_TYPES[n.type];
      lines.push(`${i + 1}. [${cfg.singular}] ${n.data.title}`);
      if (n.data.body) lines.push(`   ${n.data.body}`);
      lines.push("");
    });
  }

  return lines.join("\n");
}

// ====== Экспорт в Markdown ======

export function exportToMarkdown(project: LitProject): string {
  const { title, author, description, nodes, edges } = project;
  const lines: string[] = [];

  lines.push(`# ${title}`);
  lines.push("");
  lines.push(`**Автор:** ${author}  `);
  if (description) {
    lines.push("");
    lines.push(`> ${description}`);
  }
  lines.push("");

  // Группировка
  const grouped: Record<string, LitNode[]> = {};
  for (const n of nodes) {
    if (!grouped[n.type]) grouped[n.type] = [];
    grouped[n.type].push(n);
  }

  const typeOrder = ["chapter", "scene", "plotpoint", "conflict", "character", "dialogue", "location", "idea"];

  for (const t of typeOrder) {
    if (!grouped[t] || grouped[t].length === 0) continue;
    const cfg = NODE_TYPES[t as keyof typeof NODE_TYPES];
    lines.push(`## ${cfg.plural}`);
    lines.push("");
    grouped[t].forEach((n, i) => {
      lines.push(`### ${i + 1}. ${n.data.title}`);
      lines.push("");
      if (n.data.body) {
        lines.push(n.data.body);
        lines.push("");
      }
      const meta = n.data.meta ?? {};
      const metaEntries = Object.entries(meta).filter(([, v]) => v);
      if (metaEntries.length) {
        for (const [k, v] of metaEntries) {
          lines.push(`- **${k}:** ${v}`);
        }
        lines.push("");
      }
      if (n.data.tags?.length) {
        lines.push(`Теги: ${n.data.tags.map((t) => `\`#${t}\``).join(" ")}`);
        lines.push("");
      }
    });
  }

  if (edges.length > 0) {
    lines.push(`## Связи`);
    lines.push("");
    edges.forEach((e) => {
      const src = nodes.find((n) => n.id === e.source);
      const tgt = nodes.find((n) => n.id === e.target);
      if (!src || !tgt) return;
      const kindCfg = e.data?.kind ? EDGE_TYPES[e.data.kind] : null;
      const kindLabel = kindCfg ? kindCfg.label : "связь";
      lines.push(`- **[${kindLabel}]** ${src.data.title} → ${tgt.data.title}`);
    });
  }

  return lines.join("\n");
}

// ====== Скачивание файла (через Tauri save dialog) ======

export async function downloadFile(content: string, filename: string, mime = "text/plain") {
  // Tauri: открываем диалог сохранения файла
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const { writeTextFile } = await import("@tauri-apps/plugin-fs");
    const path = await save({
      defaultPath: filename,
      filters: [{
        name: mime.includes("markdown") || filename.endsWith(".md") ? "Markdown" :
              filename.endsWith(".json") ? "JSON" : "Text",
        extensions: [filename.split(".").pop() || "txt"],
      }],
    });
    if (path) {
      await writeTextFile(path, content);
    }
  } catch (err) {
    console.error("downloadFile error:", err);
    // Fallback на браузерный метод (для dev-режима без Tauri)
    const blob = new Blob([content], { type: `${mime};charset=utf-8` });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  }
}

export function slugify(s: string): string {
  return s
    .toLowerCase()
    .replace(/[^a-z0-9а-я]+/gi, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 40) || "litgraph";
}
