/**
 * regen_xray_sample.ts
 * Перегенерирует docs/xray-samples/1-сфера-предела.xray.html с новым
 * Smart X-Ray слоем (heuristics). Берёт ноды/рёбра из существующего файла,
 * прогоняет через exportWorkspaceToHtml, сохраняет обратно.
 *
 * Run: npx tsx scripts/regen_xray_sample.ts
 */

import { readFileSync, writeFileSync } from "node:fs";
import { exportWorkspaceToHtml } from "../src/lib/litgraph/export-html";
import type { LitNode, LitEdge, BackgroundLayer } from "../src/lib/litgraph/types";

const SAMPLE_PATH = "docs/xray-samples/1-сфера-предела.xray.html";

function main() {
  const html = readFileSync(SAMPLE_PATH, "utf-8");
  const m = html.match(
    /<script type="application\/json" id="litgraph-data">([\s\S]*?)<\/script>/,
  );
  if (!m) {
    console.error("ERROR: litgraph-data not found");
    process.exit(1);
  }
  const jsonStr = m[1].replace(/<\\\//g, "</");
  const data = JSON.parse(jsonStr);

  const nodes = data.nodes as LitNode[];
  const edges = data.edges as LitEdge[];
  const background = data.background as BackgroundLayer | null;
  const viewport = data.viewport as { x: number; y: number; zoom: number } | null;

  console.log(
    `Loaded ${nodes.length} nodes, ${edges.length} edges from existing sample`,
  );

  const newHtml = exportWorkspaceToHtml(nodes, edges, background, viewport, {
    title: data.project.title,
    author: data.project.author,
    description: data.project.description,
    parserVersion: data.project.parserVersion + " +heuristics-v1",
    sourceMdHash: data.project.sourceMdHash,
    createdAt: data.project.createdAt,
    analysisSnapshot: data.analysis,
  });

  writeFileSync(SAMPLE_PATH, newHtml, "utf-8");
  console.log(`Regenerated: ${SAMPLE_PATH}`);
  console.log(`New size: ${newHtml.length} chars`);

  // Verify diagnostics are embedded
  const m2 = newHtml.match(
    /<script type="application\/json" id="litgraph-data">([\s\S]*?)<\/script>/,
  )!;
  const newData = JSON.parse(m2[1].replace(/<\\\//g, "</"));
  console.log(
    `\nDiagnostics embedded: ${Object.keys(newData.diagnostics || {}).length} entries`,
  );
  console.log("Summary:", newData.diagnosticsSummary);
}

main();
