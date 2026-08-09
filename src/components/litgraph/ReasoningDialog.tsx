"use client";

import * as Lucide from "lucide-react";
import { Component, useState, useMemo, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { useLitStore } from "@/lib/litgraph/store";
import {
  reasoningExtractEvents,
  reasoningGetWorldState,
  reasoningRunCycle,
  type Event,
  type WorldStateView,
  type CycleReport,
  type Action,
  type FactValue,
} from "@/lib/tauri-commands";

interface ReasoningDialogProps {
  open: boolean;
  text: string;
  onClose: () => void;
}

// ============================================================================
// ErrorBoundary — ловит runtime-ошибки рендера, не роняя всё приложение.
// Без этого любой TypeError в дочерних компонентах превращает диалог в white screen.
// ============================================================================

interface ErrorBoundaryState {
  error: Error | null;
}

class ErrorBoundary extends Component<{ children: ReactNode }, ErrorBoundaryState> {
  state: ErrorBoundaryState = { error: null };

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error };
  }

  componentDidCatch(error: Error, info: { componentStack: string }) {
    console.error("[ReasoningDialog] render crash:", error, info);
  }

  render() {
    if (this.state.error) {
      return (
        <div className="rounded-md bg-red-50 border border-red-300 p-3 text-xs text-red-800">
          <div className="font-medium mb-1">⚠️ Render error in ReasoningDialog</div>
          <pre className="whitespace-pre-wrap break-words font-mono text-[10px] text-red-700">
            {this.state.error.message}
          </pre>
          <pre className="whitespace-pre-wrap break-words font-mono text-[9px] text-red-500 mt-2">
            {this.state.error.stack}
          </pre>
          <Button
            size="sm"
            variant="outline"
            className="mt-2 h-7 text-xs"
            onClick={() => this.setState({ error: null })}
          >
            Сбросить
          </Button>
        </div>
      );
    }
    return this.props.children;
  }
}

// ============================================================================
// Хелперы для рендера типов
// ============================================================================

function actionLabel(a: Action): string {
  if (typeof a === "string") return a;
  if (!a || typeof a !== "object") return String(a ?? "");
  const keys = Object.keys(a);
  if (keys.length === 0) return "?";
  const tag = keys[0];
  const inner = (a as Record<string, Record<string, unknown>>)[tag];
  if (inner && typeof inner === "object") {
    const innerKeys = Object.keys(inner);
    if (innerKeys.length > 0) {
      const firstVal = (inner as Record<string, unknown>)[innerKeys[0]];
      return `${tag}(${String(firstVal ?? "")})`;
    }
  }
  return tag;
}

function factValueLabel(v: FactValue | undefined | null): string {
  if (v == null) return "—";
  // Unit-вариант Unknown сериализуется как bare string "Unknown".
  if (typeof v === "string") return v.toLowerCase();
  // Остальные варианты — объекты { Tag: value }.
  if (typeof v !== "object") return String(v);
  if ("Bool" in v) return v.Bool ? "true" : "false";
  if ("Str" in v) return `"${v.Str}"`;
  if ("Int" in v) return String(v.Int);
  if ("Float" in v) return v.Float.toFixed(2);
  if ("Entity" in v) return `→${v.Entity}`;
  if ("List" in v) return `[${v.List.map(factValueLabel).join(", ")}]`;
  try {
    return JSON.stringify(v);
  } catch {
    return "?";
  }
}

function chapterLabel(chapterNum: number, suffix: string | null): string {
  return `Глава ${chapterNum ?? "?"}${suffix ?? ""}`;
}

// ============================================================================
// Под-компоненты
// ============================================================================

function MetricBox({
  label,
  value,
  color,
}: {
  label: string;
  value: string | number;
  color: string;
}) {
  return (
    <div className="rounded-md border bg-white p-2.5">
      <div className="text-[10px] text-stone-500 uppercase tracking-wide">
        {label}
      </div>
      <div className="text-lg font-bold" style={{ color }}>
        {value}
      </div>
    </div>
  );
}

