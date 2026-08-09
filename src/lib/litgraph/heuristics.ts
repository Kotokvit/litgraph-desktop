/**
 * heuristics.ts
 * =============
 * Smart X-Ray: пост-процессинговый слой диагностики поверх сырого парсера.
 *
 * Принцип (Centaur Manifest, Phase B):
 *   Мы НЕ меняем основной парсер (Rust, characters.rs). Вместо этого
 *   прогоняем готовые ноды через набор эвристик, которые помечают
 *   подозрительные места и предлагают автору проверить/исправить.
 *
 *   «Не чёрный ящик, а интерактивный ассистент».
 *
 * Что делает модуль:
 *   analyzeWorkspace(nodes, edges) → NodeDiagnostic[] (по ноде на каждый узел)
 *   - SUSPECT_WORD:       title в списке полисемантических абстракций (Архив, Бездна…)
 *   - LOW_SPEECH_RATIO:   freq > 50, но speechCount/freq < 5% → концепт?
 *   - MINIMAL_SPEECH:     speechCount < 3 при freq > 20 → мало речевых сигналов
 *   - NO_SPEECH_VERBS:    speechCount == 0 → defensive (v0.3.0 уже не должен пускать)
 *   - CROSS_TYPE_COLLISION: location с тем же 4-char prefix что и character → merge hint
 *
 * Confidence score (0..1):
 *   1.0  → ok       (зелёный)
 *   0.5–0.99 → suspect (жёлтый)
 *   < 0.5 → error   (красный)
 *
 * Используется:
 *   - в export-html.ts: каждый узел получает диагностический блок,
 *     HTML X-ray подсвечивает подозрительные ноды цветом и показывает
 *     warnings в sidebar.
 *   - (future) в React GUI: Inspector будет показывать ту же диагностику live.
 *
 * Лицензия: MIT (часть LitGraph).
 */

import type { LitNode, LitEdge } from "./types";

// ============================================================================
// Типы
// ============================================================================

export type WarningLevel = "info" | "warn" | "error";
export type DiagnosticLevel = "ok" | "suspect" | "error";

export interface Warning {
  level: WarningLevel;
  /** Машино-читаемый код: SUSPECT_WORD, LOW_SPEECH_RATIO, ... */
  code: string;
  /** Человекочитаемое сообщение (русский). */
  message: string;
  /** Опциональные детали (цифры, контекст). */
  detail?: string;
}

export interface Suggestion {
  /** Машино-читаемый код: MERGE_WITH_CHARACTER, RECLASSIFY_AS_LOCATION, ... */
  code: string;
  message: string;
  /** Тип предлагаемого действия. */
  action: "merge" | "reclassify" | "inspect_context";
  /** Для merge — id целевой ноды. */
  targetNodeId?: string;
  /** Опциональные детали (цифры, контекст). */
  detail?: string;
}

export interface NodeDiagnostic {
  /** Id ноды, к которой относится диагностика. */
  nodeId: string;
  /** 0..1 — насколько мы уверены в классификации. */
  confidence: number;
  /** ok | suspect | error — buckets for color coding. */
  level: DiagnosticLevel;
  warnings: Warning[];
  suggestions: Suggestion[];
  /** Краткая сводка для tooltip'а (1 строка). */
  summary: string;
}

// ============================================================================
// Константы
// ============================================================================

/**
 * Полисемантические абстракции — существительные, которые часто
 * капитализируются в фэнтези/философском тексте, но могут быть
 * НЕ именами персонажей: локация, концепт, метафора, собирательное.
 *
 * ВАЖНО: это НЕ стоп-лист (мы не удаляем эти слова). Это список
 * подозрения — они остаются в графе, но получают warning и жёлтую
 * рамку в X-ray, призывая автора проверить контекст.
 *
 * Список расширяется по мере обнаружения ложных срабатываний.
 * Доменно-специфичные имена (Мнемар, Секвестр, Этерия) ЗДЕСЬ ОТСУТСТВУЮТ —
 * их фильтрует speech-verb signal в самом парсере.
 */
