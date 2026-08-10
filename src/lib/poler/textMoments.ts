/**
 * Text Moments — поиск фрагментов текста для узла графа.
 *
 * Порт алгоритма fragment-clustering из POLER v6
 * (super-z-skills/skills/poler-toolkit/scripts/poler_v6.py),
 * адаптированный под задачу LitGraph:
 *   - На входе:LitNode (с title + meta.forms aliases) + исходный markdown
 *   - На выходе: фрагменты текста, где упоминается сущность узла,
 *     сгруппированные по главам
 *
 * Отличия от POLER v6:
 *   - Без вычисления ε (information density) — это не нужно для GUI
 *   - Без кросс-файловой кластеризации (мы работаем с одним файлом)
 *   - Добавлена группировка по главам (использует те же regex что и Rust parser)
 *
 * Ссылки на алгебру:
 *   docs/poler_math/POLER_SPEC.md — спецификация операторов A, J, H, Π_Λ
 *   src/lib/poler/textGraph.ts   — TypeScript-порт матриц (co-occurrence, Laplacian)
 *   Здесь используется только Π_Λ-подобная проекция: ищем «где узел проявлен
 *   в тексте», что соответствует проектированию векторного представления узла
 *   на текстовое пространство.
 */

// ============================================================================
// ТИПЫ
// ============================================================================

export interface ChapterBoundary {
  /** Числовой номер главы (без суффикса). 0 = пролог/вступление. */
  num: number;
  /** Суффикс суб-главы: "б" для "28б", "" для обычных. */
  suffix: string;
  /** Полный заголовок: "Глава 28в" / "Розділ 5" / "Chapter 12". */
  title: string;
  /** Byte offset начала главы в исходном тексте. */
  pos: number;
  /** Byte offset конца главы (начала следующей или length(text)). */
  end: number;
}

export interface TextMoment {
  /** Глава, в которой находится фрагмент. */
  chapter: ChapterBoundary;
  /** Позиция совпадения (byte offset в исходном тексте). */
  position: number;
  /** Начало окна фрагмента (position - contextBefore). */
  start: number;
  /** Конец окна фрагмента (position + matched.length + contextAfter). */
  end: number;
  /** Текст фрагмента (±contextChars вокруг совпадения). */
  text: string;
  /** Какое ключевое слово совпало. */
  matchedKeyword: string;
  /** Сколько уникальных ключевых слов узла встретилось в окне фрагмента. */
  keywordCount: number;
  /** Нормализованная информационная плотность 0..100 (приближение POLER ε). */
  density: number;
}

export interface TextMomentsResult {
  /** Все фрагменты, отсортированные по позиции. */
  moments: TextMoment[];
  /** Группировка по главам (для UI). */
  byChapter: { chapter: ChapterBoundary; moments: TextMoment[] }[];
  /** Сводная статистика. */
  stats: {
    totalMoments: number;
    totalChapters: number;
    avgDensity: number;
    maxDensity: number;
  };
}

// ============================================================================
// ДЕТЕКЦИЯ ГЛАВ (порт parser/chapters.rs)
// ============================================================================