function EventRow({ event }: { event: Event }) {
  const target = event.target ? ` → ${event.target}` : "";
  const time = chapterLabel(
    event.time?.chapterNum ?? 0,
    event.time?.chapterSuffix ?? null,
  );
  const sourceText = event.sourceText ?? "";
  const confidence = typeof event.confidence === "number" ? event.confidence : 0;
  let provenanceTag = "—";
  try {
    const p = event.provenance as unknown;
    // Все варианты Provenance — unit, поэтому на проводе bare string.
    if (typeof p === "string") {
      provenanceTag = p;
    } else if (p && typeof p === "object") {
      // Fallback: если Rust-сторона когда-либо добавит newtype/struct вариант.
      provenanceTag = Object.keys(p as Record<string, unknown>)[0] ?? "—";
    }
  } catch {
    /* ignore */
  }
  return (
    <div className="rounded-md border border-stone-200 bg-stone-50 p-2 text-xs">
      <div className="flex items-center justify-between gap-2">
        <span className="font-mono text-stone-900">
          <span className="text-purple-700">{event.actor}</span>
          <span className="text-stone-500">.</span>
          <span className="text-indigo-700">{actionLabel(event.action)}</span>
          <span className="text-stone-700">{target}</span>
        </span>
        <span className="text-[10px] text-stone-500 font-mono">{time}</span>
      </div>
      {sourceText && (
        <div className="mt-1 text-stone-600 italic truncate">
          «{sourceText.slice(0, 120)}
          {sourceText.length > 120 ? "…" : ""}»
        </div>
      )}
      <div className="mt-1 text-[10px] text-stone-400 flex gap-3">
        <span>conf={confidence.toFixed(2)}</span>
        <span>{provenanceTag}</span>
      </div>
    </div>
  );
}

