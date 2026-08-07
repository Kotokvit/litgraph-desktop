"use client";

import * as Lucide from "lucide-react";
import { useState, useMemo } from "react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { analyzeCharacters, type CharacterAnalysisResult } from "@/lib/poler/nerBridge";

// Цвета кластеров (как в matplotlib tab10)
const CLUSTER_COLORS = [
  "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728",
  "#9467bd", "#8c564b", "#e377c2", "#7f7f7f",
];

interface CharacterGraphDialogProps {
  open: boolean;
  text: string;
  onClose: () => void;
}

export function CharacterGraphDialog({ open, text, onClose }: CharacterGraphDialogProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<CharacterAnalysisResult | null>(null);

  async function handleAnalyze() {
    if (!text.trim()) {
      setError("Нет текста для анализа. Импортируйте .md сначала.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await analyzeCharacters(text);
      setResult(data);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  // Сортировка рёбер по весу
  const topEdges = useMemo(() => {
    if (!result?.graph?.edges) return [];
    return [...result.graph.edges].sort((a, b) => b.weight - a.weight).slice(0, 30);
  }, [result]);

  // Цвет персонажа по кластеру
  const charColor = useMemo(() => {
    const map: Record<string, string> = {};
    if (!result?.poler?.clusters) return map;
    result.poler.clusters.forEach((c, i) => {
      const color = CLUSTER_COLORS[i % CLUSTER_COLORS.length];
      c.characters.forEach((ch) => { map[ch] = color; });
    });
    return map;
  }, [result]);

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-4xl max-h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lucide.Share2 className="w-5 h-5 text-violet-600" />
            Граф персонажей: POLER-физика (полный текст)
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto lit-scroll space-y-3">
          {!result && (
            <>
              <p className="text-xs text-stone-500 leading-relaxed">
                <strong>POLER на персонажах</strong> — двухслойный анализ:
                NLP извлекает атомы (сущности) → POLER собирает их в молекулы смысла.
              </p>
              <ol className="text-xs text-stone-600 space-y-1 ml-4 list-decimal">
                <li>NER (spaCy + pymorphy3) извлекает персонажей с объединением падежей</li>
                <li>Строится граф co-occurrence (кто с кем в одних сценах)</li>
                <li>POLER-оператор <code className="text-violet-700">H = Π_Λ(L + γJ - B/m)Π_Λ</code></li>
                <li>K-means кластеризация в пространстве собственных векторов</li>
              </ol>

              <div className="rounded-md bg-emerald-50 border border-emerald-200 p-3 text-xs text-emerald-800">
                <strong>✓ Полный текст</strong> — обрабатываются ВСЕ символы (чанками по 50k),
                без обрезки. Это Rust/Python версия, работает в Tauri desktop.
              </div>

              <div className="rounded-md bg-amber-50 border border-amber-200 p-3 text-xs text-amber-800">
                <strong>⚠ Требования:</strong>
                <ul className="mt-1 ml-4 list-disc space-y-0.5">
                  <li>Python 3 в venv <code>~/.litgraph-venv</code></li>
                  <li><code>pip install spacy pymorphy3 numpy scipy scikit-learn</code></li>
                  <li><code>python -m spacy download ru_core_news_sm</code></li>
                </ul>
              </div>

              <div className="text-[10px] text-stone-400">
                Текст: {text.length.toLocaleString()} символов ·{" "}
                {text.split(/\s+/).filter(Boolean).length.toLocaleString()} слов
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
                  label="Персонажей"
                  value={result.graph.nNodes.toString()}
                  color="#aa6633"
                />
                <MetricBox
                  label="Связей"
                  value={result.graph.nEdges.toString()}
                  color="#3366aa"
                />
                <MetricBox
                  label="Силуэт"
                  value={result.poler.silhouette.toFixed(3)}
                  color="#2ca02c"
                  hint={
                    result.poler.silhouette > 0.5 ? "отлично" :
                    result.poler.silhouette > 0.3 ? "хорошо" : "слабо"
                  }
                />
                <MetricBox
                  label="Кластеров"
                  value={result.poler.clusters.length.toString()}
                  color="#d62728"
                />
              </div>

              {/* Спектр */}
              {result.poler.eigenvalues.length > 0 && (
                <div className="rounded-md bg-stone-50 border border-stone-200 p-3">
                  <div className="text-xs font-medium text-stone-600 mb-2">
                    Спектр POLER-оператора (наименьшие λ)
                  </div>
                  <div className="flex items-end gap-1 h-16">
                    {result.poler.eigenvalues.map((lam, i) => {
                      const maxVal = Math.max(...result.poler.eigenvalues, 0.1);
                      const h = Math.max(2, (lam / maxVal) * 60);
                      return (
                        <div key={i} className="flex-1 flex flex-col items-center" title={`λ_${i + 1} = ${lam.toFixed(4)}`}>
                          <div className="w-full bg-violet-500 rounded-t" style={{ height: `${h}px` }} />
                          <div className="text-[9px] text-stone-500 mt-0.5">{lam.toFixed(2)}</div>
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}

              {/* Кластеры персонажей */}
              <div className="space-y-2">
                <div className="text-xs font-medium text-stone-600">
                  Кластеры персонажей ({result.poler.clusters.length})
                </div>
                {result.poler.clusters.map((cluster, idx) => {
                  const color = CLUSTER_COLORS[idx % CLUSTER_COLORS.length];
                  return (
                    <div
                      key={idx}
                      className="rounded-md border p-3"
                      style={{ borderColor: color + "40", background: color + "08" }}
                    >
                      <div className="flex items-center gap-2 mb-2">
                        <div className="w-3 h-3 rounded-full" style={{ background: color }} />
                        <span className="text-sm font-medium">
                          Кластер {idx + 1} — {cluster.size} персонажей
                        </span>
                        <span className="text-[10px] text-stone-500 ml-auto">
                          ⟨степень⟩ = {cluster.avgDegree.toFixed(1)}
                        </span>
                      </div>
                      <div className="flex flex-wrap gap-1.5">
                        {cluster.characters.map((ch) => (
                          <span
                            key={ch}
                            className="text-sm px-2 py-1 rounded-full font-medium"
                            style={{
                              background: color + "20",
                              color: color,
                              border: `1px solid ${color}40`,
                            }}
                          >
                            {ch}
                          </span>
                        ))}
                      </div>
                    </div>
                  );
                })}
              </div>

              {/* Топ связей */}
              {topEdges.length > 0 && (
                <div className="space-y-2">
                  <div className="text-xs font-medium text-stone-600">
                    Топ-{topEdges.length} связей (кто с кем взаимодействует)
                  </div>
                  <div className="space-y-1">
                    {topEdges.map((edge, i) => (
                      <div
                        key={i}
                        className="flex items-center gap-2 text-xs px-2 py-1.5 rounded border bg-white"
                      >
                        <span className="text-stone-400 w-6">#{i + 1}</span>
                        <span
                          className="font-medium"
                          style={{ color: charColor[edge.source] || "#666" }}
                        >
                          {edge.source}
                        </span>
                        <Lucide.ArrowRight className="w-3 h-3 text-stone-400" />
                        <span
                          className="font-medium"
                          style={{ color: charColor[edge.target] || "#666" }}
                        >
                          {edge.target}
                        </span>
                        <div className="ml-auto flex items-center gap-2">
                          <div className="w-24 h-2 bg-stone-100 rounded-full overflow-hidden">
                            <div
                              className="h-full bg-violet-500"
                              style={{
                                width: `${Math.min(100, (edge.weight / topEdges[0].weight) * 100)}%`,
                              }}
                            />
                          </div>
                          <span className="text-stone-600 w-12 text-right">
                            {edge.weight.toFixed(2)}
                          </span>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Все персонажи */}
              {result.graph.nodes.length > 0 && (
                <div className="space-y-2">
                  <div className="text-xs font-medium text-stone-600">
                    Все персонажи ({result.graph.nodes.length})
                  </div>
                  <div className="flex flex-wrap gap-1">
                    {result.graph.nodes.map((ch) => (
                      <Badge
                        key={ch}
                        variant="outline"
                        className="text-xs"
                        style={{
                          borderColor: (charColor[ch] || "#666") + "60",
                          color: charColor[ch] || "#666",
                        }}
                      >
                        {ch}
                      </Badge>
                    ))}
                  </div>
                </div>
              )}

              {result.error && (
                <div className="rounded-md bg-amber-50 border border-amber-200 p-2.5 text-xs text-amber-700">
                  ⚠ {result.error}
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
                  Анализ (может занять 30-60 сек)…
                </>
              ) : (
                <>
                  <Lucide.Share2 className="w-4 h-4 mr-1.5" />
                  Запустить POLER на персонажах
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
  label, value, color, hint,
}: { label: string; value: string; color: string; hint?: string }) {
  return (
    <div className="rounded-md border p-2 bg-white">
      <div className="text-[10px] text-stone-500">{label}</div>
      <div className="text-lg font-bold" style={{ color }}>{value}</div>
      {hint && <div className="text-[9px] text-stone-400">{hint}</div>}
    </div>
  );
}
