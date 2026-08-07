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
    // В Tauri-проекте @tauri-apps/api/core установлен
    // Динамический import через Function чтобы webpack/turbopack не пытался резолвить
    const invoke = (await (new Function('return import("@tauri-apps/api/core")')()) as any).invoke;
    const args = tauriWrapper ? { [tauriWrapper]: payload } : payload;
    return invoke(tauriCommand, args) as Promise<T>;
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
