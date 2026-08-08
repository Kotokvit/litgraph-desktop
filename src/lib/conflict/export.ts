// Экспорт конфликт-графа в PNG/PDF.
//
// Использует html-to-image (фиксит проблемы с oklch-цветами Tailwind v4
// лучше, чем html2canvas) и jsPDF для PDF-обёртки.
//
// Сохранение идёт через Tauri save dialog + writeBinaryFile —
// пользователь сам выбирает путь. В dev-режиме (без Tauri) — fallback
// на браузерный <a download>.

import { toPng } from "html-to-image";
import { jsPDF } from "jspdf";

/**
 * Скачивает бинарный файл через Tauri save dialog.
 * В dev-режиме (без Tauri) — fallback на браузерный download.
 */
async function saveBinaryFile(
  bytes: Uint8Array,
  filename: string,
  mime: string,
): Promise<string | null> {
  // Tauri: открываем диалог сохранения
  try {
    const isTauri = (await import("@tauri-apps/api/core")).isTauri();
    if (isTauri) {
      const { save } = await import("@tauri-apps/plugin-dialog");
      const { writeFile } = await import("@tauri-apps/plugin-fs");
      const ext = filename.split(".").pop() || "bin";
      const path = await save({
        defaultPath: filename,
        filters: [
          {
            name: ext.toUpperCase(),
            extensions: [ext],
          },
        ],
      });
      if (path) {
        // writeFile принимает ArrayBuffer | Uint8Array.
        await writeFile(path, bytes);
        return path;
      }
      return null;
    }
  } catch (err) {
    console.warn("Tauri save failed, falling back to browser download:", err);
  }

  // Браузерный fallback.
  // Cast: TS 5.7+ строже к Uint8Array<ArrayBufferLike> vs BlobPart.
  // bytes.buffer — ArrayBufferLike; приводим к ArrayBuffer для Blob.
  const blob = new Blob([bytes.buffer as ArrayBuffer], { type: mime });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return filename;
}

/**
 * Подготавливает DOM-узел к снимку: временно убираем max-height/overflow,
 * чтобы захватить весь контент, а не только видимую часть.
 *
 * Возвращает функцию восстановления оригинальных стилей.
 */
function prepareForCapture(element: HTMLElement): () => void {
  const orig: Record<string, string> = {
    maxHeight: element.style.maxHeight,
    overflow: element.style.overflow,
    height: element.style.height,
  };

  // Снимаем ограничения по высоте и скроллу — пусть контент
  // разворачивается на полную высоту.
  element.style.maxHeight = "none";
  element.style.overflow = "visible";
  element.style.height = "auto";

  // Принудительно белый фон (иначе прозрачный PNG выглядит плохо).
  element.style.backgroundColor = "#ffffff";

  return () => {
    element.style.maxHeight = orig.maxHeight;
    element.style.overflow = orig.overflow;
    element.style.height = orig.height;
    element.style.backgroundColor = "";
  };
}

/**
 * Делает PNG-снимок DOM-узла и сохраняет в файл.
 *
 * @param element DOM-узел для захвата (обычно — контейнер с отчётом)
 * @param filename Имя файла по умолчанию (например, "conflict-graph.png")
 */
export async function exportConflictToPng(
  element: HTMLElement,
  filename: string,
): Promise<string | null> {
  const restore = prepareForCapture(element);
  try {
    // Небольшая задержка, чтобы React/Tailwind успели перерисовать
    // после смены стилей.
    await new Promise((r) => setTimeout(r, 50));

    const dataUrl = await toPng(element, {
      // Увеличиваем pixelRatio для чёткости на retina/печати.
      pixelRatio: 2,
      // Фильтруем tooltip-ы: они показываются при hover и могут попасть
      // в снимок. Если элемент имеет data-export-ignore — пропускаем.
      filter: (node) => {
        if (node instanceof HTMLElement) {
          return !node.dataset.exportIgnore;
        }
        return true;
      },
      // Явный фон, чтобы не было артефактов прозрачности.
      backgroundColor: "#ffffff",
      // Включаем кэширование шрифтов (важно для Cyrillic).
      cacheBust: true,
    });

    // dataUrl → bytes
    const resp = await fetch(dataUrl);
    const buf = await resp.arrayBuffer();
    const bytes = new Uint8Array(buf);
    return saveBinaryFile(bytes, filename, "image/png");
  } finally {
    restore();
  }
}

