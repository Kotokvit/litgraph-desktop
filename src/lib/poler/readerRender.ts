/**
 * Reader HTML rendering — превращает исходный markdown в безопасный HTML
 * с подсветкой ключевых слов и текущего фрагмента-цели.
 *
 * Используется в ReaderDialog (полноэкранный читатель). Открывается из
 * TextMomentsDialog по клику на момент.
 *
 * Стратегия производительности:
 *   - Главы детектятся один раз (через detectChapters из textMoments.ts)
 *   - HTML каждой главы считается один раз и мемоизуется в React.useMemo
 *   - При смене currentIndex пересчитывается только та глава, которая
 *     содержит новую цель (остальные переиспользуются)
 *
 * Безопасность:
 *   - Все сегменты текста проходят через escapeHtml() перед вставкой в HTML
 *   - Ключевые слова тоже эскейпятся перед вставкой в regex
 *   - Никаких <script>, <iframe> и т.п. — только <h2>, <p>, <mark>, <br>
 */

import {
  detectChapters,
  findChapterForPosition,
  type ChapterBoundary,
} from "./textMoments";

// ============================================================================
// ЭСКЕЙП HTML
// ============================================================================

/**
 * Экранировать спецсимволы HTML.
 * Достаточно для вставки в textContent-подобные элементы (<p>, <mark>, <h2>).
 */
export function escapeHtml(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

// ============================================================================
// РЕНДЕРИНГ ГЛАВЫ
// ============================================================================

export interface ReaderRenderOptions {
  /** Ключевые слова узла (для подсветки всех упоминаний). */
  keywords: string[];
  /** Текущий фрагмент-цель (для сильной подсветки и id-якоря). */
  target: { position: number; end: number } | null;
  /** Превратить \n\n в абзацы <p> (по умолчанию true). */
  paragraphMode?: boolean;
}

export interface RenderedChapter {
  /** Глава (для заголовка и ToC). */
  chapter: ChapterBoundary;
  /** HTML-строка, безопасная для dangerouslySetInnerHTML. */
  html: string;
  /** Содержит ли эта глава текущую цель (для авто-скролла). */
  hasTarget: boolean;
}

/**
 * Найти все ключевые слова в тексте главы (case-insensitive, word-boundary aware).
 * Возвращает массив { start, end, keyword } — отсортированный по start.
 *
 * Использует тот же word-boundary алгоритм что и findKeywordPositions в textMoments.ts,
 * но без claimed-set (здесь нам нужны ВСЕ совпадения, а не уникальные позиции).
 */
function findAllKeywordMatches(
  text: string,
  keywords: string[]
): { start: number; end: number; keyword: string }[] {
  if (keywords.length === 0 || text.length === 0) return [];

  // Сортируем по длине УБЫВАЮЩЕ — чтобы "Красса" матчем охватывалась раньше,
  // чем "Красс" + "а"
  const sortedKw = [...keywords]
    .filter((k) => k.length >= 2)
    .sort((a, b) => b.length - a.length);

  // Для каждой позиции в тексте храним, какое ключевое слово её занимает
  // (longest-match-wins). Используем Map для экономии памяти.
  const claimed = new Map<number, { end: number; keyword: string }>();

  for (const kw of sortedKw) {
    const lowerKw = kw.toLowerCase();
    const escaped = lowerKw.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const re = new RegExp(`(${escaped})`, "gi");
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
      const start = m.index;
      const end = start + m[0].length;
      // Проверяем, не занята ли уже эта позиция более длинным keyword
      let conflict = false;
      for (let i = start; i < end; i++) {
        if (claimed.has(i)) {
          conflict = true;
          break;
        }
      }
      if (conflict) {
        if (m[0].length === 0) re.lastIndex++;
        continue;
      }
      // Word boundary check (кириллица + латиница)
      const before = start > 0 ? text[start - 1] : "";
      const after = end < text.length ? text[end] : "";
      const isBoundaryBefore = !before || !/[\wа-яёіїєґ]/i.test(before);
      const isBoundaryAfter = !after || !/[\wа-яёіїєґ]/i.test(after);
      if (isBoundaryBefore && isBoundaryAfter) {
        for (let i = start; i < end; i++) {
          claimed.set(i, { end, keyword: kw });
        }
      }
      if (m[0].length === 0) re.lastIndex++;
    }
  }

  // Уникализируем по start (каждое keyword оставляет ровно одну запись)
  const seen = new Set<number>();
  const matches: { start: number; end: number; keyword: string }[] = [];
  for (const [start, info] of claimed) {
    if (seen.has(start)) continue;
    seen.add(start);
    matches.push({ start, end: info.end, keyword: info.keyword });
  }
  matches.sort((a, b) => a.start - b.start);
  return matches;
}

/**
 * Отрендерить одну главу в HTML.
 *
 * Алгоритм:
 *   1. Найти все keyword matches в тексте главы
 *   2. Найти target fragment (если он в этой главе)
 *   3. Объединить matches и target в один список "интервалов"
 *   4. Пройтись по тексту, разбивая на сегменты:
 *      - plain text → escapeHtml + \n\n → </p><p>
 *      - keyword match → <mark class="reader-keyword">...</mark>
 *      - target fragment → <mark class="reader-target" id="reader-target-N">...</mark>
 *      (target имеет приоритет над keyword внутри своего интервала)
 */