const SUSPECT_WORDS: ReadonlySet<string> = new Set([
  // Русские абстрактные существительные
  "Архив", "Бездна", "Порядок", "Голос", "Эхо", "Тишина", "Тень", "Свет",
  "Сфера", "Истина", "Вечность", "Память", "Хаос", "Разум", "Дух", "Душа",
  "Судьба", "Свобода", "Война", "Смерть", "Жизнь", "Любовь", "Надежда",
  "Страх", "Боль", "Гнев", "Время", "Миг", "Мгновение", "Пустота", "Тьма",
  // Собирательные (Council, Clan, ...)
  "Совет", "Клан", "Орден", "Синдикат", "Союз", "Гильдия",
  // Здания/локации-абстракции
  "Башня", "Крепость", "Цитадель", "Храм", "Дворец", "Город", "Замок",
  // Природные явления
  "Море", "Океан", "Гора", "Лес", "Река", "Огонь", "Ветер", "Буря",
  // Украинские абстракции
  "Безодня", "Тиша", "Тінь", "Світло", "Істина", "Вічність", "Пам'ять",
  "Доля", "Свобода", "Війна", "Смерть", "Життя", "Любов", "Надія",
  "Страх", "Біль", "Гнів", "Час", "Мить", "Порожнеча", "Темрява",
]);

/**
 * Минимальный порог speech_verb_hits для доверия character-классификации.
 * Ниже — подозрительно (концепты тоже могут 1 раз «сказать» в метафоре).
 */
const MIN_SPEECH_FOR_CONFIDENCE = 3;

/**
 * Если freq > 50, но speechCount/freq ниже этого порога — концепт.
 */
const LOW_SPEECH_RATIO_THRESHOLD = 0.05;

/**
 * Минимальная частота, при которой имеет смысл ругаться на мало речевых
 * сигналов. Редко упоминаемые персонажи (freq < 20) могут и не иметь речи.
 */
const FREQ_THRESHOLD_FOR_SPEECH_CHECK = 20;

// ============================================================================
// Реализация
// ============================================================================

/**
 * Анализирует все ноды рабочего стола и возвращает массив диагностики.
 *
 * @param nodes — все ноды графа
 * @param edges — все рёбра (нужны для cross-type collision detection)
 * @returns Map<nodeId, NodeDiagnostic>
 */
export function analyzeWorkspace(
  nodes: LitNode[],
  _edges: LitEdge[],
): Map<string, NodeDiagnostic> {
  const result = new Map<string, NodeDiagnostic>();
  void _edges; // edges reserved for future heuristics (e.g. degree-based signals)

  // Pre-compute character prefixes for collision detection
  const characterPrefixes = new Map<string, LitNode>(); // prefix(4, lower) → node
  for (const n of nodes) {
    if (n.type === "character") {
      const prefix = takePrefix(n.data.title, 4).toLowerCase();
      if (prefix.length >= 3) {
        // первый выигрывает, чтобы не плодить дубли
        if (!characterPrefixes.has(prefix)) {
          characterPrefixes.set(prefix, n);
        }
      }
    }
  }

  for (const node of nodes) {
    const diag = analyzeNode(node, nodes, characterPrefixes);
    result.set(node.id, diag);
  }

  return result;
}

/**
 * Анализ одной ноды. Pure function — безопасно вызывать параллельно.
 */
function analyzeNode(
  node: LitNode,
  _allNodes: LitNode[],
  characterPrefixes: Map<string, LitNode>,
): NodeDiagnostic {
  void _allNodes; // reserved for future cross-node heuristics
  const warnings: Warning[] = [];
  const suggestions: Suggestion[] = [];
  let confidence = 1.0;

  const meta = (node.data.meta ?? {}) as Record<string, unknown>;
  const title = node.data.title ?? "";

  switch (node.type) {
    case "character":
      analyzeCharacter(node, meta, title, warnings, suggestions);
      break;
    case "location":
      analyzeLocation(node, meta, title, characterPrefixes, warnings, suggestions);
      break;
    case "chapter":
      // Главы не диагностируем — у них нет проблемы типизации.
      return {
        nodeId: node.id,
        confidence: 1.0,
        level: "ok",
        warnings: [],
        suggestions: [],
        summary: "Глава: структурная единица, проверке не подлежит.",
      };
    default:
      // Другие типы (scene, plotpoint, ...) — без диагностики в v1.
      return {
        nodeId: node.id,
        confidence: 1.0,
        level: "ok",
        warnings: [],
        suggestions: [],
        summary: `Тип «${node.type}»: диагностика не настроена.`,
      };
  }

  // === Пересчёт confidence на основе warnings ===
  for (const w of warnings) {
    if (w.level === "error") confidence -= 0.5;
    else if (w.level === "warn") confidence -= 0.25;
    else confidence -= 0.05;
  }
  if (confidence < 0) confidence = 0;
  if (confidence > 1) confidence = 1;

  const level: DiagnosticLevel =
    confidence < 0.5 ? "error" : confidence < 0.85 ? "suspect" : "ok";

  const summary = buildSummary(node, confidence, warnings, suggestions);

  return {
    nodeId: node.id,
    confidence: Math.round(confidence * 100) / 100,
    level,
    warnings,
    suggestions,
    summary,
  };
}

