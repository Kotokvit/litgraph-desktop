// Определение окружения: Tauri или веб-превью
// В Tauri window.__TAURI_INTERNALS__ существует

export const isTauri = typeof window !== "undefined" && (
  "__TAURI_INTERNALS__" in window || "__TAURI__" in window
);

// Универсальный вызов: в Tauri → invoke, в веб → fetch
export async function callApi<T = unknown>(
  tauriCommand: string,
  webEndpoint: string,
  payload: Record<string, unknown>,
): Promise<T> {
  if (isTauri) {
    // Динамический импорт Tauri API — работает только в десктоп-версии
    // @ts-ignore — модуль @tauri-apps/api/core доступен только в Tauri-проекте
    const mod = await import(/* @vite-ignore */ "@tauri-apps/api/core");
    return mod.invoke(tauriCommand, payload) as Promise<T>;
  } else {
    const res = await fetch(webEndpoint, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    const data = await res.json();
    if (!res.ok) throw new Error(data.error || "Неизвестная ошибка");
    return data as T;
  }
}
