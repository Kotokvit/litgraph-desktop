/**
 * NER Bridge — вызов Tauri команд extract_entities и analyze_characters.
 *
 * В Tauri: вызывает Rust через invoke → Python (spaCy + POLER).
 * В веб-превью: на данный момент не работает (нужен Python).
 */

import { isTauri, callApi } from "@/lib/litgraph/api";
import type { NerResult } from "./nerTypes";

// === Типы для графа персонажей (POLER на персонажах) ===

export interface CharacterEdge {
  source: string;
  target: string;
  weight: number;
}

export interface CharacterGraph {
  nodes: string[];
  edges: CharacterEdge[];
  nNodes: number;
  nEdges: number;
}

export interface CharacterCluster {
  cluster: number;
  characters: string[];
  size: number;
  avgDegree: number;
}

export interface PolerResult {
  eigenvalues: number[];
  clusters: CharacterCluster[];
  silhouette: number;
  gamma: number;
  kModes: number;
}

export interface CharacterAnalysisResult {
  entities: NerResult;
  graph: CharacterGraph;
  poler: PolerResult;
  error?: string;
}

/**
 * Извлечь сущности из текста (NER).
 *
 * В Tauri: вызывает Rust команду → Python spaCy.
 * В веб-превью: возвращает ошибку (нужен Python backend).
 */
export async function extractEntities(text: string): Promise<NerResult> {
  if (!isTauri) {
    throw new Error(
      "NER доступен только в Tauri-версии. В веб-превью Python spaCy недоступен. " +
      "Соберите desktop-версию: cargo tauri build"
    );
  }

  const result = await callApi<NerResult>(
    "extract_entities",
    "/api/ner-extract",
    { text },
    undefined
  );

  return result;
}

/**
 * Анализ графа персонажей: NER + POLER-физика.
 *
 * Полный пайплайн:
 * 1. NER извлекает персонажей (spaCy + pymorphy3)
 * 2. Строится граф co-occurrence (кто с кем в одних сценах)
 * 3. POLER-оператор H = Π_Λ(L + γJ - B/m)Π_Λ
 * 4. K-means кластеризация в пространстве собственных векторов
 *
 * В Tauri: вызывает Rust команду → Python (poler_entities.py).
 * Обрабатывает ВЕСЬ текст (чанками по 50k), без обрезки.
 *
 * В веб-превью: не работает (нужен Python).
 */
export async function analyzeCharacters(text: string): Promise<CharacterAnalysisResult> {
  if (!isTauri) {
    throw new Error(
      "Анализ персонажей доступен только в Tauri-версии (нужен Python + spaCy). " +
      "Соберите desktop-версию: cargo tauri build"
    );
  }

  const result = await callApi<CharacterAnalysisResult>(
    "analyze_characters",
    "/api/analyze-characters",
    { text },
    undefined
  );

  return result;
}

/**
 * Проверить доступность NER (есть ли Python и spaCy).
 */
export async function checkNerAvailability(): Promise<{
  available: boolean;
  error?: string;
}> {
  if (!isTauri) {
    return {
      available: false,
      error: "Веб-превью: NER работает только в Tauri desktop",
    };
  }

  try {
    const test = await extractEntities("Тест Анна Москва");
    return { available: test.entities.length > 0 || test.stats.total >= 0 };
  } catch (e) {
    return {
      available: false,
      error: String(e),
    };
  }
}

