/**
 * background-layer.ts
 * ===================
 * Импорт и декодирование фонового слоя для canvas.
 *
 * Поддерживаемые форматы:
 *  - SVG  → нативно через new Image() (data: URL)
 *  - PNG  → нативно
 *  - JPEG → нативно
 *  - WebP → нативно (современные браузеры)
 *  - TIFF → через UTIF (tiff/tif)
 *
 * Стратегия:
 *  1. Читаем файл как ArrayBuffer.
 *  2. Если TIFF — декодируем через UTIF в RGBA, затем конвертируем в PNG data: URL.
 *  3. Иначе — оборачиваем в data: URL напрямую (base64).
 *  4. Загружаем через new Image() чтобы получить naturalWidth/Height.
 *  5. Возвращаем готовый BackgroundLayer с дефолтными параметрами.
 *
 * Автор затем может:
 *  - перетаскивать фон мышью (CanvasRenderer делает hit-test)
 *  - менять opacity / scale / rotation через Inspector
 *  - скрывать / показывать / лочить
 */

import type { BackgroundLayer, BackgroundFormat } from "./types";

// Динамический импорт UTIF чтобы не тащить в бандл если не нужен
let _UTIF: typeof import("utif") | null = null;
async function getUTIF() {
  if (!_UTIF) {
    _UTIF = await import("utif");
  }
  return _UTIF;
}

/** Определить формат файла по имени и/или MIME */
export function detectFormat(fileName: string, mime?: string): BackgroundFormat {
  const lower = fileName.toLowerCase();
  if (lower.endsWith(".svg") || mime === "image/svg+xml") return "svg";
  if (lower.endsWith(".png") || mime === "image/png") return "png";
  if (lower.endsWith(".tif") || lower.endsWith(".tiff") || mime === "image/tiff") return "tiff";
  if (lower.endsWith(".jpg") || lower.endsWith(".jpeg") || mime === "image/jpeg") return "jpeg";
  if (lower.endsWith(".webp") || mime === "image/webp") return "webp";
  return "image";
}

/** Прочитать файл как ArrayBuffer */
function readArrayBuffer(file: File): Promise<ArrayBuffer> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as ArrayBuffer);
    reader.onerror = () => reject(reader.error ?? new Error("FileReader failed"));
    reader.readAsArrayBuffer(file);
  });
}

/** ArrayBuffer → base64 data: URL */
function arrayBufferToDataUrl(buf: ArrayBuffer, mime: string): string {
  const bytes = new Uint8Array(buf);
  let binary = "";
  const chunk = 0x8000; // 32K chunks — избегаем stack overflow на String.fromCharCode
  for (let i = 0; i < bytes.length; i += chunk) {
    const slice = bytes.subarray(i, Math.min(i + chunk, bytes.length));
    binary += String.fromCharCode.apply(null, Array.from(slice));
  }
  const base64 = btoa(binary);
  return `data:${mime};base64,${base64}`;
}

/**
 * Декодировать TIFF через UTIF.
 * Возвращает PNG data: URL.
 */
async function decodeTiffToPngDataUrl(buf: ArrayBuffer): Promise<{ src: string; width: number; height: number }> {
  const UTIF = await getUTIF();
  const ifds = UTIF.decode(buf);
  if (!ifds || ifds.length === 0) {
    throw new Error("TIFF: не найдено IFD (invalid TIFF)");
  }
  // Первый кадр
  const first = ifds[0];
  UTIF.decodeImage(buf, first, ifds);
  const rgba = UTIF.toRGBA8(first);
  const width: number = first.width;
  const height: number = first.height;

  // RGBA → canvas → PNG
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Canvas 2D context unavailable");
  const imageData = new ImageData(new Uint8ClampedArray(rgba), width, height);
  ctx.putImageData(imageData, 0, 0);
  const pngDataUrl = canvas.toDataURL("image/png");
  return { src: pngDataUrl, width, height };
}

/** Загрузить data: URL в HTMLImageElement, вернуть naturalWidth/Height */
function loadImage(src: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("Image failed to load"));
    img.src = src;
  });
}

export interface ImportOptions {
  /** Начальная позиция в мировых координатах canvas (если null — центрируется в viewport) */
  position?: { x: number; y: number } | null;
  /** Начальный масштаб (если null — подбирается чтобы влезть в ~600px) */
  scale?: number | null;
  /** Непрозрачность (по умолчанию 0.55) */
  opacity?: number;
}

/**
 * Главный публичный метод: декодировать File в BackgroundLayer.
 *
 * Бросает Error при неподдерживаемом формате или сбое декодирования.
 */
