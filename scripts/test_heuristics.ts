/**
 * test_heuristics.ts
 * Тестирует модуль heuristics.ts на реальном X-ray sample
 * (1-Сфера Предела). Прогоняет analyzeWorkspace на нодах из
 * docs/xray-samples/1-сфера-предела.xray.html и показывает
 * какие warnings/suggestions были сгенерированы.
 *
 * Run: npx tsx scripts/test_heuristics.ts
 */

import { readFileSync, writeFileSync } from "node:fs";
import { analyzeWorkspace } from "../src/lib/litgraph/heuristics";
import type { LitNode, LitEdge } from "../src/lib/litgraph/types";

const SAMPLE_PATH = "docs/xray-samples/1-сфера-предела.xray.html";

function main() {
  const html = readFileSync(SAMPLE_PATH, "utf-8");
  const m = html.match(
    /<script type="application\/json" id="litgraph-data">([\s\S]*?)<\/script>/,
  );
  if (!m) {
    console.error("ERROR: litgraph-data not found in", SAMPLE_PATH);
    process.exit(1);
  }
  // Undo the </ → <\/ safety replace
  const jsonStr = m[1].replace(/<\\\//g, "</");
  const data = JSON.parse(jsonStr);

  const nodes = data.nodes as LitNode[];
  const edges = data.edges as LitEdge[];

  console.log(`Loaded ${nodes.length} nodes, ${edges.length} edges from sample`);

  const diag = analyzeWorkspace(nodes, edges);

  console.log(`\nDiagnostics generated for ${diag.size} nodes\n`);

  // Сводка
  let okCount = 0, suspectCount = 0, errorCount = 0;
  let totalWarn = 0, totalSug = 0;
  for (const d of diag.values()) {
    if (d.level === "ok") okCount++;
    else if (d.level === "suspect") suspectCount++;
    else errorCount++;
    totalWarn += d.warnings.length;
    totalSug += d.suggestions.length;
  }
  console.log("=== SUMMARY ===");
  console.log(`  OK:       ${okCount}`);
  console.log(`  SUSPECT:  ${suspectCount}`);
  console.log(`  ERROR:    ${errorCount}`);
  console.log(`  Total warnings:    ${totalWarn}`);
  console.log(`  Total suggestions: ${totalSug}`);

  // Детальный вывод подозрительных нод
  console.log("\n=== SUSPECT / ERROR NODES ===\n");
  const nodeById = new Map(nodes.map((n) => [n.id, n]));
  const suspectDiags = Array.from(diag.values()).filter(
    (d) => d.level !== "ok",
  );
  suspectDiags.sort((a, b) => a.confidence - b.confidence);

  for (const d of suspectDiags) {
    const node = nodeById.get(d.nodeId);
    if (!node) continue;
    console.log(
      `  [${d.level.toUpperCase()} conf=${Math.round(d.confidence * 100)}%] ` +
        `${node.type}: "${node.data.title}"`,
    );
    console.log(`    ${d.summary}`);
    for (const w of d.warnings) {
      console.log(`    ⚠ ${w.code} [${w.level}]: ${w.message}`);
      if (w.detail) console.log(`       detail: ${w.detail}`);
    }
    for (const s of d.suggestions) {
      console.log(`    → ${s.code}: ${s.message}`);
      if (s.targetNodeId) {
        const tgt = nodeById.get(s.targetNodeId);
        console.log(`       target: ${tgt?.data.title ?? s.targetNodeId}`);
      }
    }
    console.log();
  }

  // Сохраним полный отчёт в JSON
  const report = {
    sample: SAMPLE_PATH,
    summary: { okCount, suspectCount, errorCount, totalWarn, totalSug },
    suspectNodes: suspectDiags.map((d) => {
      const node = nodeById.get(d.nodeId);
      return {
        nodeId: d.nodeId,
        nodeType: node?.type,
        nodeTitle: node?.data.title,
        confidence: d.confidence,
        level: d.level,
        summary: d.summary,
        warnings: d.warnings,
        suggestions: d.suggestions,
      };
    }),
  };
  writeFileSync(
    "scripts/heuristics_report.json",
    JSON.stringify(report, null, 2),
    "utf-8",
  );
  console.log("\nFull report saved to scripts/heuristics_report.json");
}

main();
