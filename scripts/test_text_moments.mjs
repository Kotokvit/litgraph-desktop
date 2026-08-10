// Smoke test for src/lib/poler/textMoments.ts
// Run: node --experimental-strip-types scripts/test_text_moments.mjs
// or:  bun run scripts/test_text_moments.mjs (after transpiling)
//
// Quick sanity check: build a tiny synthetic "novel", run findTextMoments
// on a character node, verify it finds both occurrences and groups them
// by chapter correctly.

import { findTextMoments, detectChapters, extractKeywords, highlightKeywords } from "../src/lib/poler/textMoments.ts";

const sample = `# Тестовый роман

Глава 1. Прибытие

Марта пришла на вокзал. Она огляделась и увидела Красса,
который стоял у колонны. Красс не заметил её.

Глава 2. Разговор

— Красс, ты здесь? — спросила Марта.
— Да, Марта, я здесь, — ответил Красс.

Глава 3. Прощание

Марта ушла. Красс остался стоять у колонны.
`;

const node = {
  data: {
    title: "Красс",
    meta: {
      forms: ["Красс", "Красса", "Крассу"],
    },
  },
};

console.log("=== TEST 1: extractKeywords ===");
const kw = extractKeywords(node);
console.log("Keywords:", kw);
if (kw.length < 3) throw new Error("Expected at least 3 keywords (Красс + Красса + Крассу)");
console.log("PASS");

console.log("\n=== TEST 2: detectChapters ===");
const chapters = detectChapters(sample);
console.log("Chapters:");
for (const c of chapters) console.log(`  ${c.title} pos=${c.pos} end=${c.end}`);
if (chapters.length < 3) throw new Error(`Expected at least 3 chapters, got ${chapters.length}`);
const ch1 = chapters.find(c => c.title === "Глава 1");
if (!ch1) throw new Error("Глава 1 not found");
console.log("PASS");

console.log("\n=== TEST 3: findTextMoments ===");
const result = findTextMoments(sample, node, { contextChars: 80 });
console.log(`Total moments: ${result.stats.totalMoments}`);
console.log(`Total chapters with matches: ${result.stats.totalChapters}`);
console.log(`Avg density: ${result.stats.avgDensity.toFixed(2)}`);
console.log(`Max density: ${result.stats.maxDensity.toFixed(2)}`);
console.log("\nBy chapter:");
for (const group of result.byChapter) {
  console.log(`  ${group.chapter.title} (${group.moments.length} matches)`);
  for (const m of group.moments) {
    console.log(`    pos=${m.position} kw="${m.matchedKeyword}" density=${m.density.toFixed(2)}`);
    console.log(`    text: "${m.text.substring(0, 80)}..."`);
  }
}
if (result.stats.totalMoments < 3) throw new Error(`Expected at least 3 moments, got ${result.stats.totalMoments}`);
console.log("PASS");

console.log("\n=== TEST 4: highlightKeywords ===");
const sample_text = "Красс стоял у колонны и не видел Красса.";
const segs = highlightKeywords(sample_text, ["Красс", "Красса"]);
console.log("Segments:");
for (const s of segs) {
  console.log(`  ${s.isMatch ? "[MATCH]" : "[text]"} "${s.text}"`);
}
const matches = segs.filter(s => s.isMatch);
if (matches.length < 2) throw new Error(`Expected at least 2 matches, got ${matches.length}`);
console.log("PASS");

console.log("\n=== ALL TESTS PASSED ===");
