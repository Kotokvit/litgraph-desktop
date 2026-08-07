// Определение окружения: Tauri или веб-превью
export const isTauri = typeof window !== "undefined" && (
  "__TAURI_INTERNALS__" in window || "__TAURI__" in window
);

// Универсальный вызов API
// В веб-превью: fetch к Next.js API route
// В Tauri: invoke через глобальный __TAURI_INTERNALS__ (без import)
export async function callApi<T = unknown>(
  _tauriCommand: string,
  webEndpoint: string,
  payload: Record<string, unknown>,
  tauriWrapper?: string,
): Promise<T> {
  if (isTauri) {
    // В Tauri используем глобальный invoke без import модуля
    const w = window as any;
    const invoke = w.__TAURI_INTERNALS__?.invoke || w.__TAURI__?.core?.invoke;
    if (invoke) {
      const args = tauriWrapper ? { [tauriWrapper]: payload } : payload;
      return invoke(_tauriCommand, args) as Promise<T>;
    }
  }

  // Веб-превью: обычный fetch
  const res = await fetch(webEndpoint, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(payload),
  });
  const data = await res.json();
  if (!res.ok) throw new Error(data.error || "Неизвестная ошибка");
  return data as T;
}
