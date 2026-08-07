/**
 * NER Bridge — вызов Tauri команды extract_entities.
 *
 * В Tauri: вызывает Rust через invoke.
 * В веб-превью: на данный момент NER не работает (нужен Python).
 * В будущем: можно сделать /api/ner-extract endpoint на Next.js.
 */

import { isTauri, callApi } from "@/lib/litgraph/api";
import type { NerResult } from "./nerTypes";

/**
 * Извлечь сущности из текста.
 *
 * В Tauri: вызывает Rust команду → Python spaCy.
 * В веб-превью: возвращает ошибку (нужен Python backend).
 *
 * @param text текст для анализа (русский)
 * @returns результат NER с сущностями и статистикой
 */
export async function extractEntities(text: string): Promise<NerResult> {
  if (!isTauri) {
    throw new Error(
      "NER доступен только в Tauri-версии. В веб-превью Python spaCy недоступен. " +
      "Соберите desktop-версию: cargo tauri build"
    );
  }

  // callApi сам определит Tauri окружение и вызовет invoke
  const result = await callApi<NerResult>(
    "extract_entities", // Tauri команда
    "/api/ner-extract", // веб endpoint (не используется в Tauri)
    { text },           // параметры
    undefined           // без wrapper
  );

  return result;
}

/**
 * Проверить доступность NER (есть ли Python и spaCy).
 * Делает тестовый запуск на коротком тексте.
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
