// Определение окружения: Tauri или веб-превью
export const isTauri = typeof window !== "undefined" && (
  "__TAURI_INTERNALS__" in window || "__TAURI__" in window
);

// Универсальный вызов: в Tauri → invoke, в веб → fetch
// tauriWrapper: если Rust-команда ожидает параметр обёрнутый в ключ
//   (например parse_md(params: ParseParams) → нужно { params: {...} })
export async function callApi<T = unknown>(
  tauriCommand: string,
  webEndpoint: string,
  payload: Record<string, unknown>,
  tauriWrapper?: string,
): Promise<T> {
  if (isTauri) {
    // @ts-ignore — модуль доступен только в Tauri-проекте
    const mod = await import(/* @vite-ignore */ "@tauri-apps/api/core");
    const args = tauriWrapper ? { [tauriWrapper]: payload } : payload;
    return mod.invoke(tauriCommand, args) as Promise<T>;
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
