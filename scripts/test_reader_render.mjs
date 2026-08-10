// Smoke test for src/lib/poler/readerRender.ts
// Run: node --experimental-strip-types scripts/test_reader_render.mjs
//
// Verifies:
//   1. escapeHtml escapes < > & " '
//   2. renderChapter produces <h2>, <mark class="reader-keyword"> for keyword matches
//   3. renderChapter produces <mark class="reader-target" id="..."> for target fragment
//   4. Target wins over keyword inside its interval
//   5. XSS-safe: <script> in source text is escaped
//   6. Multi-chapter: renderAllChapters returns one RenderedChapter per detected chapter

import {
  escapeHtml,
  renderChapter,
  renderAllChapters,
  findChapterIndexForPosition,
} from "../src/lib/poler/readerRender.ts";
import { detectChapters } from "../src/lib/poler/textMoments.ts";

let pass = 0, fail = 0;
function check(name, cond) {
  if (cond) { pass++; console.log(`✓ ${name}`); }
  else { fail++; console.error(`✗ ${name}`); }
}

console.log("=== TEST 1: escapeHtml ===");
const escaped = escapeHtml(`<script>alert("x")</script> & 'yo'`);
check("escapes <", !escaped.includes("<script"));
check("escapes &", escaped.includes("&amp;"));
check("escapes '", escaped.includes("&#39;"));
check("escapes \"", escaped.includes("&quot;"));

console.log("\n=== TEST 2: renderChapter — keyword highlight ===");
const sample2 = `Глава 1. Прибытие

Марта пришла на вокзал. Она огляделась и увидела Красса,
который стоял у колонны. Красс не заметил её.`;
const chapters2 = detectChapters(sample2);
const ch2 = chapters2[0];
const rendered2 = renderChapter(ch2, sample2, {
  keywords: ["Красс", "Красса"],
  target: null,
});
check("has <h2>", rendered2.html.includes("<h2"));
check("has reader-keyword mark for 'Красс'", rendered2.html.includes('mark class="reader-keyword"'));
check("has no reader-target", !rendered2.html.includes('reader-target'));
check("hasTarget false", rendered2.hasTarget === false);

console.log("\n=== TEST 3: renderChapter — target highlight ===");
// Находим позицию 'Красса' в тексте
const targetPos = sample2.indexOf("Красса");
const targetEnd = targetPos + "Красса".length;
const rendered3 = renderChapter(ch2, sample2, {
  keywords: ["Красс", "Красса"],
  target: { position: targetPos, end: targetEnd },
});
check("has reader-target mark", rendered3.html.includes('mark class="reader-target"'));
check("hasTarget true", rendered3.hasTarget === true);
check("target has id", rendered3.html.includes('id="reader-target-'));
// "Красса" должна быть внутри target, не внутри keyword
const targetHtml = rendered3.html.match(/<mark class="reader-target"[^>]*>([^<]*)<\/mark>/);
check("target contains 'Красса'", targetHtml && targetHtml[1].includes("Красса"));

console.log("\n=== TEST 4: renderChapter — XSS safety ===");
const xssText = `Глава 1. Тест

Марта сказала <script>alert('xss')</script> и Красс ответил.
<img src=x onerror=alert(1)>`;
const chsX = detectChapters(xssText);
const renderedX = renderChapter(chsX[0], xssText, {
  keywords: ["Красс"],
  target: null,
});
check("no raw <script> in HTML", !renderedX.html.includes("<script>"));
check("script is escaped", renderedX.html.includes("&lt;script&gt;"));
check("no raw <img tag in HTML", !renderedX.html.includes("<img"));
check("img is escaped", renderedX.html.includes("&lt;img"));

console.log("\n=== TEST 5: renderAllChapters — multi-chapter ===");
const multi = `# Тест

Глава 1. Первая

Красс пришёл.

Глава 2. Вторая

Марта ушла. Красс остался.

Глава 3. Третья

Финал.`;
const allRendered = renderAllChapters(multi, {
  keywords: ["Красс"],
  target: null,
});
check("renders 3 chapters (+ prologue if any)", allRendered.length >= 3);
check("each has html", allRendered.every(r => r.html.length > 0));
check("each has chapter ref", allRendered.every(r => r.chapter !== undefined));

console.log("\n=== TEST 6: findChapterIndexForPosition ===");
const idx0 = findChapterIndexForPosition(chapters2, 0);
const idxMid = findChapterIndexForPosition(chapters2, targetPos);
check("idx0 is 0 (or prologue)", idx0 === 0);
check("idxMid is valid", idxMid >= 0 && idxMid < chapters2.length);

console.log("\n=== TEST 7: target wins over keyword ===");
// Создаём ситуацию, где target полностью содержит keyword match
const overlapText = `Глава 1. Тест

Красс пришёл домой.`;
const chsO = detectChapters(overlapText);
const krassPos = overlapText.indexOf("Красс");
// Target: позиция за 5 символов до Красса и на 5 после
const targetStart = Math.max(0, krassPos - 5);
const targetEnd2 = krassPos + "Красс".length + 5;
const renderedO = renderChapter(chsO[0], overlapText, {
  keywords: ["Красс"],
  target: { position: targetStart, end: targetEnd2 },
});
// "Красс" должна быть ВНУТРИ target, не в отдельном keyword mark
const targetMatch = renderedO.html.match(/<mark class="reader-target"[^>]*>([\s\S]*?)<\/mark>/);
check("target span exists", !!targetMatch);
check("Красс inside target", targetMatch && targetMatch[1].includes("Красс"));
// Не должно быть отдельного keyword mark внутри target
check("no separate keyword mark inside target",
  !renderedO.html.includes('mark class="reader-keyword">Красс</mark>'));

console.log(`\n========================================`);
console.log(`RESULT: ${pass} passed, ${fail} failed`);
if (fail > 0) {
  console.error("SMOKE TESTS FAILED");
  process.exit(1);
}
console.log("ALL SMOKE TESTS PASSED");