export async function importBackgroundImage(
  file: File,
  opts: ImportOptions = {},
): Promise<BackgroundLayer> {
  const format = detectFormat(file.name, file.type);
  let src: string;
  let naturalWidth: number;
  let naturalHeight: number;

  if (format === "tiff") {
    // TIFF — декодируем через UTIF
    const buf = await readArrayBuffer(file);
    const decoded = await decodeTiffToPngDataUrl(buf);
    src = decoded.src;
    naturalWidth = decoded.width;
    naturalHeight = decoded.height;
  } else {
    // SVG / PNG / JPEG / WebP — читаем как data: URL напрямую
    const mime =
      format === "svg" ? "image/svg+xml" :
      format === "png" ? "image/png" :
      format === "jpeg" ? "image/jpeg" :
      format === "webp" ? "image/webp" :
      file.type || "application/octet-stream";

    const buf = await readArrayBuffer(file);
    src = arrayBufferToDataUrl(buf, mime);

    // Загружаем чтобы получить naturalWidth/Height
    const img = await loadImage(src);
    naturalWidth = img.naturalWidth || img.width;
    naturalHeight = img.naturalHeight || img.height;
  }

  if (!naturalWidth || !naturalHeight) {
    throw new Error("Не удалось определить размеры изображения");
  }

  // Подбираем масштаб чтобы изображение было ~600px по большей стороне
  const targetMaxSide = 600;
  const defaultScale =
    opts.scale ??
    Math.min(
      targetMaxSide / naturalWidth,
      targetMaxSide / naturalHeight,
      1, // не увеличиваем маленькие изображения
    );

  return {
    id: `bg_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`,
    src,
    format,
    name: file.name,
    naturalWidth,
    naturalHeight,
    opacity: opts.opacity ?? 0.55,
    visible: true,
    x: opts.position?.x ?? 0,
    y: opts.position?.y ?? 0,
    scale: defaultScale,
    rotation: 0,
    locked: false,
    pinnedToScreen: false,
  };
}

/**
 * Открыть системный диалог выбора файла через Tauri (если доступен)
 * или через скрытый <input type=file> в браузере.
 *
 * Возвращает File | null (null если пользователь отменил).
 */
export async function pickImageFileViaDialog(): Promise<File | null> {
  // Tauri detection
  const isTauri =
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window);

  if (isTauri) {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({
        multiple: false,
        filters: [
          {
            name: "Изображения (SVG, PNG, TIFF, JPEG, WebP)",
            extensions: ["svg", "png", "tiff", "tif", "jpg", "jpeg", "webp"],
          },
        ],
      });
      if (!selected) return null;

      // Tauri возвращает путь (string) или { path, name } — нужно прочитать файл
      const path = typeof selected === "string" ? selected : (selected as any).path;
      if (!path) return null;

      // Читаем как бинарник через Tauri fs
      const { readFile } = await import("@tauri-apps/plugin-fs");
      const bytes = await readFile(path);
      const name = path.split(/[\\/]/).pop() || "background";
      // Определяем MIME по расширению
      const mime =
        name.toLowerCase().endsWith(".svg") ? "image/svg+xml" :
        name.toLowerCase().endsWith(".png") ? "image/png" :
        name.toLowerCase().endsWith(".tif") || name.toLowerCase().endsWith(".tiff") ? "image/tiff" :
        name.toLowerCase().endsWith(".jpg") || name.toLowerCase().endsWith(".jpeg") ? "image/jpeg" :
        name.toLowerCase().endsWith(".webp") ? "image/webp" :
        "application/octet-stream";
      return new File([bytes], name, { type: mime });
    } catch (err) {
      console.warn("[LitGraph] Tauri dialog failed, falling back to <input>:", err);
      // проваливаемся в браузерный fallback
    }
  }

  // Браузерный fallback — через скрытый <input type=file>
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".svg,.png,.tiff,.tif,.jpg,.jpeg,.webp,image/*";
    input.onchange = () => {
      const f = input.files?.[0];
      resolve(f ?? null);
    };
    // Если пользователь закрывает диалог без выбора — onchange не сработает,
    // промис останется pending. Это приемлемо для MVP.
    input.click();
  });
}

/**
 * Подсказка для UI: читать ли файл как data: URL через FileReader
 * (старый путь, как в .md импорте), или использовать pickImageFileViaDialog.
 * Возвращает true если Tauri доступна и лучше использовать нативный диалог.
 */
export function preferNativeDialog(): boolean {
  return (
    typeof window !== "undefined" &&
    ("__TAURI_INTERNALS__" in window || "__TAURI__" in window)
  );
}
