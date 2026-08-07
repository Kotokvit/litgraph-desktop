// Epsilon-алгоритм важности фрагмента текста
// Основан на алгоритме POLER v6 (poler_v6.py)
// ε = (κ × kw_intensity × d_sq + emotion) / len_norm
//
// Физический смысл:
// - d_sq: сумма квадратов редкости слов (TF-IDF-like)
// - kw_intensity: логарифм частоты ключевого слова
// - emotion: эмоциональные маркеры × 1.5 (коэффициент преломления)
// - len_norm: √(unique_words) — закон излучения, не штрафует за длину

// Эмоциональные маркеры (60 мультимовних)
// Вес каждого = 1.5 (эмпирически подобранный коэффициент преломления)
const EMOTIONAL_MARKERS = new Set<string>([
  // УКР
  "хаос","сила","свідомість","реальність","істина","тінь","світло","темрява",
  "безодня","вічність","тиша","пам'ять","страх","надія","любов","зрада",
  "прощення","самотність","доля","свобода","вибір","правда","війна","смерть",
  "життя","кров","вогонь"," pain","біль","гнів","час","мить",
  // РУС
  "хаос","сила","сознание","реальность","истина","тень","свет","тьма",
  "бездна","вечность","тишина","память","страх","надежда","любовь","предательство",
  // EN
  "chaos","power","consciousness","reality","truth","shadow","light","darkness",
  "abyss","eternity","silence","memory","fear","hope","love","betrayal",
  "forgiveness","loneliness","fate","freedom","choice","war","death","life",
  "blood","fire","pain","anger","time","moment",
]);

// Стоп-слово (не считаются редкими)
const STOP_WORDS_EPSILON = new Set<string>([
  // УКР
  "і","та","й","в","у","на","з","до","за","від","по","при","про","для","із","від",
  "це","той","ця","те","він","вона","воно","вони","його","її","їх",
  "я","ти","ми","ви","мене","тебе","себе","мені","тобі","собі",
  "але","або","що","як","де","куди","коли","чому","тому","тож",
  "був","була","було","були","є","бути","ніхто","нічого","все","всі",
  "сьогодні","вчора","завтра","тепер","тоді","потім","раптом",
  "швидко","знову","ще","вже","тільки","навіть","можливо",
  "так","ні","авжеж","звичайно","добре",
  // РУС
  "и","в","на","с","к","за","от","по","при","про","для","из","не","ни",
  "это","тот","эта","эти","он","она","оно","они","его","её","их",
  "я","ты","мы","вы","меня","тебя","себя","мне","тебе","себе",
  "но","или","что","как","где","куда","когда","почему","поэтому",
  "был","была","было","были","есть","быть",
  "сегодня","вчера","завтра","теперь","тогда","потом","внезапно",
  "быстро","снова","ещё","уже","только","даже","возможно",
  "да","нет","конечно","хорошо",
  // EN
  "the","a","an","and","or","but","in","on","at","to","for","of","with",
  "this","that","these","those","he","she","it","they","his","her","its",
  "is","was","were","been","have","has","had","not","no",
  "i","you","we","me","my","your","our",
]);

export interface EpsilonResult {
  epsilon: number;        // сырой epsilon (до нормализации)
  normalized: number;     // 0-100 после глобальной нормализации
  wordCount: number;
  uniqueWords: number;
  emotionCount: number;
  kwCount: number;
  kwIntensity: number;
  dSq: number;
  lenNorm: number;
}

/**
 * Редкость слова: -log(p)
 * p = count(w) / total_words
 * total_words — объём ВСЕГО текста (инвариантная рідкість)
 */
function wordRarity(word: string, totalWords: number, counts: Map<string, number>): number {
  const count = counts.get(word) || 1;
  const p = count / Math.max(totalWords, 1);
  return -Math.log(Math.max(p, 1e-10));
}

/**
 * Токенизация: lowercase, фильтр стоп-слов, длина > 2
 * ВАЖНО: \w не ловит кириллицу! Используем Unicode- aware regex
 */
function tokenize(text: string): string[] {
  // \p{L} = любая буква Unicode (включая кириллицу)
  // \p{N} = любая цифра Unicode
  const raw = text.toLowerCase().match(/[\p{L}\p{N}'']+/gu) || [];
  return raw.filter((t) => !STOP_WORDS_EPSILON.has(t) && t.length > 2);
}

/**
 * Подсчёт частот слов во всём тексте
 */
export function buildWordCounts(text: string): { counts: Map<string, number>; total: number } {
  const tokens = tokenize(text);
  const counts = new Map<string, number>();
  for (const t of tokens) {
    counts.set(t, (counts.get(t) || 0) + 1);
  }
  return { counts, total: tokens.length };
}