const CHAPTER_PATTERNS: { name: string; re: RegExp }[] = [
  { name: "ru-Глава",   re: /^Глава\s+(\d+[а-я]?)/gim },
  { name: "uk-Розділ",  re: /^Розділ\s+(\d+[а-я]?)/gim },
  { name: "uk-Частина", re: /^Частина\s+(\d+[а-я]?)/gim },
  { name: "en-Chapter", re: /^Chapter\s+(\d+[a-z]?)/gim },
  { name: "en-Part",    re: /^Part\s+(\d+[a-z]?)/gim },
  { name: "ru-Часть",   re: /^Часть\s+(\d+[а-я]?)/gim },
  { name: "md-hash-num",      re: /^#\s+(\d+[а-я]?)[\s.]/gim },
  { name: "md-hashhash-num",  re: /^##\s+(\d+[а-я]?)[\s.]/gim },
  { name: "md-hash-hash-num", re: /^###\s+(\d+[а-я]?)[\s.]/gim },
];

/**
 * Детекция глав в тексте.
 * Возвращает массив ChapterBoundary, отсортированный по позиции.
 * Использует тот же набор regex что и Rust parser/chapters.rs:9 patterns.
 */
export function detectChapters(text: string): ChapterBoundary[] {
  let bestMatches: { pos: number; numStr: string }[] = [];
  let bestCount = 0;

  for (const { re } of CHAPTER_PATTERNS) {
    const matches: { pos: number; numStr: string }[] = [];
    let m: RegExpExecArray | null;
    // Reset lastIndex (на случай если regex использовался ранее)
    re.lastIndex = 0;
    while ((m = re.exec(text)) !== null) {
      const numStr = m[1];
      // Пост-фильтр: если после числа идёт цифра — это не глава ("280")
      const after = text.substring(m.index + m[0].length, m.index + m[0].length + 1);
      if (after && /\d/.test(after)) continue;
      matches.push({ pos: m.index, numStr });
    }
    if (matches.length > bestCount) {
      bestCount = matches.length;
      bestMatches = matches;
    }
  }

  if (bestMatches.length === 0) {
    // Нет глав — весь текст одна «глава 0»
    return [{
      num: 0,
      suffix: "",
      title: "Текст целиком",
      pos: 0,
      end: text.length,
    }];
  }

  // Сортируем по позиции
  bestMatches.sort((a, b) => a.pos - b.pos);

  // Дедупликация по полному numStr (включая букву)
  const seen = new Set<string>();
  const unique: { pos: number; numStr: string }[] = [];
  for (const m of bestMatches) {
    if (seen.has(m.numStr)) continue;
    seen.add(m.numStr);
    unique.push(m);
  }

  // Строим ChapterBoundary
  const chapters: ChapterBoundary[] = unique.map((m, i) => {
    const numStr = m.numStr;
    // Разделяем цифру и суффикс
    const match = numStr.match(/^(\d+)([а-я]?|[a-z]?)$/i);
    const num = match ? parseInt(match[1], 10) : 0;
    const suffix = match ? match[2] : "";
    const end = i + 1 < unique.length ? unique[i + 1].pos : text.length;
    return {
      num,
      suffix,
      title: `Глава ${numStr}`,
      pos: m.pos,
      end,
    };
  });

  // Если первая глава не с позиции 0 — добавляем «пролог» (глава 0)
  if (chapters.length > 0 && chapters[0].pos > 0) {
    chapters.unshift({
      num: 0,
      suffix: "",
      title: "Пролог",
      pos: 0,
      end: chapters[0].pos,
    });
  }

  return chapters;
}

/**
 * Найти главу, в которой находится данная позиция.
 * Бинарный поиск по chapter.pos.
 */
export function findChapterForPosition(
  chapters: ChapterBoundary[],
  position: number
): ChapterBoundary {
  let lo = 0;
  let hi = chapters.length - 1;
  let result = chapters[0];
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (chapters[mid].pos <= position) {
      result = chapters[mid];
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  return result;
}

// ============================================================================
// ПОИСК КЛЮЧЕВЫХ СЛОВ
// ============================================================================

/**
 * Извлечь ключевые слова из узла графа.
 *   - title (основное имя)
 *   - meta.forms (массив форм слова, из NER)
 *   - meta.aliases (если есть)
 *   - title без суффиксов типа «(копия)»
 */
export function extractKeywords(node: {
  data: {
    title: string;
    meta?: Record<string, unknown> | null;
  };
}): string[] {
  const keywords = new Set<string>();

  // Очищаем title от суффиксов
  const cleanTitle = node.data.title
    .replace(/\s*\(копия\)\s*$/i, "")
    .replace(/\s*\(.*?\)\s*$/g, "")
    .trim();
  if (cleanTitle.length >= 2) keywords.add(cleanTitle);

  // meta.forms — массив форм слова
  const meta = node.data.meta;
  if (meta) {
    const forms = meta.forms;
    if (Array.isArray(forms)) {
      for (const f of forms) {
        if (typeof f === "string" && f.length >= 2) keywords.add(f);
      }
    }
    const aliases = meta.aliases;
    if (Array.isArray(aliases)) {
      for (const a of aliases) {
        if (typeof a === "string" && a.length >= 2) keywords.add(a);
      }
    }
  }

  return Array.from(keywords);
}

/**
 * Найти все позиции всех ключевых слов в тексте (case-insensitive).
 * Возвращает массив { position, keyword, length }.
 *
 * Сортируется по позиции. Дубликаты (одно и то же ключевое слово дважды
 * в одном месте) не отфильтрованы — нужны для подсчёта keyword_count в окне.
 */
export function findKeywordPositions(
  text: string,
  keywords: string[]
): { position: number; keyword: string; length: number }[] {
  const results: { position: number; keyword: string; length: number }[] = [];

  // Сортируем по длине УБЫВАЮЩЕ — чтобы "Красса" нашлась раньше "Красс",
  // и не дала ложного match "Красс"+"а" внутри "Красса".
  // (Альтернатива: word-boundary regex с lookbehind — но JS не поддерживает
  // lookbehind в Safari < 16. Проще отсортировать.)
  const sortedKw = [...keywords].sort((a, b) => b.length - a.length);
  // Track which positions have already been claimed by a longer keyword
  const claimed = new Set<number>();

  for (const kw of sortedKw) {
    if (kw.length < 2) continue;
    const lowerKw = kw.toLowerCase();
    // Escape regex special chars
    const escaped = lowerKw.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    // Word-boundary aware search
    const re = new RegExp(`(${escaped})`, "gi");
    let m: RegExpExecArray | null;
    while ((m = re.exec(text)) !== null) {
      // Skip if this position is already claimed by a longer keyword
      if (claimed.has(m.index)) {
        if (m[0].length === 0) re.lastIndex++;
        continue;
      }
      // Проверяем word boundary: предыдущий и следующий символы должны быть
      // не-буквами (или границами текста). \w в JS не покрывает кириллицу,
      // поэтому добавляем явный диапазон а-яёіїєґ.
      const before = m.index > 0 ? text[m.index - 1] : "";
      const after = m.index + m[0].length < text.length
        ? text[m.index + m[0].length]
        : "";
      const isBoundaryBefore = !before || !/[\wа-яёіїєґ]/i.test(before);
      const isBoundaryAfter = !after || !/[\wа-яёіїєґ]/i.test(after);
      if (isBoundaryBefore && isBoundaryAfter) {
        results.push({
          position: m.index,
          keyword: kw,
          length: m[0].length,
        });
        // Claim all positions covered by this match
        for (let i = m.index; i < m.index + m[0].length; i++) {
          claimed.add(i);
        }
      }
      // Защита от zero-length match
      if (m[0].length === 0) re.lastIndex++;
    }
  }

  results.sort((a, b) => a.position - b.position);
  return results;
}

// ============================================================================
// КЛАСТЕРИЗАЦИЯ ФРАГМЕНТОВ (порт cluster_fragments из POLER v6)
// ============================================================================

/**
 * Дедупликация близких позиций (порт deduplicate_positions из POLER v6).
 * Если две позиции ближе чем minDistance — оставляем только первую.
 */
export function deduplicatePositions(
  positions: number[],
  minDistance = 80
): number[] {
  if (positions.length === 0) return [];
  const sorted = [...positions].sort((a, b) => a - b);
  const merged = [sorted[0]];
  for (let i = 1; i < sorted.length; i++) {
    if (sorted[i] - merged[merged.length - 1] >= minDistance) {
      merged.push(sorted[i]);
    }
  }
  return merged;
}

/**
 * Извлечь фрагмент текста ±contextChars вокруг позиции.
 * Подгоняет под char boundaries (не разрезает UTF-8 суррогатные пары).
 */
export function extractFragment(
  text: string,
  position: number,
  matchLength: number,
  contextChars: number
): { start: number; end: number; text: string } {
  let start = Math.max(0, position - contextChars);
  let end = Math.min(text.length, position + matchLength + contextChars);

  // Подгонка под char boundary (Array.from для суррогатных пар)
  // В JS строки — UTF-16, поэтому проверяем кодовые единицы
  while (start > 0 && (text.charCodeAt(start) & 0xfc00) === 0xdc00) start--;
  while (end < text.length && (text.charCodeAt(end) & 0xfc00) === 0xdc00) end++;

  // Расширяем до границы слова (чтобы не разрезать слово)
  while (start > 0 && /\S/.test(text[start - 1]) && !/[\s.!?\n]/.test(text[start - 1])) {
    start--;
  }
  while (end < text.length && /\S/.test(text[end]) && !/[\s.!?\n]/.test(text[end])) {
    end++;
  }

  return {
    start,
    end,
    text: text.substring(start, end).replace(/\s+/g, " ").trim(),
  };
}

/**
 * Вычислить приближенную плотность информационной насыщенности фрагмента.
 * Упрощённая версия POLER ε: считает количество уникальных ключевых слов
 * в окне, нормализованное по длине фрагмента.
 *
 * Полная POLER v6 формула:
 *   ε = (κ · kw_intensity · Σ word_rarity² + emotion) / sqrt(|unique|)
 *
 * Здесь используем упрощение:
 *   density = (keywordHits / fragmentWordCount) * 100  → 0..100
 */
export function computeDensity(
  fragmentText: string,
  keywords: string[]
): { density: number; keywordCount: number } {
  if (fragmentText.length === 0 || keywords.length === 0) {
    return { density: 0, keywordCount: 0 };
  }
  const lower = fragmentText.toLowerCase();
  let totalHits = 0;
  let uniqueHits = 0;
  for (const kw of keywords) {
    const lowerKw = kw.toLowerCase();
    const escaped = lowerKw.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    // JS regex \b НЕ работает с кириллицей (только ASCII word boundaries).
    // Используем lookbehind/lookahead для проверки границ символов.
    // Разрешаем границы: начало/конец строки или не-буквенный символ
    // (включая кириллицу как буквенный).
    const re = new RegExp(
      `(?:^|[^\\wа-яёіїєґ])(${escaped})(?=$|[^\\wа-яёіїєґ])`,
      "gi"
    );
    const matches = lower.match(re);
    if (matches && matches.length > 0) {
      totalHits += matches.length;
      uniqueHits++;
    }
  }
  const wordCount = fragmentText.split(/\s+/).filter(Boolean).length;
  const density = wordCount > 0
    ? Math.min(100, (totalHits / wordCount) * 100 * 5) // x5 boost для визуальной разницы
    : 0;
  return { density, keywordCount: uniqueHits };
}

// ============================================================================
// MAIN: findTextMoments
// ============================================================================

export interface FindTextMomentsOptions {
  /** Контекст вокруг совпадения (символов). По умолчанию 200. */
  contextChars?: number;
  /** Минимальное расстояние между позициями для дедупликации. По умолчанию 80. */
  minDistance?: number;
  /** Максимум фрагментов (защита от перегрузки UI). По умолчанию 200. */
  maxMoments?: number;
}

/**
 * Главная функция: найти все «моменты» в тексте, где упоминается сущность узла.
 *
 * Алгоритм:
 *   1. detectChapters(text) → границы глав
 *   2. extractKeywords(node) → список имён/алиасов
 *   3. findKeywordPositions(text, keywords) → все позиции
 *   4. deduplicatePositions(positions, minDistance) → уникальные
 *   5. Для каждой позиции: extractFragment + computeDensity + findChapterForPosition
 *   6. Группировка по главам
 *
 * @returns TextMomentsResult с массивом moments и группировкой byChapter
 */
export function findTextMoments(
  text: string,
  node: { data: { title: string; meta?: Record<string, unknown> | null } },
  options: FindTextMomentsOptions = {}
): TextMomentsResult {
  const {
    contextChars = 200,
    minDistance = 80,
    maxMoments = 200,
  } = options;

  // Шаг 1: детекция глав
  const chapters = detectChapters(text);

  // Шаг 2: ключевые слова
  const keywords = extractKeywords(node);
  if (keywords.length === 0 || text.length === 0) {
    return {
      moments: [],
      byChapter: [],
      stats: { totalMoments: 0, totalChapters: 0, avgDensity: 0, maxDensity: 0 },
    };
  }

  // Шаг 3: поиск позиций
  const allPositions = findKeywordPositions(text, keywords);
  if (allPositions.length === 0) {
    return {
      moments: [],
      byChapter: [],
      stats: { totalMoments: 0, totalChapters: 0, avgDensity: 0, maxDensity: 0 },
    };
  }

  // Шаг 4: дедупликация (порт deduplicate_positions из POLER v6).
  // v0.5.1 fix: дедупликация должна происходить ВНУТРИ главы, а не между
  // главами. Раньше 2 совпадения в Главе 2 (pos=200) и Главе 3 (pos=270)
  // сливались в одно (расстояние 70 < minDistance=80), что давало
  // некорректный результат: Глава 3 вообще пропадала из выдачи.
  // Решение: сортируем по (chapter.pos, position), и проверяем расстояние
  // только если chapter не сменился.
  const positionsWithChapter = allPositions.map((p) => ({
    ...p,
    chapterPos: findChapterForPosition(chapters, p.position).pos,
  }));
  positionsWithChapter.sort(
    (a, b) =>
      a.chapterPos - b.chapterPos || a.position - b.position
  );
  const deduped: typeof positionsWithChapter = [];
  for (const p of positionsWithChapter) {
    if (deduped.length === 0) {
      deduped.push(p);
      continue;
    }
    const last = deduped[deduped.length - 1];
    const sameChapter = last.chapterPos === p.chapterPos;
    if (sameChapter && p.position - last.position < minDistance) {
      // Skip — слишком близко в той же главе
      continue;
    }
    deduped.push(p);
  }
  const limited = deduped.slice(0, maxMoments);

  // Шаг 5: построение фрагментов
  const moments: TextMoment[] = limited.map((p) => {
    const chapter = findChapterForPosition(chapters, p.position);
    const frag = extractFragment(text, p.position, p.length, contextChars);
    const { density, keywordCount } = computeDensity(frag.text, keywords);
    return {
      chapter,
      position: p.position,
      start: frag.start,
      end: frag.end,
      text: frag.text,
      matchedKeyword: p.keyword,
      keywordCount,
      density,
    };
  });

  // Шаг 6: группировка по главам
  const byChapterMap = new Map<string, TextMoment[]>();
  for (const m of moments) {
    const key = `${m.chapter.num}-${m.chapter.suffix}`;
    if (!byChapterMap.has(key)) byChapterMap.set(key, []);
    byChapterMap.get(key)!.push(m);
  }
  const byChapter = Array.from(byChapterMap.entries())
    .map(([, ms]) => ({
      chapter: ms[0].chapter,
      moments: ms.sort((a, b) => a.position - b.position),
    }))
    .sort((a, b) => {
      // Сортировка: по num, потом по suffix
      if (a.chapter.num !== b.chapter.num) return a.chapter.num - b.chapter.num;
      return a.chapter.suffix.localeCompare(b.chapter.suffix);
    });

  const densities = moments.map((m) => m.density);
  const avgDensity = densities.length > 0
    ? densities.reduce((s, v) => s + v, 0) / densities.length
    : 0;
  const maxDensity = densities.length > 0 ? Math.max(...densities) : 0;

  return {
    moments,
    byChapter,
    stats: {
      totalMoments: moments.length,
      totalChapters: byChapter.length,
      avgDensity,
      maxDensity,
    },
  };
}

// ============================================================================
// УТИЛИТЫ ДЛЯ UI
// ============================================================================

/**
 * Подсветить ключевые слова в тексте фрагмента.
 * Возвращает массив сегментов { text, isMatch }.
 */
export function highlightKeywords(
  fragmentText: string,
  keywords: string[]
): { text: string; isMatch: boolean; keyword?: string }[] {
  if (keywords.length === 0 || fragmentText.length === 0) {
    return [{ text: fragmentText, isMatch: false }];
  }

  // Строим объединённый regex со всеми ключевыми словами.
  // Сортируем по длине УБЫВАЮЩЕ — чтобы "Красса" матчалась как "Красса",
  // а не разрывалась на "Красс" + "а". Альтернативы в regex пробуются
  // слева направо, поэтому длинные должны идти первыми.
  const escaped = keywords
    .filter((k) => k.length >= 2)
    .sort((a, b) => b.length - a.length)
    .map((k) => k.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  if (escaped.length === 0) {
    return [{ text: fragmentText, isMatch: false }];
  }
  const re = new RegExp(`(${escaped.join("|")})`, "gi");

  const segments: { text: string; isMatch: boolean; keyword?: string }[] = [];
  let lastIdx = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(fragmentText)) !== null) {
    if (m.index > lastIdx) {
      segments.push({
        text: fragmentText.substring(lastIdx, m.index),
        isMatch: false,
      });
    }
    segments.push({
      text: m[0],
      isMatch: true,
      keyword: m[0],
    });
    lastIdx = m.index + m[0].length;
    if (m[0].length === 0) re.lastIndex++;
  }
  if (lastIdx < fragmentText.length) {
    segments.push({
      text: fragmentText.substring(lastIdx),
      isMatch: false,
    });
  }
  return segments;
}
