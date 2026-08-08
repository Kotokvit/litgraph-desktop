// IPC-мост для конфликт-графа: вызывает Tauri-команду get_conflict_graph,
// которая запускает Python-пайплайн (NER + SVO + J-матрица) и возвращает
// типизированный ConflictGraph.

import { callApi, isTauri } from "@/lib/litgraph/api";
import type { ConflictGraph } from "./types";

/**
 * Построить конфликт-граф из текста: текст → NER → SVO → J-матрица.
 *
 * В Tauri: вызывает Rust-команду `get_conflict_graph`, которая через
 * std::process::Command запускает conflict_graph.py (с spaCy + pymorphy3).
 *
 * В веб-превью: бросает ошибку — Python недоступен без Tauri.
 *
 * @param text Исходный текст произведения (главы, сцены).
 * @returns ConflictGraph с nodes, edges, matrix, stats.
 */
export async function getConflictGraph(text: string): Promise<ConflictGraph> {
  if (!isTauri) {
    throw new Error(
      "Конфликт-граф доступен только в Tauri-версии (нужен Python + spaCy). " +
        "Соберите desktop-версию: cargo tauri build",
    );
  }
  return callApi<ConflictGraph>(
    "get_conflict_graph", // Tauri command name
    "/api/conflict-graph", // legacy web endpoint (не используется в Tauri)
    { text }, // payload — становится { text: "..." }
    undefined, // без wrapper-ключа
  );
}