export function renderChapter(
  chapter: ChapterBoundary,
  fullText: string,
  options: ReaderRenderOptions
): RenderedChapter {
  const { keywords, target, paragraphMode = true } = options;

  // Извлекаем текст главы
  const chapterText = fullText.substring(chapter.pos, chapter.end);

  // Заголовок главы (детектируем первую строку)
  const firstNewline = chapterText.indexOf("\n");
  const headerLine =
    firstNewline > 0
      ? chapterText.substring(0, firstNewline).trim()
      : chapterText.substring(0, Math.min(80, chapterText.length)).trim();
  const headerHtml = `<h2 class="reader-chapter" data-chapter-pos="${chapter.pos}">${escapeHtml(
    headerLine || chapter.title
  )}</h2>`;

  // Тело главы (без заголовка)
  const bodyStart = firstNewline > 0 ? firstNewline + 1 : 0;
  const body = chapterText.substring(bodyStart);

  // Находим target в координатах body
  const targetInChapter =
    target &&
    target.position >= chapter.pos &&
    target.end <= chapter.end;
  // Смещаем target в координаты body
  const targetBodyStart = targetInChapter
    ? Math.max(0, target.position - chapter.pos - bodyStart)
    : -1;
  const targetBodyEnd = targetInChapter
    ? Math.min(body.length, target.end - chapter.pos - bodyStart)
    : -1;

  // Находим keyword matches в координатах body
  const kwMatches = findAllKeywordMatches(body, keywords);

  // Объединяем интервалы с приоритетом target
  type Interval = {
    start: number;
    end: number;
    type: "keyword" | "target";
  };
  const intervals: Interval[] = [];
  if (targetInChapter) {
    intervals.push({
      start: targetBodyStart,
      end: targetBodyEnd,
      type: "target",
    });
  }
  for (const m of kwMatches) {
    // Если match полностью внутри target — пропускаем (target имеет приоритет)
    if (
      targetInChapter &&
      m.start >= targetBodyStart &&
      m.end <= targetBodyEnd
    ) {
      continue;
    }
    // Если match пересекается с target — обрезаем match с обеих сторон
    let s = m.start;
    let e = m.end;
    if (targetInChapter) {
      if (s < targetBodyStart && e > targetBodyStart && e <= targetBodyEnd) {
        e = targetBodyStart;
      } else if (
        s >= targetBodyStart &&
        s < targetBodyEnd &&
        e > targetBodyEnd
      ) {
        s = targetBodyEnd;
      } else if (s < targetBodyStart && e > targetBodyEnd) {
        // match охватывает target с обеих сторон — разбиваем на 2 интервала
        intervals.push({ start: s, end: targetBodyStart, type: "keyword" });
        s = targetBodyEnd;
      } else if (s >= targetBodyStart && e <= targetBodyEnd) {
        continue;
      }
    }
    if (e > s) {
      intervals.push({ start: s, end: e, type: "keyword" });
    }
  }
  intervals.sort((a, b) => a.start - b.start);

  // Строим HTML
  let html = headerHtml;
  if (paragraphMode) {
    html += "<p>";
  }

  let lastIdx = 0;
  for (const iv of intervals) {
    if (iv.start < lastIdx) continue; // защита от наложений
    const before = body.substring(lastIdx, iv.start);
    const escapedBefore = escapeHtml(before);
    html += paragraphMode
      ? escapedBefore.replace(/\n{2,}/g, "</p><p>").replace(/\n/g, "<br>")
      : escapedBefore.replace(/\n/g, "<br>");

    const segText = body.substring(iv.start, iv.end);
    const escapedSeg = escapeHtml(segText);
    if (iv.type === "target") {
      html += `<mark class="reader-target" id="reader-target-${iv.start}">${escapedSeg}</mark>`;
    } else {
      html += `<mark class="reader-keyword">${escapedSeg}</mark>`;
    }
    lastIdx = iv.end;
  }
  // Хвост
  const tail = body.substring(lastIdx);
  const escapedTail = escapeHtml(tail);
  html += paragraphMode
    ? escapedTail.replace(/\n{2,}/g, "</p><p>").replace(/\n/g, "<br>")
    : escapedTail.replace(/\n/g, "<br>");

  if (paragraphMode) {
    html += "</p>";
  }

  return {
    chapter,
    html,
    hasTarget: !!targetInChapter,
  };
}

// ============================================================================
// РЕНДЕР ВСЕХ ГЛАВ
// ============================================================================

/**
 * Отрендерить все главы сразу.
 *
 * Для больших романов (1MB+) это может занять ~50-100ms.
 * В React-компоненте оборачивать в useMemo с зависимостями
 * [sourceMarkdown, keywords, target].
 */
export function renderAllChapters(
  sourceMarkdown: string,
  options: ReaderRenderOptions
): RenderedChapter[] {
  const chapters = detectChapters(sourceMarkdown);
  return chapters.map((c) => renderChapter(c, sourceMarkdown, options));
}

/**
 * Найти индекс главы, содержащей данную позицию.
 * Используется для авто-скролла ToC к активной главе.
 */
export function findChapterIndexForPosition(
  chapters: ChapterBoundary[],
  position: number
): number {
  let lo = 0;
  let hi = chapters.length - 1;
  let result = 0;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (chapters[mid].pos <= position) {
      result = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return result;
}

// ============================================================================
// ЭКСПОРТ ПОВТОРНЫЙ (для удобства)
// ============================================================================

export { detectChapters, findChapterForPosition };
export type { ChapterBoundary };
