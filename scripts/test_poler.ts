/**
 * Тест POLER-анализа — проверка что код работает в Node.js окружении.
 * Запуск: bun run scripts/test_poler.ts
 */
import { analyzeText } from "../src/lib/poler/analyze";
import { readFileSync } from "fs";

const text = readFileSync("/home/z/my-project/poler-prototype/data/sample_text.txt", "utf-8");

console.log("=== POLER Test (TS, client-side) ===");
console.log(`Text: ${text.length} chars, ${text.split(/\s+/).length} words\n`);

const start = Date.now();
const result = analyzeText(text, {
  gamma: 0.05,
  kModes: 4,
  windowSize: 5,
  minFreq: 2,
});
const elapsed = Date.now() - start;

console.log(`✓ Analysis completed in ${elapsed}ms`);
console.log(`Nodes: ${result.nNodes}`);
console.log(`Edges: ${result.nEdges}`);
console.log(`Silhouette: ${result.silhouette.toFixed(4)}`);
console.log(`Iterations: ${result.iterations} (converged: ${result.converged})`);
console.log(`Eigenvalues: [${result.eigenvalues.map((v) => v.toFixed(4)).join(", ")}]`);
console.log(`Energy: ${result.energyStart.toFixed(6)} → ${result.energyFinal.toFixed(6)}\n`);

console.log("Top 10 words by ||POLER-mode||:");
result.clusters.slice(0, 10).forEach((c, i) => {
  console.log(
    `  ${i + 1}. ${c.word.padEnd(12)} cluster=${c.cluster}  ||p||=${c.modeNorm.toFixed(4)}  deg=${c.degree.toFixed(1)}`
  );
});

console.log("\nBottom 5 (least significant):");
result.clusters.slice(-5).forEach((c, i) => {
  console.log(
    `  ${result.clusters.length - 5 + i + 1}. ${c.word.padEnd(12)} cluster=${c.cluster}  ||p||=${c.modeNorm.toFixed(4)}  deg=${c.degree.toFixed(1)}`
  );
});

// Сравнение с Python-прототипом
console.log("\n=== Сравнение с Python-прототипом ===");
const expected = {
  nNodes: 40,
  nEdges: 419,
  silhouette: 0.372,
  top3: ["знал", "вронский", "не"],
};
console.log(`nNodes:        ${result.nNodes === expected.nNodes ? "✓" : "✗"}  (got ${result.nNodes}, expected ${expected.nNodes})`);
console.log(`nEdges:        ${result.nEdges === expected.nEdges ? "✓" : "✗"}  (got ${result.nEdges}, expected ${expected.nEdges})`);
console.log(
  `silhouette:    ${Math.abs(result.silhouette - expected.silhouette) < 0.05 ? "✓" : "✗"}  (got ${result.silhouette.toFixed(4)}, expected ${expected.silhouette})`
);
const gotTop3 = result.clusters.slice(0, 3).map((c) => c.word);
console.log(`top-3 words:   ${JSON.stringify(gotTop3) === JSON.stringify(expected.top3) ? "✓" : "≈"}  (got ${JSON.stringify(gotTop3)}, expected ${JSON.stringify(expected.top3)})`);

console.log("\n=== Test PASSED ===");
