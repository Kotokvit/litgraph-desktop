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
import { getConflictGraph } from "@/lib/conflict/api";
import {
  type ConflictGraph,
  type ConflictNode,
  type ConflictEdge,
  CONFLICT_COLORS,
  ROLE_LABELS,
  POLARITY_LABELS,
} from "@/lib/conflict/types";

interface ConflictGraphDialogProps {
  open: boolean;
  text: string;
  onClose: () => void;
}

export function ConflictGraphDialog({ open, text, onClose }: ConflictGraphDialogProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ConflictGraph | null>(null);
  const [hoveredNode, setHoveredNode] = useState<string | null>(null);
  const [hoveredEdge, setHoveredEdge] = useState<number | null>(null);

  async function handleAnalyze() {
    if (!text.trim()) {
      setError("Нет текста для анализа. Импортируйте .md сначала.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const data = await getConflictGraph(text);
      setResult(data);
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  // Топ рёбер по |весу|
  const topEdges = useMemo(() => {
    if (!result) return [];
    return [...result.edges]
      .sort((a, b) => Math.abs(b.weight) - Math.abs(a.weight))
      .slice(0, 20);
  }, [result]);

  // Координаты узлов для SVG: круговая раскладка
  const nodePositions = useMemo(() => {
    if (!result) return {} as Record<string, { x: number; y: number }>;
    const nodes = result.nodes;
    const n = nodes.length;
    const cx = 280, cy = 220, r = 150;
    const positions: Record<string, { x: number; y: number }> = {};
    // Сортируем: агрессоры слева, жертвы справа, нейтралы сверху
    const sorted = [...nodes].sort((a, b) => {
      const ra = a.role === "aggressor" ? 0 : a.role === "victim" ? 2 : 1;
      const rb = b.role === "aggressor" ? 0 : b.role === "victim" ? 2 : 1;
      if (ra !== rb) return ra - rb;
      return Math.abs(b.balance) - Math.abs(a.balance);
    });
    sorted.forEach((node, i) => {
      // Полукруг слева-направо: агрессоры в верхней левой части, жертвы в правой
      const angle = (Math.PI * (i + 0.5)) / n; // 0..π
      positions[node.character] = {
        x: cx + r * Math.cos(Math.PI - angle) * 1.3, // зеркалим
        y: cy - r * Math.sin(angle) * 0.9,
      };
    });
    return positions;
  }, [result]);

  // Макс |balance| для масштабирования размера узла
  const maxAbsBalance = useMemo(() => {
    if (!result || result.nodes.length === 0) return 1;
    return Math.max(...result.nodes.map((n) => Math.abs(n.balance)), 1);
  }, [result]);

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-5xl max-h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lucide.Swords className="w-5 h-5 text-red-600" />
            Конфликт-граф: рентген агрессоров и жертв (SVO → J-матрица)
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto lit-scroll space-y-3">
          {!result && (
            <>
              <p className="text-xs text-stone-500 leading-relaxed">
                <strong>Конфликт-граф</strong> — направленный анализ: кто на кого
                воздействует в тексте. Строится через 3-этапный Python-пайплайн.
              </p>
              <ol className="text-xs text-stone-600 space-y-1 ml-4 list-decimal">
                <li>NER (spaCy + pymorphy3) извлекает персонажей</li>
                <li>SVO: dependency parsing находит триплеты «кто → что сделал → с кем»</li>
                <li>
                  J-матрица: <code className="text-red-700">J[i,j] = +w, J[j,i] = −w</code>{" "}
                  (антисимметричная),{" "}
                  <code className="text-red-700">net = Σ J[i,*]</code> определяет роль
                </li>
              </ol>

              <div className="rounded-md bg-red-50 border border-red-200 p-3 text-xs text-red-800">
                <strong>Что показывает «рентген»:</strong>
                <ul className="mt-1 ml-4 list-disc space-y-0.5">
                  <li>Красные узлы — агрессоры (net J &gt; 0)</li>
                  <li>Синие узлы — жертвы (net J &lt; 0)</li>
                  <li>Стрелки — направленные действия (толщина ~ вес)</li>
                  <li>Пунктир — negated («не остановил», вес ×0.3)</li>
                </ul>
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
                  value={String(result.stats.nodeCount)}
                  color="#aa6633"
                />
                <MetricBox
                  label="Направленных действий"
                  value={String(result.stats.edgeCount)}
                  color="#DC2626"
                />
                <MetricBox
                  label="Агрессоров"
                  value={String(result.stats.aggressors.length)}
                  color={CONFLICT_COLORS.aggressor}
                />
                <MetricBox
                  label="Жертв"
                  value={String(result.stats.victims.length)}
                  color={CONFLICT_COLORS.victim}
                />
              </div>

              {/* SVG направленный граф */}
              {result.nodes.length > 0 && (
                <div className="space-y-2">
                  <div className="text-xs font-medium text-stone-600 flex items-center justify-between">
                    <span>Направленный граф конфликта</span>
                    <span className="text-[10px] text-stone-400">
                      Наведи на узел или ребро для деталей · J-матрица v{result.svoVersion}
                    </span>
                  </div>
                  <div className="rounded-md border bg-white p-2">
                    <svg
                      viewBox="0 0 560 440"
                      className="w-full h-auto"
                      style={{ maxHeight: 440 }}
                    >
                      <defs>
                        {/* Стрелки разных цветов */}
                        {[
                          ["arrow-red", CONFLICT_COLORS.edgeAggression],
                          ["arrow-gray", CONFLICT_COLORS.edgeNeutral],
                          ["arrow-green", CONFLICT_COLORS.edgePositive],
                          ["arrow-dashed", CONFLICT_COLORS.edgeNegated],
                        ].map(([id, color]) => (
                          <marker
                            key={id}
                            id={id}
                            viewBox="0 0 10 10"
                            refX="9"
                            refY="5"
                            markerWidth="7"
                            markerHeight="7"
                            orient="auto-start-reverse"
                          >
                            <path d="M 0 0 L 10 5 L 0 10 z" fill={color} />
                          </marker>
                        ))}
                      </defs>

                      {/* Рёбра */}
                      {topEdges.map((edge, i) => {
                        const from = nodePositions[edge.from];
                        const to = nodePositions[edge.to];
                        if (!from || !to) return null;

                        // Контрольная точка для кривой Безье
                        const mx = (from.x + to.x) / 2;
                        const my = (from.y + to.y) / 2;
                        const dx = to.x - from.x;
                        const dy = to.y - from.y;
                        const len = Math.sqrt(dx * dx + dy * dy) || 1;
                        // Перпендикуляр для изгиба
                        const offset = 25;
                        const cx = mx + (-dy / len) * offset;
                        const cy = my + (dx / len) * offset;

                        // Укорачиваем концы, чтобы не залезать в круги
                        const fromR = 26 + Math.abs(result.nodes.find(n => n.character === edge.from)?.balance || 0) * 6;
                        const toR = 26 + Math.abs(result.nodes.find(n => n.character === edge.to)?.balance || 0) * 6;
                        const fromX = from.x + (dx / len) * fromR;
                        const fromY = from.y + (dy / len) * fromR;
                        const toX = to.x - (dx / len) * (toR + 8);
                        const toY = to.y - (dy / len) * (toR + 8);

                        const color = edge.negated
                          ? CONFLICT_COLORS.edgeNegated
                          : edge.polarity === "negative"
                          ? CONFLICT_COLORS.edgeAggression
                          : edge.polarity === "positive"
                          ? CONFLICT_COLORS.edgePositive
                          : CONFLICT_COLORS.edgeNeutral;

                        const strokeWidth = 1.5 + Math.min(Math.abs(edge.weight) * 0.8, 2.5);
                        const isHovered = hoveredEdge === i;
                        const isDimmed = hoveredEdge !== null && !isHovered;

                        // Позиция лейбла — середина кривой
                        const labelX = (fromX + 2 * cx + toX) / 4;
                        const labelY = (fromY + 2 * cy + toY) / 4;

                        return (
                          <g
                            key={i}
                            style={{ cursor: "pointer" }}
                            onMouseEnter={() => setHoveredEdge(i)}
                            onMouseLeave={() => setHoveredEdge(null)}
                            opacity={isDimmed ? 0.25 : 1}
                          >
                            <path
                              d={`M ${fromX} ${fromY} Q ${cx} ${cy} ${toX} ${toY}`}
                              fill="none"
                              stroke={color}
                              strokeWidth={isHovered ? strokeWidth + 1 : strokeWidth}
                              strokeDasharray={edge.negated ? "5 4" : undefined}
                              markerEnd={
                                edge.negated
                                  ? "url(#arrow-dashed)"
                                  : edge.polarity === "negative"
                                  ? "url(#arrow-red)"
                                  : edge.polarity === "positive"
                                  ? "url(#arrow-green)"
                                  : "url(#arrow-gray)"
                              }
                            />
                            {/* Лейбл глагола */}
                            {(isHovered || topEdges.length <= 8) && (
                              <g>
                                <rect
                                  x={labelX - (edge.verbs.join(", ").length * 3.3 + 6)}
                                  y={labelY - 9}
                                  width={edge.verbs.join(", ").length * 6.6 + 12}
                                  height={18}
                                  rx={3}
                                  fill="white"
                                  stroke={color}
                                  strokeWidth={0.8}
                                  opacity={0.95}
                                />
                                <text
                                  x={labelX}
                                  y={labelY + 4}
                                  textAnchor="middle"
                                  fontSize={10}
                                  fontWeight={600}
                                  fill={CONFLICT_COLORS.edgeNegated}
                                >
                                  {edge.verbs.join(", ").slice(0, 28)}
                                  {edge.verbs.join(", ").length > 28 ? "…" : ""}
                                </text>
                              </g>
                            )}
                          </g>
                        );
                      })}

                      {/* Узлы */}
                      {result.nodes.map((node) => {
                        const pos = nodePositions[node.character];
                        if (!pos) return null;
                        const fill =
                          node.role === "aggressor"
                            ? CONFLICT_COLORS.aggressorFill
                            : node.role === "victim"
                            ? CONFLICT_COLORS.victimFill
                            : CONFLICT_COLORS.neutralFill;
                        const stroke =
                          node.role === "aggressor"
                            ? CONFLICT_COLORS.aggressor
                            : node.role === "victim"
                            ? CONFLICT_COLORS.victim
                            : CONFLICT_COLORS.neutral;
                        const radius = 26 + (Math.abs(node.balance) / maxAbsBalance) * 14;
                        const isHovered = hoveredNode === node.character;
                        const isDimmed =
                          hoveredNode !== null && !isHovered &&
                          !topEdges.some(
                            (e) =>
                              (e.from === hoveredNode && e.to === node.character) ||
                              (e.to === hoveredNode && e.from === node.character),
                          );

                        return (
                          <g
                            key={node.character}
                            style={{ cursor: "pointer" }}
                            onMouseEnter={() => setHoveredNode(node.character)}
                            onMouseLeave={() => setHoveredNode(null)}
                            opacity={isDimmed ? 0.35 : 1}
                          >
                            <circle
                              cx={pos.x}
                              cy={pos.y}
                              r={radius}
                              fill={fill}
                              stroke={stroke}
                              strokeWidth={isHovered ? 3.5 : 2.5}
                            />
                            <text
                              x={pos.x}
                              y={pos.y + 4}
                              textAnchor="middle"
                              fontSize={12}
                              fontWeight={700}
                              fill={CONFLICT_COLORS.edgeNegated}
                            >
                              {node.character.length > 14
                                ? node.character.slice(0, 12) + "…"
                                : node.character}
                            </text>
                            {/* Net balance под узлом */}
                            <text
                              x={pos.x}
                              y={pos.y + radius + 14}
                              textAnchor="middle"
                              fontSize={11}
                              fontWeight={600}
                              fill={stroke}
                            >
                              {node.balance > 0 ? "+" : ""}
                              {node.balance.toFixed(1)}
                            </text>
                          </g>
                        );
                      })}
                    </svg>
                  </div>

                  {/* Tooltip / details panel */}
                  {hoveredNode && (
                    <div className="rounded-md bg-stone-50 border p-2.5 text-xs">
                      {(() => {
                        const n = result.nodes.find((x) => x.character === hoveredNode);
                        if (!n) return null;
                        const out = result.edges.filter((e) => e.from === hoveredNode);
                        const inc = result.edges.filter((e) => e.to === hoveredNode);
                        return (
                          <div className="space-y-1">
                            <div className="font-semibold">
                              {n.character}{" "}
                              <span
                                className="px-1.5 py-0.5 rounded text-[10px] text-white ml-1"
                                style={{
                                  backgroundColor:
                                    n.role === "aggressor"
                                      ? CONFLICT_COLORS.aggressor
                                      : n.role === "victim"
                                      ? CONFLICT_COLORS.victim
                                      : CONFLICT_COLORS.neutral,
                                }}
                              >
                                {ROLE_LABELS[n.role]}
                              </span>
                            </div>
                            <div className="text-stone-600">
                              out={n.outgoing.toFixed(1)} · in={n.incoming.toFixed(1)} ·{" "}
                              balance=<b>{n.balance > 0 ? "+" : ""}{n.balance.toFixed(2)}</b>
                            </div>
                            {out.length > 0 && (
                              <div>
                                <span className="text-stone-500">Действует на:</span>{" "}
                                {out.map((e, i) => (
                                  <span key={i} className="text-red-700">
                                    {e.to} ({e.verbs.join(", ")})
                                    {i < out.length - 1 ? ", " : ""}
                                  </span>
                                ))}
                              </div>
                            )}
                            {inc.length > 0 && (
                              <div>
                                <span className="text-stone-500">Подвергается от:</span>{" "}
                                {inc.map((e, i) => (
                                  <span key={i} className="text-blue-700">
                                    {e.from} ({e.verbs.join(", ")})
                                    {i < inc.length - 1 ? ", " : ""}
                                  </span>
                                ))}
                              </div>
                            )}
                          </div>
                        );
                      })()}
                    </div>
                  )}

                  {hoveredEdge !== null && topEdges[hoveredEdge] && (
                    <div className="rounded-md bg-stone-50 border p-2.5 text-xs">
                      <EdgeDetail edge={topEdges[hoveredEdge]} />
                    </div>
                  )}
                </div>
              )}

              {/* Асимметрия J: агрессоры vs жертвы */}
              {result.nodes.length > 0 && (
                <div className="space-y-2">
                  <div className="text-xs font-medium text-stone-600 flex items-center justify-between">
                    <span>Асимметрия J: агрессоры vs жертвы</span>
                    <span className="text-[10px] text-stone-400">
                      красный = агрессия, синий = подавление
                    </span>
                  </div>
                  <div className="space-y-1">
                    {result.nodes.map((n, i) => (
                      <BalanceBar key={i} node={n} maxAbs={maxAbsBalance} />
                    ))}
                  </div>
                </div>
              )}

              {/* Топ направленных действий */}
              {topEdges.length > 0 && (
                <div className="space-y-2">
                  <div className="text-xs font-medium text-stone-600">
                    Направленные действия (топ-{Math.min(15, topEdges.length)})
                  </div>
                  <div className="space-y-1">
                    {topEdges.slice(0, 15).map((edge, i) => (
                      <EdgeRow
                        key={i}
                        edge={edge}
                        index={i + 1}
                        onHover={() => setHoveredEdge(i)}
                        onLeave={() => setHoveredEdge(null)}
                      />
                    ))}
                  </div>
                </div>
              )}

              {result.stats.rawTripletCount === 0 && (
                <div className="rounded-md bg-amber-50 border border-amber-200 p-2.5 text-xs text-amber-700">
                  ⚠ SVO-триплеты не найдены. Возможно, в тексте нет явных
                  направленных действий между персонажами, или NER не распознал
                  имена. Попробуйте другой фрагмент.
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
                  Анализ конфликта (10-30 сек)…
                </>
              ) : (
                <>
                  <Lucide.Swords className="w-4 h-4 mr-1.5" />
                  Запустить рентген конфликта
                </>
              )}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ── Подкомпоненты ──────────────────────────────────────────────────────

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

function BalanceBar({ node, maxAbs }: { node: ConflictNode; maxAbs: number }) {
  const isAggressor = node.role === "aggressor";
  const isVictim = node.role === "victim";
  const color = isAggressor
    ? CONFLICT_COLORS.aggressor
    : isVictim
    ? CONFLICT_COLORS.victim
    : CONFLICT_COLORS.neutral;
  const label = isAggressor
    ? "агрессор"
    : isVictim
    ? "жертва"
    : "нейтрал";
  return (
    <div className="flex items-center gap-2 text-xs px-2 py-1.5 rounded border bg-white">
      <span className="font-medium w-32 truncate" style={{ color }}>
        {node.character}
      </span>
      <span className="text-[10px] w-16" style={{ color }}>
        {label}
      </span>
      <div className="flex-1 flex items-center gap-1">
        <span className="text-[10px] text-stone-500 w-12 text-right">
          out={node.outgoing.toFixed(1)}
        </span>
        <div className="flex-1 h-2 bg-stone-100 rounded-full overflow-hidden relative">
          <div className="absolute left-1/2 top-0 bottom-0 w-px bg-stone-300" />
          {node.balance > 0 && (
            <div
              className="absolute top-0 bottom-0 bg-red-400"
              style={{
                left: "50%",
                width: `${(node.balance / maxAbs) * 50}%`,
              }}
            />
          )}
          {node.balance < 0 && (
            <div
              className="absolute top-0 bottom-0 bg-blue-400"
              style={{
                right: "50%",
                width: `${(Math.abs(node.balance) / maxAbs) * 50}%`,
              }}
            />
          )}
        </div>
        <span className="text-[10px] text-stone-500 w-12">
          in={node.incoming.toFixed(1)}
        </span>
      </div>
      <span
        className="text-xs font-mono w-12 text-right"
        style={{ color }}
      >
        {node.balance > 0 ? "+" : ""}
        {node.balance.toFixed(1)}
      </span>
    </div>
  );
}

function EdgeRow({
  edge,
  index,
  onHover,
  onLeave,
}: {
  edge: ConflictEdge;
  index: number;
  onHover: () => void;
  onLeave: () => void;
}) {
  const color = edge.negated
    ? CONFLICT_COLORS.edgeNegated
    : edge.polarity === "negative"
    ? CONFLICT_COLORS.edgeAggression
    : edge.polarity === "positive"
    ? CONFLICT_COLORS.edgePositive
    : CONFLICT_COLORS.edgeNeutral;
  const weightPercent = Math.min(Math.abs(edge.weight) * 25, 100);
  return (
    <div
      className="flex items-center gap-2 text-xs px-2 py-1.5 rounded border bg-white hover:bg-stone-50"
      onMouseEnter={onHover}
      onMouseLeave={onLeave}
    >
      <span className="text-stone-400 w-6">#{index}</span>
      <span className="font-medium" style={{ color: CONFLICT_COLORS.aggressor }}>
        {edge.from}
      </span>
      <Lucide.ArrowRight
        className="w-3 h-3"
        style={{ color }}
      />
      <span className="font-medium" style={{ color: CONFLICT_COLORS.victim }}>
        {edge.to}
      </span>
      <span
        className="text-[10px] px-1.5 py-0.5 rounded text-white"
        style={{ backgroundColor: color }}
      >
        {edge.negated
          ? "negated"
          : POLARITY_LABELS[edge.polarity]}
      </span>
      <div className="flex-1 flex items-center gap-2">
        <div className="flex-1 h-1.5 bg-stone-100 rounded-full overflow-hidden">
          <div
            className="h-full rounded-full"
            style={{
              width: `${weightPercent}%`,
              backgroundColor: color,
            }}
          />
        </div>
        <span className="text-[10px] text-stone-500 italic truncate max-w-32">
          {edge.verbs.join(", ")}
        </span>
      </div>
      <span className="font-mono w-10 text-right" style={{ color }}>
        {edge.weight > 0 ? "+" : ""}
        {edge.weight.toFixed(1)}
      </span>
    </div>
  );
}

function EdgeDetail({ edge }: { edge: ConflictEdge }) {
  const color = edge.negated
    ? CONFLICT_COLORS.edgeNegated
    : edge.polarity === "negative"
    ? CONFLICT_COLORS.edgeAggression
    : edge.polarity === "positive"
    ? CONFLICT_COLORS.edgePositive
    : CONFLICT_COLORS.edgeNeutral;
  return (
    <div className="space-y-1">
      <div className="font-semibold">
        {edge.from} → {edge.to}{" "}
        <span
          className="px-1.5 py-0.5 rounded text-[10px] text-white ml-1"
          style={{ backgroundColor: color }}
        >
          {edge.negated ? "negated" : POLARITY_LABELS[edge.polarity]}
        </span>
      </div>
      <div className="text-stone-600">
        Вес: <b>{edge.weight > 0 ? "+" : ""}{edge.weight.toFixed(2)}</b> · глаголы:{" "}
        <i>{edge.verbs.join(", ")}</i> · {edge.verbCount}{" "}
        {edge.verbCount === 1 ? "действие" : "действий"}
        {edge.pronounResolved && " · pronoun resolved"}
      </div>
      {edge.sentence && (
        <div className="text-stone-500 italic">
          «{edge.sentence}
          {edge.sentence.length >= 200 ? "…" : ""}»
        </div>
      )}
    </div>
  );
}
