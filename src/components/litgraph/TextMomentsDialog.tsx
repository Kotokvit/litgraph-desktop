"use client";

import * as Lucide from "lucide-react";
import { useState, useMemo, useEffect, useCallback } from "react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { useLitStore } from "@/lib/litgraph/store";
import {
  findTextMoments,
  highlightKeywords,
  extractKeywords,
  type TextMomentsResult,
} from "@/lib/poler/textMoments";
import type { LitNode } from "@/lib/litgraph/types";

interface TextMomentsDialogProps {
  open: boolean;
  nodeId: string | null;
  onClose: () => void;
}

export function TextMomentsDialog({
  open,
  nodeId,
  onClose,
}: TextMomentsDialogProps) {
  const nodes = useLitStore((s) => s.nodes);
  const sourceMarkdown = useLitStore((s) => s.sourceMarkdown);

  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<TextMomentsResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [contextChars, setContextChars] = useState(200);
  const [activeChapterKey, setActiveChapterKey] = useState<string | null>(null);

  const node: LitNode | undefined = useMemo(
    () => (nodeId ? nodes.find((n) => n.id === nodeId) : undefined),
    [nodes, nodeId]
  );

  const keywords = useMemo(
    () => (node ? extractKeywords(node) : []),
    [node]
  );

  const runAnalysis = useCallback(() => {
    if (!node || !sourceMarkdown.trim()) {
      setError(
        !node
          ? "Узел не выбран"
          : "Исходный markdown пустой. Импортируйте .md файл, чтобы включить поиск по тексту."
      );
      setResult(null);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      // Выполняем в setTimeout чтобы UI успел отрисовать loading state
      setTimeout(() => {
        try {
          const r = findTextMoments(sourceMarkdown, node, {
            contextChars,
            maxMoments: 200,
          });
          setResult(r);
          if (r.byChapter.length > 0) {
            setActiveChapterKey(
              `${r.byChapter[0].chapter.num}-${r.byChapter[0].chapter.suffix}`
            );
          }
        } catch (e) {
          setError(String(e));
        } finally {
          setLoading(false);
        }
      }, 0);
    } catch (e) {
      setError(String(e));
      setLoading(false);
    }
  }, [node, sourceMarkdown, contextChars]);

  // Авто-запуск при открытии
  useEffect(() => {
    if (open && nodeId && !result) {
      runAnalysis();
    }
    if (!open) {
      setResult(null);
      setError(null);
      setActiveChapterKey(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, nodeId]);

  const activeChapter = useMemo(() => {
    if (!result || !activeChapterKey) return null;
    return result.byChapter.find(
      (c) => `${c.chapter.num}-${c.chapter.suffix}` === activeChapterKey
    );
  }, [result, activeChapterKey]);

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-5xl max-h-[92vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lucide.Search className="w-5 h-5 text-violet-600" />
            Моменты в тексте:{" "}
            <span className="text-violet-700">{node?.data.title ?? "—"}</span>
          </DialogTitle>
        </DialogHeader>

        {/* Контекстная панель */}
        <div className="flex items-center gap-3 px-1 py-2 border-b text-xs">
          <div className="flex items-center gap-1.5">
            <Lucide.FileText className="w-3.5 h-3.5 text-stone-500" />
            <span className="text-stone-500">Ключевые слова:</span>
            <div className="flex flex-wrap gap-1">
              {keywords.length === 0 ? (
                <span className="text-stone-400 italic">не найдены</span>
              ) : (
                keywords.slice(0, 8).map((k) => (
                  <Badge
                    key={k}
                    variant="secondary"
                    className="text-[10px] py-0 px-1.5 bg-violet-50 text-violet-700 border-violet-200"
                  >
                    {k}
                  </Badge>
                ))
              )}
              {keywords.length > 8 && (
                <span className="text-stone-400">+{keywords.length - 8}</span>
              )}
            </div>
          </div>
          <div className="ml-auto flex items-center gap-2">
            <label className="text-stone-500">Окно:</label>
            <input
              type="range"
              min={80}
              max={500}
              step={20}
              value={contextChars}
              onChange={(e) => setContextChars(parseInt(e.target.value))}
              className="w-24 accent-violet-600"
            />
            <span className="text-stone-600 font-mono w-12 text-right">
              {contextChars}
            </span>
            <Button
              size="sm"
              variant="outline"
              onClick={runAnalysis}
              disabled={loading || !node || !sourceMarkdown.trim()}
              className="h-7 text-xs"
            >
              <Lucide.RefreshCw
                className={`w-3 h-3 mr-1 ${loading ? "animate-spin" : ""}`}
              />
              Пересканировать
            </Button>
          </div>
        </div>

        {error && (
          <div className="rounded-md bg-red-50 border border-red-200 p-2.5 text-sm text-red-700 m-3">
            <Lucide.AlertCircle className="w-4 h-4 inline mr-1.5" />
            {error}
          </div>
        )}

        {!error && result && (
          <>
            {/* Метрики */}
            <div className="grid grid-cols-4 gap-2 m-3 mb-1">
              <MetricBox
                label="Моментов"
                value={result.stats.totalMoments.toString()}
                color="#1f77b4"
              />
              <MetricBox
                label="Глав"
                value={result.stats.totalChapters.toString()}
                color="#ff7f0e"
              />
              <MetricBox
                label="Средняя плотность"
                value={result.stats.avgDensity.toFixed(1)}
                color="#2ca02c"
                hint="POLER ε-approx"
              />
              <MetricBox
                label="Макс. плотность"
                value={result.stats.maxDensity.toFixed(1)}
                color="#d62728"
                hint="самый насыщенный фрагмент"
              />
            </div>

            {result.stats.totalMoments === 0 ? (
              <div className="m-6 text-center text-stone-500">
                <Lucide.SearchX className="w-10 h-10 mx-auto mb-2 text-stone-300" />
                <p className="text-sm">
                  Совпадений не найдено. Возможно, имя персонажа в тексте
                  написано иначе, чем в графе.
                </p>
                <p className="text-xs text-stone-400 mt-1">
                  Попробуйте отредактировать узел и добавить алиасы через
                  meta.forms.
                </p>
              </div>
            ) : (
              <div className="flex-1 flex overflow-hidden">
                {/* Список глав (sidebar) */}
                <div className="w-48 border-r overflow-y-auto lit-scroll shrink-0">
                  <div className="text-[10px] uppercase tracking-wider text-stone-400 p-2 pb-1">
                    Главы ({result.byChapter.length})
                  </div>
                  {result.byChapter.map((c) => {
                    const key = `${c.chapter.num}-${c.chapter.suffix}`;
                    const isActive = key === activeChapterKey;
                    return (
                      <button
                        key={key}
                        onClick={() => setActiveChapterKey(key)}
                        className={`w-full text-left px-2 py-1.5 text-xs transition-colors border-l-2 ${
                          isActive
                            ? "bg-violet-50 border-violet-500 text-violet-900 font-medium"
                            : "border-transparent text-stone-600 hover:bg-stone-50"
                        }`}
                      >
                        <div className="flex items-baseline justify-between">
                          <span className="truncate">{c.chapter.title}</span>
                          <span
                            className={`text-[10px] ml-1 shrink-0 ${
                              isActive ? "text-violet-600" : "text-stone-400"
                            }`}
                          >
                            {c.moments.length}
                          </span>
                        </div>
                      </button>
                    );
                  })}
                </div>

                {/* Фрагменты активной главы */}
                <div className="flex-1 overflow-y-auto lit-scroll p-3 space-y-2.5">
                  {!activeChapter ? (
                    <div className="text-xs text-stone-400 italic">
                      Выберите главу слева
                    </div>
                  ) : (
                    activeChapter.moments.map((m, idx) => {
                      const segments = highlightKeywords(m.text, keywords);
                      const maxDensity = result.stats.maxDensity || 1;
                      const densityPct = (m.density / maxDensity) * 100;
                      return (
                        <div
                          key={`${m.chapter.num}-${idx}`}
                          className="rounded-md border border-stone-200 bg-white p-2.5 hover:border-violet-300 transition-colors"
                        >
                          <div className="flex items-center gap-2 mb-1.5">
                            <span className="text-[10px] font-mono text-stone-400">
                              #{idx + 1}
                            </span>
                            <span className="text-[10px] px-1.5 py-0.5 rounded-full bg-stone-100 text-stone-600">
                              pos: {m.position.toLocaleString()}
                            </span>
                            {m.matchedKeyword && (
                              <Badge
                                variant="secondary"
                                className="text-[10px] py-0 px-1.5 bg-violet-50 text-violet-700 border-violet-200"
                              >
                                «{m.matchedKeyword}»
                              </Badge>
                            )}
                            {m.keywordCount > 1 && (
                              <Badge
                                variant="secondary"
                                className="text-[10px] py-0 px-1.5 bg-emerald-50 text-emerald-700 border-emerald-200"
                              >
                                +{m.keywordCount - 1} совпадений в окне
                              </Badge>
                            )}
                            {/* Полоса плотности */}
                            <div className="ml-auto flex items-center gap-1">
                              <div
                                className="w-16 h-1.5 rounded-full bg-stone-100 overflow-hidden"
                                title={`POLER ε ≈ ${m.density.toFixed(2)}`}
                              >
                                <div
                                  className="h-full bg-gradient-to-r from-violet-400 to-violet-700"
                                  style={{ width: `${densityPct}%` }}
                                />
                              </div>
                              <span className="text-[9px] text-stone-500 font-mono w-8 text-right">
                                {m.density.toFixed(1)}
                              </span>
                            </div>
                          </div>
                          <p className="text-xs text-stone-700 leading-relaxed">
                            {segments.map((seg, i) =>
                              seg.isMatch ? (
                                <mark
                                  key={i}
                                  className="bg-violet-200 text-violet-900 px-0.5 rounded font-medium"
                                >
                                  {seg.text}
                                </mark>
                              ) : (
                                <span key={i}>{seg.text}</span>
                              )
                            )}
                          </p>
                        </div>
                      );
                    })
                  )}
                </div>
              </div>
            )}
          </>
        )}

        {!error && !result && loading && (
          <div className="m-12 text-center text-stone-500">
            <Lucide.Loader2 className="w-8 h-8 mx-auto mb-2 animate-spin text-violet-500" />
            <p className="text-sm">Сканирование текста…</p>
            <p className="text-xs text-stone-400 mt-1">
              {sourceMarkdown.length.toLocaleString()} символов ·{" "}
              {keywords.length} ключевых слов
            </p>
          </div>
        )}

        <DialogFooter className="border-t pt-3">
          <Button variant="outline" onClick={onClose}>
            Закрыть
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function MetricBox({
  label,
  value,
  color,
  hint,
}: {
  label: string;
  value: string;
  color: string;
  hint?: string;
}) {
  return (
    <div className="rounded-md border p-2 bg-white">
      <div className="text-[10px] text-stone-500">{label}</div>
      <div className="text-lg font-bold" style={{ color }}>
        {value}
      </div>
      {hint && <div className="text-[9px] text-stone-400">{hint}</div>}
    </div>
  );
}
