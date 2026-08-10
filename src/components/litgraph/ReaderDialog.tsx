"use client";

import * as Lucide from "lucide-react";
import { useState, useMemo, useEffect, useRef, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogTitle,
} from "@/components/ui/dialog";
import { useLitStore } from "@/lib/litgraph/store";
import {
  renderAllChapters,
  findChapterIndexForPosition,
  type RenderedChapter,
} from "@/lib/poler/readerRender";
import { detectChapters } from "@/lib/poler/textMoments";

/**
 * Reader — полноэкранный читатель исходного текста с подсветкой:
 *   - Все упоминания ключевых слов узла (subtle violet background)
 *   - Текущий фрагмент-цель (сильная подсветка + авто-скролл)
 *   - ToC глав слева, навигация prev/next момент снизу/сверху
 *
 * Открывается из TextMomentsDialog по клику на момент.
 */
export function ReaderDialog() {
  const open = useLitStore((s) => s.readerOpen);
  const target = useLitStore((s) => s.readerTarget);
  const sourceMarkdown = useLitStore((s) => s.sourceMarkdown);
  const closeReader = useLitStore((s) => s.closeReader);
  const setReaderIndex = useLitStore((s) => s.setReaderIndex);

  const [activeChapterIdx, setActiveChapterIdx] = useState(0);
  const scrollContainerRef = useRef<HTMLDivElement | null>(null);
  // Счётчик для форсирования авто-скролла (меняется при смене target или при
  // ручном клике на "next moment" — даже если currentIndex уже был тот же)
  const [scrollNonce, setScrollNonce] = useState(0);

  // Главы (без рендеринга HTML — только для ToC)
  const chapters = useMemo(
    () => (open ? detectChapters(sourceMarkdown) : []),
    [open, sourceMarkdown]
  );

  // Текущий момент (для подсказки в шапке)
  const currentMoment = target
    ? target.moments[target.currentIndex]
    : null;

  // Рендеринг HTML всех глав. Мемоизуется по [sourceMarkdown, keywords, target.position]
  // — то есть пересчитывается только когда сменилась цель (а не при каждой прокрутке).
  const renderedChapters: RenderedChapter[] = useMemo(() => {
    if (!open || !sourceMarkdown || !target) return [];
    return renderAllChapters(sourceMarkdown, {
      keywords: target.keywords,
      target: currentMoment
        ? { position: currentMoment.position, end: currentMoment.end }
        : null,
    });
  }, [
    open,
    sourceMarkdown,
    target?.keywords,
    target?.moments,
    target?.currentIndex,
    currentMoment?.position,
    currentMoment?.end,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    scrollNonce,
  ]);

  // Авто-скролл к целевому элементу при открытии / смене индекса
  useEffect(() => {
    if (!open || !currentMoment || renderedChapters.length === 0) return;

    // Находим индекс главы, в которой находится текущий момент
    const chIdx = findChapterIndexForPosition(chapters, currentMoment.position);
    if (chIdx !== activeChapterIdx) {
      setActiveChapterIdx(chIdx);
    }

    // Даём React время отрендерить HTML, потом скроллим к якорю
    const t = setTimeout(() => {
      const container = scrollContainerRef.current;
      if (!container) return;
      // Ищем якорь в DOM. Якорь имеет id="reader-target-{bodyOffset}", где
      // bodyOffset = position - chapter.pos - bodyStart. Мы не знаем bodyStart
      // здесь, но можем искать по селектору mark.reader-target.
      const anchor = container.querySelector("mark.reader-target");
      if (anchor) {
        anchor.scrollIntoView({
          behavior: "smooth",
          block: "center",
        });
      } else {
        // Фолбэк: скроллим к главе, содержащей цель
        const chapterEl = container.querySelector(
          `[data-chapter-pos="${chapters[chIdx].pos}"]`
        );
        if (chapterEl) {
          chapterEl.scrollIntoView({
            behavior: "smooth",
            block: "start",
          });
        }
      }
    }, 60);

    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scrollNonce, open, target?.currentIndex]);

  // Сброс активной главы при закрытии
  useEffect(() => {
    if (!open) {
      setActiveChapterIdx(0);
      setScrollNonce(0);
    } else {
      // Форсируем авто-скролл при открытии
      setScrollNonce((n) => n + 1);
    }
  }, [open]);

  // Прокрутка к конкретной главе (по клику в ToC)
  const scrollToChapter = useCallback(
    (idx: number) => {
      if (!scrollContainerRef.current || !chapters[idx]) return;
      setActiveChapterIdx(idx);
      const container = scrollContainerRef.current;
      const el = container.querySelector(
        `[data-chapter-pos="${chapters[idx].pos}"]`
      );
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "start" });
      }
    },
    [chapters]
  );

  // Трекинг скролла: обновляем activeChapterIdx
  useEffect(() => {
    if (!open) return;
    const container = scrollContainerRef.current;
    if (!container) return;
    let raf = 0;
    const onScroll = () => {
      cancelAnimationFrame(raf);
      raf = requestAnimationFrame(() => {
        const scrollTop = container.scrollTop;
        const viewportH = container.clientHeight;
        // Находим главу, чей заголовок выше центра viewport
        const midY = scrollTop + viewportH / 3;
        let activeIdx = 0;
        for (let i = 0; i < chapters.length; i++) {
          const el = container.querySelector(
            `[data-chapter-pos="${chapters[i].pos}"]`
          ) as HTMLElement | null;
          if (!el) continue;
          const elTop = el.offsetTop;
          if (elTop <= midY) {
            activeIdx = i;
          } else {
            break;
          }
        }
        if (activeIdx !== activeChapterIdx) {
          setActiveChapterIdx(activeIdx);
        }
      });
    };
    container.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      container.removeEventListener("scroll", onScroll);
      cancelAnimationFrame(raf);
    };
  }, [open, chapters, activeChapterIdx]);

  // Навигация prev/next момент
  const goPrev = useCallback(() => {
    if (!target) return;
    if (target.currentIndex > 0) {
      setReaderIndex(target.currentIndex - 1);
      setScrollNonce((n) => n + 1);
    }
  }, [target, setReaderIndex]);

  const goNext = useCallback(() => {
    if (!target) return;
    if (target.currentIndex < target.moments.length - 1) {
      setReaderIndex(target.currentIndex + 1);
      setScrollNonce((n) => n + 1);
    }
  }, [target, setReaderIndex]);

  // Горячие клавиши: ← / → для навигации, Esc — закрыть
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "ArrowLeft" && !e.shiftKey) {
        e.preventDefault();
        goPrev();
      } else if (e.key === "ArrowRight" && !e.shiftKey) {
        e.preventDefault();
        goNext();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, goPrev, goNext]);

  if (!open || !target) return null;

  const totalMoments = target.moments.length;
  const idx = target.currentIndex;

  return (
    <Dialog open={open} onOpenChange={(v) => !v && closeReader()}>
      <DialogContent
        className="max-w-[100vw] w-screen h-screen max-h-[100vh] rounded-none p-0 flex flex-col gap-0"
        showCloseButton={false}
      >
        <DialogTitle className="sr-only">
          Читатель: {target.nodeTitle}
        </DialogTitle>

        {/* Шапка */}
        <header className="flex items-center gap-3 px-4 py-2.5 border-b bg-white shrink-0">
          <Button
            size="sm"
            variant="ghost"
            onClick={closeReader}
            className="h-8 text-xs"
          >
            <Lucide.X className="w-4 h-4 mr-1.5" />
            Закрыть
          </Button>

          <div className="h-5 w-px bg-stone-200" />

          <div className="flex items-center gap-2 min-w-0">
            <Lucide.BookOpen className="w-4 h-4 text-violet-600 shrink-0" />
            <span className="text-sm font-semibold text-stone-800 truncate">
              {target.nodeTitle}
            </span>
            <Badge
              variant="secondary"
              className="text-[10px] py-0 px-1.5 bg-violet-50 text-violet-700 border-violet-200 shrink-0"
            >
              {totalMoments} моментов
            </Badge>
          </div>

          {/* Прогресс и навигация */}
          <div className="ml-auto flex items-center gap-2">
            <span className="text-xs text-stone-500 font-mono">
              {idx + 1} / {totalMoments}
            </span>
            <Button
              size="sm"
              variant="outline"
              onClick={goPrev}
              disabled={idx === 0}
              className="h-8 w-8 p-0"
              title="Предыдущий момент (←)"
            >
              <Lucide.ChevronLeft className="w-4 h-4" />
            </Button>
            <Button
              size="sm"
              variant="outline"
              onClick={goNext}
              disabled={idx === totalMoments - 1}
              className="h-8 w-8 p-0"
              title="Следующий момент (→)"
            >
              <Lucide.ChevronRight className="w-4 h-4" />
            </Button>
          </div>
        </header>

        {/* Подсказка про текущий момент */}
        {currentMoment && (
          <div className="px-4 py-1.5 bg-violet-50 border-b border-violet-100 text-xs flex items-center gap-2 shrink-0">
            <Lucide.MapPin className="w-3.5 h-3.5 text-violet-600 shrink-0" />
            <span className="text-violet-900">
              {currentMoment.chapterTitle}
            </span>
            <span className="text-violet-400">·</span>
            <span className="text-violet-700 font-mono">
              pos: {currentMoment.position.toLocaleString()}
            </span>
            {currentMoment.matchedKeyword && (
              <>
                <span className="text-violet-400">·</span>
                <span className="text-violet-700">
                  «{currentMoment.matchedKeyword}»
                </span>
              </>
            )}
            <div className="ml-auto text-[10px] text-violet-500">
              ← → для навигации
            </div>
          </div>
        )}

        {/* Тело: ToC + текст */}
        <div className="flex-1 flex overflow-hidden">
          {/* ToC */}
          <aside className="w-56 border-r bg-stone-50 shrink-0 flex flex-col">
            <div className="text-[10px] uppercase tracking-wider text-stone-400 p-2 pb-1 border-b">
              Главы ({chapters.length})
            </div>
            <div className="flex-1 overflow-y-auto lit-scroll">
              {chapters.map((c, i) => {
                // Подсчитываем моменты в этой главе
                const momentsInChapter = target.moments.filter(
                  (m) => m.position >= c.pos && m.position < c.end
                ).length;
                const isActive = i === activeChapterIdx;
                return (
                  <button
                    key={`${c.pos}-${i}`}
                    onClick={() => scrollToChapter(i)}
                    className={`w-full text-left px-2 py-1.5 text-xs transition-colors border-l-2 ${
                      isActive
                        ? "bg-violet-100 border-violet-500 text-violet-900 font-medium"
                        : "border-transparent text-stone-600 hover:bg-stone-100"
                    }`}
                  >
                    <div className="flex items-baseline justify-between gap-1">
                      <span className="truncate">{c.title}</span>
                      {momentsInChapter > 0 && (
                        <span
                          className={`text-[10px] shrink-0 ${
                            isActive ? "text-violet-600" : "text-stone-400"
                          }`}
                        >
                          {momentsInChapter}
                        </span>
                      )}
                    </div>
                  </button>
                );
              })}
            </div>
          </aside>

          {/* Текст */}
          <div
            ref={scrollContainerRef}
            className="flex-1 overflow-y-auto lit-scroll"
          >
            <div className="reader-content max-w-3xl mx-auto px-6 py-8 prose prose-sm">
              {renderedChapters.map((rc, i) => (
                <div
                  key={`${rc.chapter.pos}-${i}`}
                  dangerouslySetInnerHTML={{ __html: rc.html }}
                />
              ))}
            </div>
          </div>
        </div>

        {/* Стили для подсветки (inline, чтобы не зависеть от tailwind config) */}
        <style>{`
          .reader-content h2.reader-chapter {
            font-size: 1.25rem;
            font-weight: 700;
            color: #1f2937;
            margin: 2rem 0 1rem;
            padding-bottom: 0.5rem;
            border-bottom: 1px solid #e5e7eb;
            scroll-margin-top: 1rem;
          }
          .reader-content p {
            margin: 0 0 1rem;
            line-height: 1.75;
            color: #292524;
          }
          .reader-content mark.reader-keyword {
            background-color: rgba(139, 92, 246, 0.18);
            color: #5b21b6;
            padding: 0 2px;
            border-radius: 2px;
            font-weight: 500;
          }
          .reader-content mark.reader-target {
            background-color: rgba(245, 158, 11, 0.35);
            color: #78350f;
            padding: 2px 4px;
            border-radius: 3px;
            font-weight: 600;
            box-shadow: 0 0 0 2px rgba(245, 158, 11, 0.4);
            scroll-margin-top: 80px;
            scroll-margin-bottom: 80px;
          }
          .reader-content br + br {
            display: block;
            margin-top: 0.5rem;
            content: "";
          }
        `}</style>
      </DialogContent>
    </Dialog>
  );
}
