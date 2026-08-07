import { NextRequest, NextResponse } from "next/server";
import { z } from "zod";
import {
  buildWordCounts,
  computeEpsilon,
  normalizeEpsilons,
  computeEpsilonFragmented,
  normalizeFragmentedEpsilons,
} from "@/lib/litgraph/epsilon";

// ====== Типы ======
interface ParsedChapter {
  num: number;
  title: string;
  body: string;
  fullText: string;
  pos: number;
  end: number;
}

interface ParsedCharacter {
  name: string;
  aliases: string[];
  count: number;
  description: string;
}

interface ParsedLocation {
  name: string;
  aliases: string[];
  count: number;
  description: string;
}

interface ParsedTheme {
  name: string;
  count: number;
  description: string;
}

interface GraphNode {
  id: string;
  type: string;
  position: { x: number; y: number };
  data: {
    title: string;
    body: string;
    type: string;
    tags: string[];
    meta: Record<string, unknown>;
    fullText?: string;
  };
}

interface GraphEdge {
  id: string;
  source: string;
  target: string;
  sourceHandle: string | null;
  targetHandle: string | null;
  type: string;
  animated: boolean;
  data: { kind: string };
}

function uid(prefix: string): string {
  return `${prefix}_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

// ====== 1. Детекция глав ======
function detectChapters(text: string): { chapters: ParsedChapter[]; prologueText: string } {
  const patterns: { name: string; regex: RegExp }[] = [
    { name: "uk-Глава", regex: /Глава\s+(\d+)/g },
    { name: "uk-Розділ", regex: /Розділ\s+(\d+)/g },
    { name: "uk-Частина", regex: /Частина\s+(\d+)/g },
    { name: "en-Chapter", regex: /Chapter\s+(\d+)/g },
    { name: "en-Part", regex: /Part\s+(\d+)/g },
    { name: "ru-Часть", regex: /Часть\s+(\d+)/g },
    { name: "md-hash-num", regex: /^#\s+(\d+)[\s.]/gm },
    { name: "md-hashhash-num", regex: /^##\s+(\d+)[\s.]/gm },
    { name: "md-hash-hash-num", regex: /^###\s+(\d+)[\s.]/gm },
  ];

  let bestMatches: RegExpMatchArray[] = [];
  for (const { regex } of patterns) {
    const matches = [...text.matchAll(regex)];
    if (matches.length > bestMatches.length) bestMatches = matches as RegExpMatchArray[];
  }

  if (bestMatches.length === 0) {
    const bodyClean = text.split(/\s+/).join(" ");
    return {
      chapters: [{
        num: 1, title: "Текст целиком",
        body: bodyClean.slice(0, 400) + (bodyClean.length > 400 ? "…" : ""),
        fullText: text, pos: 0, end: text.length,
      }],
      prologueText: "",
    };
  }

  const seen = new Map<number, RegExpMatchArray>();
  for (const m of bestMatches) {
    const num = parseInt(m[1], 10);
    if (!seen.has(num)) seen.set(num, m);
  }
  const sorted = Array.from(seen.values()).sort((a, b) => (a.index ?? 0) - (b.index ?? 0));

  const prologueText = sorted[0].index ? text.slice(0, sorted[0].index) : "";
  const chapters: ParsedChapter[] = [];

  for (let i = 0; i < sorted.length; i++) {
    const m = sorted[i];
    const num = parseInt(m[1], 10);
    const pos = m.index ?? 0;
    const matchEnd = pos + m[0].length;
    const nextPos = i + 1 < sorted.length ? sorted[i + 1].index ?? text.length : text.length;
    const bodyText = text.slice(matchEnd, nextPos).trim();
    const bodyClean = bodyText.replace(/\s+/g, " ").trim();
    const bodyPreview = bodyClean.slice(0, 400) + (bodyClean.length > 400 ? "…" : "");

    // Извлечение заголовка
    const after = text.slice(matchEnd, matchEnd + 500);
    const title = extractTitleFromAfter(after);

    chapters.push({
      num, title, body: bodyPreview, fullText: bodyText,
      pos, end: nextPos,
    });
  }
  return { chapters, prologueText };
}

function extractTitleFromAfter(after: string): string {
  let cleaned = after.replace(/^[\s:\-—]+/, "");
  if (cleaned.startsWith("(")) {
    const end = cleaned.indexOf(")");
    if (end > 0) cleaned = cleaned.slice(end + 1).replace(/^[\s:]+/, "");
  }

  let candidate: string;
  const newlinePos = cleaned.indexOf("\n");
  if (newlinePos > 0 && newlinePos < 200) {
    candidate = cleaned.slice(0, newlinePos);
  } else {
    const dotMatch = cleaned.match(/[.!?…]\s+[А-ЯЮЯЩЬЦФВІЇҐЄA-Z]/);
    if (dotMatch && dotMatch.index !== undefined && dotMatch.index < 150) {
      candidate = cleaned.slice(0, dotMatch.index);
    } else {
      candidate = cleaned.slice(0, 80);
    }
  }

  candidate = candidate
    .replace(/\(Робоча назва\)\s*:?\s*/g, "")
    .replace(/\(Виправлена версія\)\s*/g, "")
    .replace(/\(Фінальна версія\)\s*/g, "")
    .replace(/\(ФІНАЛЬНИЙ ТЕКСТ\)\s*/g, "")
    .replace(/\(Відредагована версія\)\s*/g, "")
    .replace(/Глава\s*\d+\s*:?\s*/g, "")
    .replace(/\(Частина\s+[IVX]+\s*[—-]\s*[^)]+\)/g, "")
    .replace(/\(Арка\s+\d+\s*:\s*"[^"]+"\)/g, "")
    .replace(/\(Арка\s+"[^"]+"\)/g, "")
    .replace(/\(Континент\s+[^)]+\)/g, "")
    .replace(/\(Локація:\s*[^)]+\)/g, "")
    .replace(/Місце дії:\s*[^.]+\.?\s*/g, "")
    .replace(/\(Початок\)/g, "")
    .replace(/\s+/g, " ")
    .trim();

  if (candidate.length > 70) {
    const cut = candidate.slice(0, 70);
    const lastSep = Math.max(cut.lastIndexOf(","), cut.lastIndexOf(" — "), cut.lastIndexOf(" - "));
    if (lastSep > 30) candidate = cut.slice(0, lastSep);
    else candidate = cut;
  }
  return candidate || "Глава";
}

// ====== 2. Персонажи ======
const STOP_WORDS = new Set<string>([
  "Цей","Ця","Це","Той","Та","Те","Він","Вона","Воно","Вони","Його","Її","Їх","Мій","Твій","Наш","Ваш","Свій","Своя","Своє",
  "Бо","Що","Як","Де","Куди","Звідки","Коли","Чому","Чи","Тож","Тут","Там","Так","Ні","Якщо","Але","Однак","Отже","Проте","Також",
  "Був","Була","Було","Були","Є","Бути","Крім","Замість","Після","Перед","Між","Біля","Над","Під","За","На","Кожен","Кожна","Кожне","Усі","Всі",
  "Сьогодні","Вчора","Завтра","Тепер","Тоді","Потім","Раптом","Незабаром","Швидко","Повільно","Знову","Ще","Вже","Тільки","Навіть","Можливо",
  "Дякую","Вибачте","Пробачте","Будь","Ласка","Скажи","Подивися","Послухай","Боже","Господи","Так","Ні","Авжеж","Звичайно","Добре","Погано",
  "Світло","Темрява","Тиша","Вогонь","Вода","Повітря","Земля","Небо",
  "Этот","Эта","Эти","Тот","Он","Она","Оно","Они","Его","Её","Их","Мой","Твой","Наш","Ваш","Свой",
  "Потому","Что","Как","Где","Куда","Откуда","Когда","Почему","Ли","Итак","Здесь","Там","Так","Нет","Если","Но","Однако","Также",
  "Был","Была","Было","Были","Есть","Быть","Кроме","Вместо","После","Перед","Между","Около","Над","Под","За","На",
  "Каждый","Каждая","Все","Всё","Сегодня","Вчера","Завтра","Теперь","Тогда","Потом","Внезапно","Скоро","Быстро","Медленно","Снова","Ещё","Уже","Только","Даже","Возможно",
  "Спасибо","Извините","Прости","Пожалуйста","Скажи","Посмотри","Послушай","Боже","Господи","Да","Нет","Конечно","Хорошо","Плохо",
  "Свет","Тьма","Тишина","Огонь","Вода","Воздух","Земля","Небо",
  "The","This","That","These","Those","He","She","It","They","We","You","His","Her","Its","Their","My","Your","Our",
  "But","And","Or","Not","Yes","No","Oh","Ah","When","Where","What","Who","Why","How","Which",
  "Here","There","Now","Then","Today","Yesterday","Tomorrow","Because","If","Although","However","So","Therefore","Also","Too",
  "Was","Were","Been","Have","Has","Had","Some","Any","All","Every","Each","Both","One","Two","Three","First","Second","Third",
  "Good","Bad","Please","Thanks","Thank","Mr","Mrs","Dr","Ms",
]);

function detectCharacters(text: string): ParsedCharacter[] {
  const wordRegex = /(?<![a-zA-Z\u0400-\u04FF])([А-ЯЁA-Z][а-яёa-z\u0400-\u04FF]{2,})(?![a-zA-Z\u0400-\u04FF])/g;
  const wordCounts: Record<string, number> = {};
  let match: RegExpExecArray | null;

  while ((match = wordRegex.exec(text)) !== null) {
    const word = match[1];
    const start = match.index;
    if (start === 0) continue;
    const precedingStart = Math.max(0, start - 3);
    const preceding = text.slice(precedingStart, start);
    if (/[.!?…]["'»]?\s*$/.test(preceding)) continue;
    if (STOP_WORDS.has(word)) continue;
    wordCounts[word] = (wordCounts[word] || 0) + 1;
  }

  const groups: Record<string, { rep: string; count: number; forms: Set<string> }> = {};
  for (const [word, count] of Object.entries(wordCounts)) {
    if (count < 5) continue;
    const key = word.slice(0, 4).toLowerCase();
    if (!groups[key]) groups[key] = { rep: word, count: 0, forms: new Set() };
    groups[key].count += count;
    groups[key].forms.add(word);
    if (word.length < groups[key].rep.length) groups[key].rep = word;
  }

  return Object.values(groups)
    .filter((g) => g.count >= 5)
    .sort((a, b) => b.count - a.count)
    .slice(0, 25)
    .map((g) => ({
      name: g.rep,
      aliases: Array.from(g.forms),
      count: g.count,
      description: `Персонаж, упомянутый ${g.count} раз. Формы: ${Array.from(g.forms).slice(0, 6).join(", ")}.`,
    }));
}

// ====== 3. Локации ======
function detectLocations(text: string): ParsedLocation[] {
  const locRegex = /(?<![a-zA-Z\u0400-\u04FF])(?:у|в|на|біля|під|над|за|до|із|від|через|крізь|около|под|возле|перед|in|at|on|near|under|over|behind|from|through)\s+([А-ЯЁA-Z][а-яёa-z\u0400-\u04FF]{2,})(?![a-zA-Z\u0400-\u04FF])/g;
  const locCounts: Record<string, number> = {};
  let match: RegExpExecArray | null;

  while ((match = locRegex.exec(text)) !== null) {
    const word = match[1];
    if (STOP_WORDS.has(word)) continue;
    locCounts[word] = (locCounts[word] || 0) + 1;
  }

  const groups: Record<string, { rep: string; count: number; forms: Set<string> }> = {};
  for (const [word, count] of Object.entries(locCounts)) {
    if (count < 3) continue;
    const key = word.slice(0, 4).toLowerCase();
    if (!groups[key]) groups[key] = { rep: word, count: 0, forms: new Set() };
    groups[key].count += count;
    groups[key].forms.add(word);
    if (word.length < groups[key].rep.length) groups[key].rep = word;
  }

  return Object.values(groups)
    .filter((g) => g.count >= 3)
    .sort((a, b) => b.count - a.count)
    .slice(0, 15)
    .map((g) => ({
      name: g.rep,
      aliases: Array.from(g.forms),
      count: g.count,
      description: `Локация, упомянутая ${g.count} раз с предлогами места.`,
    }));
}

// ====== 4. Темы ======
const THEME_KEYWORDS: Record<string, string> = {
  "тиша":"Тишина","мовчання":"Молчание","пам'ять":"Память","світло":"Свет","темрява":"Тьма","тінь":"Тень","тіні":"Тень",
  "страх":"Страх","надія":"Надежда","любов":"Любовь","зрада":"Предательство","самотність":"Одиночество","доля":"Судьба",
  "свобода":"Свобода","вибір":"Выбор","правда":"Правда","брехня":"Ложь","війна":"Война","смерть":"Смерть","життя":"Жизнь",
  "кров":"Кровь","вогонь":"Огонь","вода":"Вода","повітря":"Воздух","земля":"Земля","небо":"Небо","біль":"Боль","гнів":"Гнев",
  "час":"Время","мить":"Мгновение","вічність":"Вечность","голос":"Голос","шепіт":"Шёпот","слово":"Слово","мова":"Язык/речь",
  "тишина":"Тишина","молчание":"Молчание","свет":"Свет","тьма":"Тьма","тень":"Тень","надежда":"Надежда","любовь":"Любовь",
  "предательство":"Предательство","одиночество":"Одиночество","судьба":"Судьба","выбор":"Выбор",
  "война":"Война","жизнь":"Жизнь","кровь":"Кровь","огонь":"Огонь","воздух":"Воздух","вечность":"Вечность","мгновение":"Мгновение",
  "время":"Время","боль":"Боль","печаль":"Печаль","радость":"Радость","гнев":"Гнев","нежность":"Нежность","шёпот":"Шёпот",
  "бездна":"Бездна","мрак":"Мрак",
  "silence":"Silence","memory":"Memory","light":"Light","darkness":"Darkness","shadow":"Shadow","fear":"Fear","hope":"Hope",
  "love":"Love","betrayal":"Betrayal","loneliness":"Loneliness","fate":"Fate","freedom":"Freedom","choice":"Choice",
  "truth":"Truth","war":"War","death":"Death","life":"Life","blood":"Blood","fire":"Fire","water":"Water","air":"Air",
  "eternity":"Eternity","time":"Time","pain":"Pain","voice":"Voice","whisper":"Whisper","word":"Word","abyss":"Abyss",
};

function detectThemes(text: string): ParsedTheme[] {
  const lowerText = text.toLowerCase();
  const counts: Record<string, number> = {};

  for (const [keyword, themeName] of Object.entries(THEME_KEYWORDS)) {
    const re = new RegExp(`(?<![a-zа-яё\\u0400-\\u04FF])${keyword}(?![a-zа-яё\\u0400-\\u04FF])`, "g");
    const matches = lowerText.match(re);
    if (matches && matches.length >= 5) {
      counts[themeName] = (counts[themeName] || 0) + matches.length;
    }
  }

  return Object.entries(counts)
    .filter(([, c]) => c >= 5)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 10)
    .map(([name, count]) => ({
      name, count,
      description: `Сквозной мотив «${name.toLowerCase()}» — встречается ${count} раз в тексте.`,
    }));
}

// ====== 5. Сборка графа ======
function escapeReg(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function buildGraph(
  chapters: ParsedChapter[],
  prologueText: string,
  characters: ParsedCharacter[],
  locations: ParsedLocation[],
  epsilonResults: import("@/lib/litgraph/epsilon").ChapterEpsilonResult[],
  prologueEpsilon: import("@/lib/litgraph/epsilon").ChapterEpsilonResult | null,
) {
  const nodes: GraphNode[] = [];
  const edges: GraphEdge[] = [];

  let prologueId: string | null = null;
  if (prologueText.trim().length > 100) {
    prologueId = uid("ch");
    const body = prologueText.replace(/\s+/g, " ").trim().slice(0, 400) + "…";
    const meta: Record<string, unknown> = {
      wordCount: prologueText.split(/\s+/).length,
    };
    if (prologueEpsilon) {
      meta.epsilon = Math.round(prologueEpsilon.normalized);
      meta.emotion = prologueEpsilon.emotionCount;
    }
    nodes.push({
      id: prologueId, type: "chapter", position: { x: 0, y: 0 },
      data: {
        title: "Пролог", body, type: "chapter", tags: ["пролог"],
        meta, fullText: prologueText,
      },
    });
  }

  const chapterIds: Record<number, string> = {};
  for (let idx = 0; idx < chapters.length; idx++) {
    const ch = chapters[idx];
    const id = uid("ch");
    chapterIds[ch.num] = id;
    const chChars = characters
      .filter((c) => c.aliases.some((a) => new RegExp(`(?<![a-zA-Z\\u0400-\\u04FF])${escapeReg(a)}(?![a-zA-Z\\u0400-\\u04FF])`).test(ch.fullText)))
      .map((c) => c.name);
    const chLocs = locations
      .filter((l) => l.aliases.some((a) => new RegExp(escapeReg(a)).test(ch.fullText)))
      .map((l) => l.name);
    const eps = epsilonResults[idx];
    const meta: Record<string, unknown> = {
      wordCount: ch.fullText.split(/\s+/).length,
      epsilon: Math.round(eps.normalized),
      emotion: eps.emotionCount,
      uniqueWords: eps.uniqueWords,
      fragments: eps.fragments.map((f) => ({
        p: f.position,
        e: Math.round(f.epsilon),
        emo: f.emotionCount,
      })),
    };
    if (chChars.length) meta.characters = chChars.slice(0, 5).join(", ");
    if (chLocs.length) meta.locations = chLocs.slice(0, 3).join(", ");
    nodes.push({
      id, type: "chapter", position: { x: 0, y: 0 },
      data: {
        title: `Глава ${ch.num}: ${ch.title}`, body: ch.body,
        type: "chapter", tags: [], meta, fullText: ch.fullText,
      },
    });
  }

  const charIds: Record<string, string> = {};
  for (const c of characters) {
    const id = uid("chr");
    charIds[c.name] = id;
    const chaptersWith = chapters.filter((ch) =>
      c.aliases.some((a) => new RegExp(`(?<![a-zA-Z\\u0400-\\u04FF])${escapeReg(a)}(?![a-zA-Z\\u0400-\\u04FF])`).test(ch.fullText))
    );
    nodes.push({
      id, type: "character", position: { x: 0, y: 0 },
      data: {
        title: c.name, body: c.description, type: "character", tags: [],
        meta: {
          mentions: c.count,
          chapters: `${chaptersWith.length} глав`,
          firstChapter: chaptersWith.length ? `Глава ${chaptersWith[0].num}` : "—",
        },
      },
    });
  }

  const locIds: Record<string, string> = {};
  for (const l of locations) {
    const id = uid("loc");
    locIds[l.name] = id;
    const chaptersWith = chapters.filter((ch) =>
      l.aliases.some((a) => new RegExp(escapeReg(a)).test(ch.fullText))
    );
    nodes.push({
      id, type: "location", position: { x: 0, y: 0 },
      data: {
        title: l.name, body: l.description, type: "location", tags: [],
        meta: {
          mentions: l.count,
          chapters: `${chaptersWith.length} глав`,
          firstChapter: chaptersWith.length ? `Глава ${chaptersWith[0].num}` : "—",
        },
      },
    });
  }

  // Связи: поток глав
  const ordered: string[] = [];
  if (prologueId) ordered.push(prologueId);
  for (const ch of chapters) ordered.push(chapterIds[ch.num]);
  for (let i = 0; i < ordered.length - 1; i++) {
    edges.push({
      id: uid("e"), source: ordered[i], target: ordered[i + 1],
      sourceHandle: null, targetHandle: null, type: "smoothstep", animated: true,
      data: { kind: "flow" },
    });
  }

  // Персонажи → главы
  for (const c of characters) {
    const cid = charIds[c.name];
    for (const ch of chapters) {
      const count = c.aliases.reduce(
        (sum, a) => sum + (ch.fullText.match(new RegExp(`(?<![a-zA-Z\\u0400-\\u04FF])${escapeReg(a)}(?![a-zA-Z\\u0400-\\u04FF])`, "g")) || []).length,
        0
      );
      if (count >= 3) {
        edges.push({
          id: uid("e"), source: cid, target: chapterIds[ch.num],
          sourceHandle: null, targetHandle: null, type: "smoothstep", animated: false,
          data: { kind: "character" },
        });
      }
    }
  }

  // Локации → главы
  for (const l of locations) {
    const lid = locIds[l.name];
    for (const ch of chapters) {
      const count = l.aliases.reduce(
        (sum, a) => sum + (ch.fullText.match(new RegExp(escapeReg(a), "g")) || []).length,
        0
      );
      if (count >= 2) {
        edges.push({
          id: uid("e"), source: lid, target: chapterIds[ch.num],
          sourceHandle: null, targetHandle: null, type: "smoothstep", animated: false,
          data: { kind: "location" },
        });
      }
    }
  }

  // Раскладка
  const CHAPTER_X = 600, CHAPTER_Y_START = 60, CHAPTER_Y_STEP = 130;
  for (let i = 0; i < chapters.length; i++) {
    const n = nodes.find((x) => x.id === chapterIds[chapters[i].num]);
    if (n) n.position = { x: CHAPTER_X, y: CHAPTER_Y_START + i * CHAPTER_Y_STEP };
  }
  if (prologueId) {
    const n = nodes.find((x) => x.id === prologueId);
    if (n) n.position = { x: CHAPTER_X, y: CHAPTER_Y_START - CHAPTER_Y_STEP };
  }
  const CHAR_X = 1100, CHAR_Y_START = 60, CHAR_Y_STEP = 110;
  for (let i = 0; i < characters.length; i++) {
    const n = nodes.find((x) => x.id === charIds[characters[i].name]);
    if (n) n.position = { x: CHAR_X, y: CHAR_Y_START + i * CHAR_Y_STEP };
  }
  const LOC_X = 1500, LOC_Y_START = 60, LOC_Y_STEP = 110;
  for (let i = 0; i < locations.length; i++) {
    const n = nodes.find((x) => x.id === locIds[locations[i].name]);
    if (n) n.position = { x: LOC_X, y: LOC_Y_START + i * LOC_Y_STEP };
  }

  return { nodes, edges };
}

// ====== Обработчик ======
export async function POST(req: NextRequest) {
  try {
    const body = await req.json();
    const markdown: string = body.markdown || "";
    const projectTitle: string = body.projectTitle || "Импортированный проект";
    const author: string = body.author || "";

    if (!markdown.trim()) {
      return NextResponse.json({ error: "Пустой текст" }, { status: 400 });
    }

    const { chapters, prologueText } = detectChapters(markdown);
    const characters = detectCharacters(markdown);
    const locations = detectLocations(markdown);

    // Epsilon: ФРАГМЕНТНЫЙ анализ — разбиваем каждую главу на окна
    // Это убирает "слепоту" — видит горячие точки внутри спокойных глав
    const { counts: globalCounts, total: totalWords } = buildWordCounts(markdown);
    const epsilonResults = normalizeFragmentedEpsilons(
      chapters.map((ch) => computeEpsilonFragmented(ch.fullText, globalCounts, totalWords))
    );
    const prologueEpsilon = prologueText.trim().length > 100
      ? computeEpsilonFragmented(prologueText, globalCounts, totalWords)
      : null;

    const { nodes, edges } = buildGraph(
      chapters, prologueText, characters, locations,
      epsilonResults, prologueEpsilon,
    );

    const wordCount = markdown.split(/\s+/).filter(Boolean).length;

    return NextResponse.json({
      title: projectTitle,
      author,
      description: `Автоматически разобранный текст: ${chapters.length} глав, ${characters.length} персонажей, ${locations.length} локаций, ${edges.length} связей. Epsilon-анализ: ${epsilonResults.length} глав оценены. Всего ${wordCount} слов.`,
      nodes,
      edges,
      createdAt: 0,
      updatedAt: 0,
      stats: {
        chapters: chapters.length,
        characters: characters.length,
        locations: locations.length,
        edges: edges.length,
        words: wordCount,
      },
    });
  } catch (err) {
    console.error("parse-md error:", err);
    return NextResponse.json(
      { error: "Ошибка парсинга: " + (err as Error).message },
      { status: 500 }
    );
  }
}
