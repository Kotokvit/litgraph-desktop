"use client";

import * as Lucide from "lucide-react";
import { useState, useMemo } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Label } from "@/components/ui/label";
import { Input } from "@/components/ui/input";
import { analyzeText } from "@/lib/poler/analyze";

// Цвета кластеров (как в matplotlib tab10)
const CLUSTER_COLORS = [
  "#1f77b4", // синий
  "#ff7f0e", // оранжевый
  "#2ca02c", // зелёный
  "#d62728", // красный
  "#9467bd", // фиолетовый
  "#8c564b", // коричневый
  "#e377c2", // розовый
  "#7f7f7f", // серый
  "#bcbd22", // оливковый
  "#17becf", // бирюзовый
];

interface WordCluster {
  word: string;
  cluster: number;
  modeNorm: number;
  degree: number;
  modes: number[];
}

interface PolerResult {
  ok: boolean;
  clusters: WordCluster[];
  silhouette: number;
  eigenvalues: number[];
  nNodes: number;
  nEdges: number;
  gamma: number;
  kModes: number;
  iterations: number;
  converged: boolean;
  energyStart: number;
  energyFinal: number;
  truncated?: boolean;
}

interface PolerDialogProps {
  open: boolean;
  text: string;
  onClose: () => void;
}

export function PolerDialog({ open, text, onClose }: PolerDialogProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<PolerResult | null>(null);
  const [gamma, setGamma] = useState(0.05);
  const [kModes, setKModes] = useState(4);
  const [windowSize, setWindowSize] = useState(5);
  const [minFreq, setMinFreq] = useState(2);

  async function handleAnalyze() {
    if (!text.trim()) {
      setError("Нет текста для анализа. Импортируйте .md сначала.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      // POLER работает полностью client-side — без сервера, без Tauri invoke.
      // Это гарантирует одинаковый результат в веб-превью и в Tauri desktop.
      // Для больших текстов (>50k символов) рекомендуется Rust-порт (TODO).
      const truncatedText = text.length > 50000 ? text.slice(0, 50000) : text;
      const data = analyzeText(truncatedText, {
        gamma,
        kModes,
        windowSize,
        minFreq,
      });
      setResult({ ok: true, ...data, truncated: text.length > 50000 } as PolerResult);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  // Группировка по кластерам
  const clusterGroups = useMemo(() => {
    if (!result) return [];
    const groups: Record<number, WordCluster[]> = {};
    for (const c of result.clusters) {
      if (!groups[c.cluster]) groups[c.cluster] = [];
      groups[c.cluster].push(c);
    }
    // Сортируем кластеры по размеру (убывание)
    return Object.entries(groups)
      .map(([k, v]) => ({ cluster: parseInt(k), words: v }))
      .sort((a, b) => b.words.length - a.words.length);
  }, [result]);

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-4xl max-h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lucide.Network className="w-5 h-5 text-violet-600" />
            POLER-анализ: физика текста
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto lit-scroll space-y-3">
          {!result && (
            <>
              <p className="text-xs text-stone-500 leading-relaxed">
                POLER — детерминированный анализатор текста на основе теории графов.
                Строит граф совместной встречаемости слов, запускает диссипативную
                динамику <code className="text-violet-700">dp/dt = -η·Π_Λ·[L·p + γ·J·p - B·p/m]</code>,
                и кластеризует слова через собственные векторы оператора.
                <strong> Без ИИ, без LLM, без словарей стоп-слов</strong> — только математика.
              </p>

              <div className="grid grid-cols-2 gap-3">
                <div className="space-y-1.5">
                  <Label className="text-xs text-stone-500">
                    γ (вес резонанса): {gamma.toFixed(3)}
                  </Label>
                  <Input
                    type="range"
                    min={0}
                    max={0.5}
                    step={0.01}
                    value={gamma}
                    onChange={(e) => setGamma(parseFloat(e.target.value))}
                    className="h-8"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-stone-500">
                    k (число кластеров): {kModes}
                  </Label>
                  <Input
                    type="range"
                    min={2}
                    max={8}
                    step={1}
                    value={kModes}
                    onChange={(e) => setKModes(parseInt(e.target.value))}
                    className="h-8"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-stone-500">
                    Окно co-occurrence: {windowSize}
                  </Label>
                  <Input
                    type="range"
                    min={2}
                    max={10}
                    step={1}
                    value={windowSize}
                    onChange={(e) => setWindowSize(parseInt(e.target.value))}
                    className="h-8"
                  />
                </div>
                <div className="space-y-1.5">
                  <Label className="text-xs text-stone-500">
                    Мин. частота слова: {minFreq}
                  </Label>
                  <Input
                    type="range"
                    min={1}
                    max={5}
                    step={1}
                    value={minFreq}
                    onChange={(e) => setMinFreq(parseInt(e.target.value))}
                    className="h-8"
                  />
                </div>
              </div>

              <div className="text-[10px] text-stone-400">
                Текст: {text.length.toLocaleString()} символов ·{" "}
                {text.split(/\s+/).filter(Boolean).length.toLocaleString()} слов
                {text.length > 50000 && (
                  <span className="text-amber-600 ml-2">
                    ⚠ будет обрезан до 50k символов
                  </span>
                )}
              </div>
            </>
          )}

          {error && (
            <div className="rounded-md bg-red-50 border border-red-200 p-2.5 text-sm text-red-700">
              ❌ {error}
            </div>
          )}

          {result && (
            <div className="space-y-3">
              {/* Метрики */}
              <div className="grid grid-cols-4 gap-2">
                <MetricBox
                  label="Узлов"
                  value={result.nNodes.toString()}
                  color="#1f77b4"
                />
                <MetricBox
                  label="Рёбер"
                  value={result.nEdges.toString()}
                  color="#ff7f0e"
                />
                <MetricBox
                  label="Силуэт"
                  value={result.silhouette.toFixed(3)}
                  color="#2ca02c"
                  hint={
                    result.silhouette > 0.5
                      ? "хорошо разделены"
                      : result.silhouette > 0.3
                      ? "различимы"
                      : "слабо"
                  }
                />
                <MetricBox
                  label="Итераций"
                  value={result.iterations.toString()}
                  color="#d62728"
                  hint={result.converged ? "сошлось" : "не сошлось"}
                />
              </div>

              {/* Спектр */}
              <div className="rounded-md bg-stone-50 border border-stone-200 p-3">
                <div className="text-xs font-medium text-stone-600 mb-2">
                  Спектр POLER-оператора (наименьшие λ)
                </div>
                <div className="flex items-end gap-1 h-16">
                  {result.eigenvalues.map((lam, i) => {
                    const maxVal = Math.max(...result.eigenvalues, 0.1);
                    const h = Math.max(2, (lam / maxVal) * 60);
                    return (
                      <div
                        key={i}
                        className="flex-1 flex flex-col items-center"
                        title={`λ_${i + 1} = ${lam.toFixed(4)}`}
                      >
                        <div
                          className="w-full bg-violet-500 rounded-t"
                          style={{ height: `${h}px` }}
                        />
                        <div className="text-[9px] text-stone-500 mt-0.5">
                          {lam.toFixed(2)}
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Кластеры */}
              <div className="space-y-2">
                <div className="text-xs font-medium text-stone-600">
                  Кластеры слов ({clusterGroups.length})
                </div>
                {clusterGroups.map((group, idx) => {
                  const color =
                    CLUSTER_COLORS[idx % CLUSTER_COLORS.length];
                  return (
                    <div
                      key={group.cluster}
                      className="rounded-md border p-2"
                      style={{ borderColor: color + "40", background: color + "08" }}
                    >
                      <div className="flex items-center gap-2 mb-1.5">
                        <div
                          className="w-3 h-3 rounded-full"
                          style={{ background: color }}
                        />
                        <span className="text-xs font-medium">
                          Кластер {idx + 1} — {group.words.length} слов
                        </span>
                      </div>
                      <div className="flex flex-wrap gap-1">
                        {group.words.map((w) => (
                          <span
                            key={w.word}
                            className="text-xs px-1.5 py-0.5 rounded bg-white border"
                            style={{
                              borderColor: color + "30",
                              fontSize: `${Math.max(
                                9,
                                Math.min(14, 9 + w.modeNorm * 12)
                              )}px`,
                              fontWeight: w.modeNorm > 0.4 ? 600 : 400,
                            }}
                            title={`||p||=${w.modeNorm.toFixed(
                              3
                            )}, степень=${w.degree.toFixed(1)}`}
                          >
                            {w.word}
                          </span>
                        ))}
                      </div>
                    </div>
                  );
                })}
              </div>

              {result.truncated && (
                <div className="text-[10px] text-amber-600">
                  ⚠ Текст был обрезан до 50k символов. Для полного анализа
                  используйте Rust-версию.
                </div>
              )}
            </div>
          )}
        </div>

        <DialogFooter className="border-t pt-3">
          {result && (
            <Button
              variant="outline"
              onClick={() => setResult(null)}
              className="mr-auto"
            >
              <Lucide.RefreshCw className="w-4 h-4 mr-1.5" />
              Заново
            </Button>
          )}
          <Button variant="outline" onClick={onClose}>
            Закрыть
          </Button>
          {!result && (
            <Button onClick={handleAnalyze} disabled={loading}>
              {loading ? (
                <>
                  <Lucide.Loader2 className="w-4 h-4 mr-1.5 animate-spin" />
                  Анализ…
                </>
              ) : (
                <>
                  <Lucide.Network className="w-4 h-4 mr-1.5" />
                  Запустить POLER
                </>
              )}
            </Button>
          )}
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