// ---------------------------------------------------------------------------
// Character heuristics
// ---------------------------------------------------------------------------

function analyzeCharacter(
  _node: LitNode,
  meta: Record<string, unknown>,
  title: string,
  warnings: Warning[],
  suggestions: Suggestion[],
): void {
  void _node;
  const freq = num(meta.mentions ?? meta.freq ?? 0);
  const speech = num(meta.speechCount ?? meta.speech_verb_hits ?? 0);
  const direct = num(meta.directCount ?? meta.direct_address_hits ?? 0);

  // H1: SUSPECT_WORD — title в списке полисемантических абстракций
  if (SUSPECT_WORDS.has(title)) {
    warnings.push({
      level: "warn",
      code: "SUSPECT_WORD",
      message: `«${title}» — часто нарицательное/абстрактное существительное. Проверьте, действительно ли это персонаж.`,
      detail: `Слово встречается в SUSPECT_WORDS list (${SUSPECT_WORDS.size} слов). Это не приговор — но в фэнтези такие слова часто оказываются локацией, концептом или метафорой.`,
    });
  }

  // H2: NO_SPEECH_VERBS — defensive (v0.3.0 уже не должен пускать такие)
  if (speech === 0 && direct === 0) {
    warnings.push({
      level: "error",
      code: "NO_SPEECH_VERBS",
      message: `Ни одного глагола речи и ни одного прямого обращения. Это почти точно не персонаж.`,
      detail: `freq=${freq}, speech_verb_hits=0, direct_address_hits=0. Концепты, локации и абстракции не «говорят» в тексте — это главный критерий.`,
    });
    suggestions.push({
      code: "RECLASSIFY_AS_LOCATION_OR_CONCEPT",
      message: `Проверьте контекст. Если слово идёт после предлога места (в/на/у) — это локация. Если описывает идею — это concept/idea.`,
      action: "reclassify",
    });
  } else if (speech < MIN_SPEECH_FOR_CONFIDENCE && freq > FREQ_THRESHOLD_FOR_SPEECH_CHECK) {
    // H3: MINIMAL_SPEECH — мало речевых сигналов при высокой частоте
    warnings.push({
      level: "warn",
      code: "MINIMAL_SPEECH",
      message: `Мало глаголов речи (${speech}) при частоте ${freq}. Возможно, это концепт или собирательное.`,
      detail: `Персонажи обычно «говорят» пропорционально частоте упоминаний. Например, Рэй (freq=268, speech=73 → ratio 27%) — типичный персонаж.`,
    });
  }

  // H4: LOW_SPEECH_RATIO — ratio speech/freq слишком мал для частого слова
  if (freq > 50 && speech > 0) {
    const ratio = speech / freq;
    if (ratio < LOW_SPEECH_RATIO_THRESHOLD) {
      warnings.push({
        level: "warn",
        code: "LOW_SPEECH_RATIO",
        message: `Глаголы речи составляют только ${(ratio * 100).toFixed(1)}% от упоминаний. Это нетипично для активного персонажа.`,
        detail: `freq=${freq}, speech=${speech}, ratio=${ratio.toFixed(3)} < ${LOW_SPEECH_RATIO_THRESHOLD}. Сравните с типичным ratio 0.15–0.40 для главных героев.`,
      });
    }
  }

  // H5: VERY_HIGH_FREQ_NO_DIRECT — частый персонаж без прямых обращений
  // (прямой адрес — сильный сигнал, но парсер v0.3.0 его не находит из-за
  // слишком жёсткого паттерна «— Name,»). Это не ошибка классификации
  // персонажа, но показатель что pattern нужно ослабить.
  if (freq > 50 && direct === 0 && speech >= 3) {
    // info, не warn — персонаж скорее всего правильный, просто
    // direct-address pattern не сработал
    warnings.push({
      level: "info",
      code: "DIRECT_ADDRESS_PATTERN_MISS",
      message: `Частый персонаж без прямых обращений (direct_address_hits=0). Возможно, паттерн «— Name,» слишком строгий.`,
      detail: `Parser v0.3.0 ищет em-dash + Name + [,!?.]. В диалогах в кавычках («Name, ...») или без запятой после имени этот pattern не срабатывает. Не влияет на классификацию — просто статистика.`,
    });
  }
}

// ---------------------------------------------------------------------------
// Location heuristics
// ---------------------------------------------------------------------------