function CharacterRow({
  id,
  title,
  isAlive,
  location,
  attributes,
}: {
  id: string;
  title: string;
  isAlive: boolean | null;
  location: string | null;
  attributes: Record<string, FactValue>;
}) {
  const aliveColor =
    isAlive === null
      ? "text-stone-400"
      : isAlive
        ? "text-emerald-700"
        : "text-red-700";
  const aliveIcon = isAlive === null ? "❓" : isAlive ? "💚" : "💀";
  const aliveLabel =
    isAlive === null ? "неизвестно" : isAlive ? "жив" : "мёртв";

  return (
    <div className="rounded-md border bg-white p-2.5 text-xs">
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium text-stone-900">
          {title}{" "}
          <span className="font-mono text-[10px] text-stone-400">#{id}</span>
        </span>
        <span className={`font-medium ${aliveColor}`}>
          {aliveIcon} {aliveLabel}
        </span>
      </div>
      {location && (
        <div className="mt-1 text-stone-600">
          📍 {location}
        </div>
      )}
      {attributes && Object.keys(attributes).length > 0 && (
        <div className="mt-2 grid grid-cols-2 gap-x-3 gap-y-0.5 text-[10px]">
          {Object.entries(attributes).map(([k, v]) => (
            <div key={k} className="flex justify-between font-mono">
              <span className="text-stone-500">{k}:</span>
              <span className="text-stone-800">{factValueLabel(v)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ============================================================================
// Внутренний компонент диалога (оборачивается в ErrorBoundary)
// ============================================================================

function ReasoningDialogInner({ open, text, onClose }: ReasoningDialogProps) {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [events, setEvents] = useState<Event[] | null>(null);
  const [worldView, setWorldView] = useState<WorldStateView | null>(null);
  const [report, setReport] = useState<CycleReport | null>(null);

  const exportProject = useLitStore((s) => s.exportProject);

  async function handleRunReasoning() {
    if (!text.trim()) {
      setError("Нет текста для анализа. Импортируйте .md сначала.");
      return;
    }
    setLoading(true);
    setError(null);
    setEvents(null);
    setWorldView(null);
    setReport(null);
    try {
      const project = exportProject();
      console.log("[reasoning] extract events for text length:", text.length);
      const extractedEvents = await reasoningExtractEvents(text, project);
      console.log("[reasoning] events extracted:", extractedEvents.length);
      setEvents(extractedEvents);

      const view = await reasoningGetWorldState(project, extractedEvents);
      console.log("[reasoning] world state:", view);
      setWorldView(view);

      const cycleReport = await reasoningRunCycle(project, extractedEvents);
      console.log("[reasoning] cycle report:", cycleReport);
      setReport(cycleReport);
    } catch (err) {
      console.error("[reasoning] pipeline error:", err);
      setError(String(err));
    } finally {
      setLoading(false);
    }
  }

  const sortedCharacters = useMemo(() => {
    if (!worldView?.characters) return [];
    return [...worldView.characters].sort((a, b) => {
      const ra = a.isAlive === false ? 1 : 0;
      const rb = b.isAlive === false ? 1 : 0;
      if (ra !== rb) return ra - rb;
      return (a.title ?? "").localeCompare(b.title ?? "");
    });
  }, [worldView]);

  const topEvents = useMemo(() => {
    if (!events) return [];
    return [...events]
      .sort((a, b) => (b.confidence ?? 0) - (a.confidence ?? 0))
      .slice(0, 30);
  }, [events]);

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-6xl max-h-[90vh] flex flex-col">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Lucide.Brain className="w-5 h-5 text-indigo-600" />
            Reasoning Engine — движок рассуждений (без LLM)
          </DialogTitle>
        </DialogHeader>

        <div className="flex-1 overflow-y-auto lit-scroll space-y-3">
          {!events && !loading && (
            <>
              <p className="text-xs text-stone-500 leading-relaxed">
                <strong>Reasoning Engine</strong> — это интеллектуальный слой
                LitGraph: алгоритм извлекает события из текста (без LLM),
                строит состояние мира, проверяет ограничения и находит
                противоречия. LLM используется только как «писатель» —
                мозгом является движок.
              </p>
              <ol className="text-xs text-stone-600 space-y-1 ml-4 list-decimal">
                <li>
                  <strong>SVO-парсер</strong> (regex на Rust) извлекает
                  события: «кто → что сделал → с кем → когда»
                </li>
                <li>
                  <strong>Inference</strong> применяет правила:{" "}
                  <code className="text-indigo-700">kill(X,Y) → Y.alive=false</code>
                </li>
                <li>
                  <strong>Constraints</strong> проверяют инварианты: «мёртвый
                  не может говорить», «узник не может перемещаться»
                </li>
                <li>
                  <strong>Contradiction Detector</strong> находит временные
                  парадоксы и причинные петли
                </li>
                <li>
                  <strong>Hypotheses</strong> генерирует 3 объяснения
                  (flashback / dream / text-error) и верифицирует их
                </li>
              </ol>

              <div className="rounded-md bg-indigo-50 border border-indigo-200 p-3 text-xs text-indigo-800">
                <strong>Что покажет «рентген»:</strong>
                <ul className="mt-1 ml-4 list-disc space-y-0.5">
                  <li>События — извлечённые из текста (фиолетовые)</li>
                  <li>Персонажи — с состоянием (💚 жив / 💀 мёртв)</li>
                  <li>
                    Парадоксы — красным (например, «Пётр мёртв с Г12, но
                    говорит в Г15»)
                  </li>
                  <li>
                    Нарушения — жёлтым (constraint violations)
                  </li>
                  <li>
                    Гипотезы — зелёным (принятые) / серым (отвергнутые)
                  </li>
                </ul>
              </div>

              <div className="rounded-md bg-emerald-50 border border-emerald-200 p-3 text-xs text-emerald-800">
                <strong>✓ Без Python. Без LLM. Чистый Rust.</strong>{" "}
                Движок работает на регулярках и правилах — мгновенно.
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

          {loading && (
            <div className="rounded-md bg-indigo-50 border border-indigo-200 p-4 text-sm text-indigo-700 flex items-center gap-2">
              <Lucide.Loader2 className="w-4 h-4 animate-spin" />
              Reasoning engine работает...
            </div>
          )}

          {worldView && report && (
            <div className="space-y-3">
              <div className="grid grid-cols-5 gap-2">
                <MetricBox label="Событий" value={report.eventsProcessed ?? 0} color="#7C3AED" />
                <MetricBox label="Фактов выведено" value={report.factsAsserted ?? 0} color="#0EA5E9" />
                <MetricBox label="Нарушений" value={(report.violations ?? []).length} color="#F59E0B" />
                <MetricBox label="Парадоксов" value={(report.temporalParadoxes ?? []).length} color="#DC2626" />
                <MetricBox
                  label="Гипотез"
                  value={`${report.hypothesesAccepted ?? 0}/${report.hypothesesGenerated ?? 0}`}
                  color="#10B981"
                />
              </div>

              {(report.temporalParadoxes ?? []).length > 0 && (
                <div className="space-y-1.5">
                  <div className="text-xs font-medium text-red-700 flex items-center gap-1.5">
                    <Lucide.AlertTriangle className="w-3.5 h-3.5" />
                    Временные парадоксы ({(report.temporalParadoxes ?? []).length})
                  </div>
                  {(report.temporalParadoxes ?? []).map((p, i) => (
                    <div
                      key={i}
                      className="rounded-md bg-red-50 border border-red-300 p-2 text-xs text-red-800"
                    >
                      <span className="font-mono text-[10px] text-red-500">
                        #{i + 1}
                      </span>{" "}
                      {p?.description ?? JSON.stringify(p)}
                    </div>
                  ))}
                </div>
              )}

              {(report.violations ?? []).length > 0 && (
                <div className="space-y-1.5">
                  <div className="text-xs font-medium text-amber-700 flex items-center gap-1.5">
                    <Lucide.AlertCircle className="w-3.5 h-3.5" />
                    Нарушения ограничений ({(report.violations ?? []).length})
                  </div>
                  {(report.violations ?? []).map((v, i) => (
                    <div
                      key={i}
                      className="rounded-md bg-amber-50 border border-amber-300 p-2 text-xs text-amber-800 font-mono"
                    >
                      <pre className="whitespace-pre-wrap break-words">
                        {(() => {
                          try { return JSON.stringify(v, null, 2); }
                          catch { return String(v); }
                        })()}
                      </pre>
                    </div>
                  ))}
                </div>
              )}

              {sortedCharacters.length > 0 && (
                <div className="space-y-1.5">
                  <div className="text-xs font-medium text-stone-700 flex items-center gap-1.5">
                    <Lucide.Users className="w-3.5 h-3.5" />
                    Состояние персонажей ({sortedCharacters.length})
                  </div>
                  <div className="grid grid-cols-2 gap-2">
                    {sortedCharacters.map((c) => (
                      <CharacterRow
                        key={c.id}
                        id={c.id}
                        title={c.title}
                        isAlive={c.isAlive}
                        location={c.location}
                        attributes={c.attributes}
                      />
                    ))}
                  </div>
                </div>
              )}

              {topEvents.length > 0 && (
                <div className="space-y-1.5">
                  <div className="text-xs font-medium text-purple-700 flex items-center gap-1.5">
                    <Lucide.Zap className="w-3.5 h-3.5" />
                    Извлечённые события ({events?.length ?? 0}, показано топ-{topEvents.length} по confidence)
                  </div>
                  <div className="space-y-1">
                    {topEvents.map((e, i) => (
                      <EventRow
                        key={`${e.id}-${e.actor}-${i}`}
                        event={e}
                      />
                    ))}
                  </div>
                </div>
              )}

              {events !== null && (events?.length ?? 0) === 0 && (
                <div className="rounded-md bg-stone-50 border border-stone-200 p-3 text-xs text-stone-600">
                  SVO-парсер не нашёл событий в тексте. Это нормально для
                  описательных текстов без действий. Попробуйте добавить
                  предложения с глаголами: «убил», «сказал», «пошёл».
                </div>
              )}

              {(report.violations ?? []).length === 0 &&
                (report.temporalParadoxes ?? []).length === 0 &&
                (report.eventsProcessed ?? 0) > 0 && (
                  <div className="rounded-md bg-emerald-50 border border-emerald-200 p-3 text-xs text-emerald-800">
                    ✓ Нарратив консистентен: ни нарушений, ни парадоксов.
                    Все {report.eventsProcessed} событий укладываются в
                    ограничения.
                  </div>
                )}
            </div>
          )}
        </div>

        <DialogFooter className="flex items-center justify-between gap-2 pt-2 border-t">
          <div className="text-[10px] text-stone-400">
            Reasoning Engine v0.1 · stateless pipeline · без LLM
          </div>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={onClose}>
              Закрыть
            </Button>
            <Button
              size="sm"
              onClick={handleRunReasoning}
              disabled={loading || !text.trim()}
              className="bg-indigo-600 hover:bg-indigo-700 text-white"
            >
              {loading ? (
                <>
                  <Lucide.Loader2 className="w-4 h-4 mr-1.5 animate-spin" />
                  Думаю...
                </>
              ) : (
                <>
                  <Lucide.Brain className="w-4 h-4 mr-1.5" />
                  Запустить reasoning
                </>
              )}
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ============================================================================
// Публичный экспорт: ErrorBoundary оборачивает внутренний компонент,
// чтобы рендер-ошибки в данных (например, неожиданный тип Action) не роняли
// весь LitApp. Теперь вместо white screen пользователь увидит красную карточку
// с текстом ошибки и кнопкой «Сбросить».
// ============================================================================

export function ReasoningDialog(props: ReasoningDialogProps) {
  return (
    <ErrorBoundary>
      <ReasoningDialogInner {...props} />
    </ErrorBoundary>
  );
}