/**
 * Вычисление epsilon для одного фрагмента (главы)
 * 
 * @param text Текст главы
 * @param globalCounts Частоты слов во всём произведении
 * @param totalWords Общее количество слов во всём произведении
 * @param keyword Ключевое слово (опционально)
 * @param kappa Коэффициент масштаба энергии (по умолчанию 1.0)
 */
export function computeEpsilon(
  chapterText: string,
  globalCounts: Map<string, number>,
  totalWords: number,
  keyword?: string,
  kappa: number = 1.0,
): EpsilonResult {
  const tokens = tokenize(chapterText);
  const cleanedText = chapterText.toLowerCase();

  // Уникальные слова (множество)
  const unique = new Set(tokens);

  // d_sq: сумма квадратов редкости уникальных слов
  let dSq = 0;
  for (const w of unique) {
    dSq += Math.pow(wordRarity(w, totalWords, globalCounts), 2);
  }

  // len_norm: √(unique_words) — закон излучения
  const lenNorm = Math.sqrt(unique.size) || 1;

  // kw_intensity: 1 + log(1 + kw_count)
  // Логарифм предотвращает "энергетический взрыв"
  let kwCount = 0;
  let kwIntensity = 1.0;
  if (keyword) {
    const kwLower = keyword.toLowerCase();
    kwCount = (cleanedText.match(new RegExp(`\\b${escapeRegex(kwLower)}\\b`, "g")) || []).length;
    kwIntensity = 1.0 + Math.log(1 + kwCount);
  }

  // emotion: Σ 1.5 за каждый эмоциональный маркер
  let emotionCount = 0;
  for (const token of tokens) {
    if (EMOTIONAL_MARKERS.has(token)) {
      emotionCount++;
    }
  }
  const emotion = emotionCount * 1.5;

  // Каноническая формула: ε = (κ × kw_intensity × d_sq + emotion) / len_norm
  const epsilon = (kappa * kwIntensity * dSq + emotion) / lenNorm;

  return {
    epsilon,
    normalized: 0, // заполнится после глобальной нормализации
    wordCount: tokens.length,
    uniqueWords: unique.size,
    emotionCount,
    kwCount,
    kwIntensity,
    dSq,
    lenNorm,
  };
}

/**
 * Глобальная нормализация epsilon в шкалу 0-100
 * "Семантические хребты" — главы с максимальной концентрацией истины
 */
export function normalizeEpsilons(results: EpsilonResult[]): EpsilonResult[] {
  if (results.length === 0) return results;
  
  const maxEpsilon = Math.max(...results.map((r) => r.epsilon));
  if (maxEpsilon <= 0) {
    return results.map((r) => ({ ...r, normalized: 0 }));
  }

  return results.map((r) => ({
    ...r,
    normalized: (r.epsilon / maxEpsilon) * 100,
  }));
}

/**
 * Резонанс: R[t] = ρ·R[t-1] + α·epsilon·(1+E)
 * Направленный — моделирует стрелу времени и инерцию нарратива
 * 
 * @param epsilons Массив epsilon по главам (по порядку)
 * @param rhoDecay Коэффициент затухания (0.85 = время жизни фонона ~5-8 глав)
 * @param alpha Усиление (0.1)
 */
export function computeResonanceSeries(
  epsilons: number[],
  rhoDecay: number = 0.85,
  alpha: number = 0.1,
): number[] {
  const resonances: number[] = [];
  let R = 0;

  for (let i = 0; i < epsilons.length; i++) {
    const E = epsilons[i];
    // Ур.10: R(t) = ρ·R(t-1) + α·E·(1+E)
    R = rhoDecay * R + alpha * E * (1 + E);
    resonances.push(R);
  }

  return resonances;
}

/**
 * Кластеризация глав с адаптивным gap
 * Группирует главы с близким epsilon и малым "смысловым разрывом"
 * 
 * @param epsilons Массив epsilon по главам
 * @param positions Позиции глав (индексы)
 * @param threshold Порог разрыва (если разница epsilon > threshold → новый кластер)
 */
export function clusterChapters(
  epsilons: number[],
  threshold: number = 20,
): number[][] {
  if (epsilons.length === 0) return [];

  const clusters: number[][] = [[0]];
  
  for (let i = 1; i < epsilons.length; i++) {
    const diff = Math.abs(epsilons[i] - epsilons[i - 1]);
    // Адаптивный gap: если разница большая → новый кластер
    if (diff > threshold) {
      clusters.push([i]);
    } else {
      clusters[clusters.length - 1].push(i);
    }
  }

  return clusters;
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