/**
 * Делает PDF-документ из DOM-узла: PNG-снимок встраивается в A4 landscape.
 *
 * Если контент высокий — автоматически разбивается на несколько страниц.
 *
 * @param element DOM-узел для захвата
 * @param filename Имя файла по умолчанию (например, "conflict-graph.pdf")
 */
export async function exportConflictToPdf(
  element: HTMLElement,
  filename: string,
): Promise<string | null> {
  const restore = prepareForCapture(element);
  try {
    await new Promise((r) => setTimeout(r, 50));

    const dataUrl = await toPng(element, {
      pixelRatio: 2,
      filter: (node) => {
        if (node instanceof HTMLElement) {
          return !node.dataset.exportIgnore;
        }
        return true;
      },
      backgroundColor: "#ffffff",
      cacheBust: true,
    });

    // Загружаем картинку, чтобы узнать её реальные размеры.
    const img = new Image();
    img.src = dataUrl;
    await new Promise<void>((resolve, reject) => {
      img.onload = () => resolve();
      img.onerror = () => reject(new Error("Не удалось загрузить PNG для PDF"));
    });

    const pxToMm = (px: number) => (px * 25.4) / 96; // 96 DPI → mm
    const imgWidthMm = pxToMm(img.width);
    const imgHeightMm = pxToMm(img.height);

    // A4 landscape: 297 × 210 mm. Оставляем поля по 8 мм.
    const pageW = 297;
    const pageH = 210;
    const margin = 8;
    const usableW = pageW - margin * 2;
    const usableH = pageH - margin * 2;

    // Масштабируем по ширине.
    const scale = usableW / imgWidthMm;
    const scaledW = imgWidthMm * scale;
    const scaledH = imgHeightMm * scale;

    const pdf = new jsPDF({
      orientation: "landscape",
      unit: "mm",
      format: "a4",
    });

    if (scaledH <= usableH) {
      // Помещается на одну страницу — центрируем по высоте.
      const offsetY = margin + (usableH - scaledH) / 2;
      pdf.addImage(dataUrl, "PNG", margin, offsetY, scaledW, scaledH);
    } else {
      // Разбиваем на несколько страниц: на каждой странице показываем
      // следующий вертикальный срез исходного изображения.
      // Реализация: вычисляем шаг (какой срез картинки помещается на 1 страницу),
      // затем добавляем страницы и смещаем картинку вверх.
      const pageContentH = usableH; // доступная высота на странице в mm
      const pageContentHPx = pageContentH / scale; // в пикселях исходной картинки
      let srcY = 0; // текущее смещение в пикселях
      let isFirst = true;

      while (srcY < img.height) {
        if (!isFirst) {
          pdf.addPage();
        }
        isFirst = false;

        // Смещение в mm относительно верхнего края usable-области.
        // Поскольку картинка шире/выше страницы, мы рисуем её целиком,
        // но со смещением так, чтобы нужный срез оказался в usable-области.
        const srcYMm = pxToMm(srcY) * scale; // смещение в mm
        // Сдвигаем картинку вверх на srcYMm относительно margin.
        pdf.addImage(
          dataUrl,
          "PNG",
          margin,
          margin - srcYMm,
          scaledW,
          scaledH,
        );

        srcY += pageContentHPx;
      }
    }

    const bytes = pdf.output("arraybuffer");
    return saveBinaryFile(
      new Uint8Array(bytes),
      filename,
      "application/pdf",
    );
  } finally {
    restore();
  }
}