function analyzeLocation(
  _node: LitNode,
  meta: Record<string, unknown>,
  title: string,
  characterPrefixes: Map<string, LitNode>,
  warnings: Warning[],
  suggestions: Suggestion[],
): void {
  void _node;
  const freq = num(meta.mentions ?? 0);

  // H6: CROSS_TYPE_LEMMA_COLLISION — location с тем же 4-char prefix что и character
  // Пример: «Рэя» (location, freq=24) ↔ «Рэй» (character, freq=268).
  // Это почти наверняка одно и то же слово в разных падежах.
  const prefix = takePrefix(title, 4).toLowerCase();
  if (prefix.length >= 3 && characterPrefixes.has(prefix)) {
    const charNode = characterPrefixes.get(prefix)!;
    if (charNode.id !== _node.id) {
      suggestions.push({
        code: "MERGE_WITH_CHARACTER",
        message: `Локация «${title}» (freq=${freq}) похожа на персонажа «${charNode.data.title}» (freq=${charNode.data.meta?.mentions ?? "?"}). Возможно, это тот же персонаж в косвенном падеже.`,
        detail: `4-char prefix «${prefix}» совпадает. Parser v0.3.0 группирует по prefix внутри типа, но не между типами. Если это одна сущность — удалите локацию.`,
        action: "merge",
        targetNodeId: charNode.id,
      });
      // Это suggestion, не warning — мы не уверены.
    }
  }

  // H7: SUSPECT_LOCATION — location name в списке персонажих абстракций
  // (бывает что «Бездна» определилась как location — это норм, но check)
  if (SUSPECT_WORDS.has(title)) {
    warnings.push({
      level: "info",
      code: "SUSPECT_LOCATION_NAME",
      message: `«${title}» — полисемантическое слово. Если оно описывает абстрактное понятие, а не физическое место — reconsider как theme/idea.`,
      detail: `Слово найдено в SUSPECT_WORDS. Может быть как настоящей локацией (Бездна = название каньона), так и абстракцией (Бездна = метафора отчаяния).`,
    });
  }

  // H8: VERY_LOW_FREQ_LOCATION — частота < 5 подозрительна для настоящей локации
  if (freq > 0 && freq < 5) {
    warnings.push({
      level: "info",
      code: "LOW_FREQ_LOCATION",
      message: `Локация упомянута всего ${freq} раз. Возможно, это случайное совпадение предлог + Name.`,
      detail: `Parser v0.3.0 имеет порог count >= 3 для locations. На больших текстах единичные упоминания могут быть шумом.`,
    });
  }
}

// ============================================================================
// Helpers
// ============================================================================

function takePrefix(s: string, n: number): string {
  // Берём первые n символов (unicode-safe). Если слово короче — возвращаем как есть.
  const chars = Array.from(s);
  return chars.slice(0, n).join("");
}

function num(v: unknown): number {
  if (typeof v === "number") return v;
  if (typeof v === "string") {
    const n = parseInt(v, 10);
    return isNaN(n) ? 0 : n;
  }
  return 0;
}

function buildSummary(
  _node: LitNode,
  confidence: number,
  warnings: Warning[],
  suggestions: Suggestion[],
): string {
  void _node;
  if (warnings.length === 0 && suggestions.length === 0) {
    return `Уверенность ${Math.round(confidence * 100)}%: замечаний нет.`;
  }
  const errCount = warnings.filter((w) => w.level === "error").length;
  const warnCount = warnings.filter((w) => w.level === "warn").length;
  const infoCount = warnings.filter((w) => w.level === "info").length;
  const sugCount = suggestions.length;

  const parts: string[] = [];
  if (errCount) parts.push(`${errCount} ошибок`);
  if (warnCount) parts.push(`${warnCount} предупр.`);
  if (infoCount) parts.push(`${infoCount} инфо`);
  if (sugCount) parts.push(`${sugCount} предложений`);

  return `Уверенность ${Math.round(confidence * 100)}%: ${parts.join(", ")}.`;
}

// ============================================================================
// Константы для экспорта (нужны в HTML mini-program)
// ============================================================================

/**
 * Возвращает SUSPECT_WORDS list как массив (для embedding в HTML X-ray).
 * В HTML mini-program эта константа позволяет пользователю навести курсор
 * на подозрительное слово и увидеть, что оно в списке.
 */
export function getSuspectWordsList(): string[] {
  return Array.from(SUSPECT_WORDS).sort();
}

/**
 * Возвращает пороги для отображения в X-ray UI.
 */
export function getThresholds() {
  return {
    MIN_SPEECH_FOR_CONFIDENCE,
    LOW_SPEECH_RATIO_THRESHOLD,
    FREQ_THRESHOLD_FOR_SPEECH_CHECK,
  };
}
