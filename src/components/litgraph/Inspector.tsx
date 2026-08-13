"use client";

import * as Lucide from "lucide-react";
import { useLitStore } from "@/lib/litgraph/store";
import { NODE_TYPES, EDGE_TYPES } from "@/lib/litgraph/types";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import type { LitNode, LitEdge } from "@/lib/litgraph/types";
import { useState } from "react";

// ====== DYNAMIS (Sprint 1) helpers ======
// S1-D добавляет в store параллельно slice `svoTriplets` (SvoTriplet[]).
// Чтобы Inspector компилировался и до, и после интеграции S1-D, объявляем
// локальный интерфейс и читаем селектор defensive (fallback на []).
interface SvoTriplet {
  subject: string;       // кто действует
  verb: string;          // что делает
  object: string;        // на ком/чём
  confidence?: number;   // 0..1
  caseValid?: boolean;   // прошла ли case-validation
  sentence?: string;     // исходное предложение
}

const ARCHETYPES = [
  "—", "Hero", "Shadow", "Mentor", "Trickster", "Anima/Animus", "Wise Old Man/Woman",
  "Threshold Guardian", "Herald", "Shapeshifter", "Fool", "Creator", "Destroyer",
];

const EMOTIONAL_VECTORS = [
  "—", "Радость", "Страх", "Гнев", "Печаль", "Отвращение", "Удивление", "Доверие",
  "Любопытство", "Вина", "Тревога", "Восторг", "Безразличие",
];

// ε (epsilon) — радиальный прогресс-бар 0..100.
// Цвет: зелёный (<40) → жёлтый (40-70) → красный (>70).
function DynamisEpsilon({ epsilon }: { epsilon: number | undefined }) {
  if (epsilon === undefined || epsilon === null || Number.isNaN(epsilon)) {
    return (
      <div className="text-[10px] text-stone-400 italic">
        ε (энергия) — не вычислена. Запустите Reasoning.
      </div>
    );
  }
  const v = Math.max(0, Math.min(100, epsilon));
  const radius = 18;
  const circumference = 2 * Math.PI * radius;
  const offset = circumference * (1 - v / 100);
  const color = v < 40 ? "#10B981" : v < 70 ? "#F59E0B" : "#EF4444";
  return (
    <div className="flex items-center gap-3">
      <svg width="44" height="44" viewBox="0 0 44 44" className="shrink-0">
        <circle cx="22" cy="22" r={radius} fill="none" stroke="#E7E5E4" strokeWidth="4" />
        <circle
          cx="22" cy="22" r={radius} fill="none" stroke={color} strokeWidth="4"
          strokeDasharray={circumference} strokeDashoffset={offset}
          strokeLinecap="round" transform="rotate(-90 22 22)"
        />
        <text x="22" y="26" textAnchor="middle" fontSize="10" fontWeight="700" fill={color}>
          {Math.round(v)}
        </text>
      </svg>
      <div className="flex-1">
        <div className="text-[10px] uppercase tracking-wider text-stone-500">ε — энергия/значимость</div>
        <div className="text-[10px] text-stone-400">
          Низкая (&lt;40) · Средняя (40-70) · Высокая (&gt;70)
        </div>
      </div>
    </div>
  );
}

// SVO-история: две колонки (Субъект / Объект), не больше 12 строк в каждой.
function DynamisSvoHistory({
  asSubject,
  asObject,
}: {
  asSubject: Array<{ verb: string; object: string; confidence?: number }>;
  asObject: Array<{ verb: string; subject: string; confidence?: number }>;
}) {
  if (asSubject.length === 0 && asObject.length === 0) {
    return (
      <div className="text-[10px] text-stone-400 italic">
        SVO-история пуста. Запустите Reasoning для анализа.
      </div>
    );
  }
  return (
    <div className="grid grid-cols-2 gap-2">
      <div>
        <div className="text-[9px] uppercase tracking-wider text-stone-500 mb-1">
          Субъект ({asSubject.length})
        </div>
        <div className="space-y-0.5 max-h-24 overflow-y-auto lit-scroll">
          {asSubject.slice(0, 12).map((t, i) => (
            <div key={i} className="text-[10px] text-stone-600 leading-tight">
              <span className="text-stone-400">→</span>{" "}
              <span className="font-medium">{t.verb}</span>{" "}
              <span className="text-stone-500">{t.object}</span>
              {t.confidence !== undefined && t.confidence < 0.6 && (
                <span className="ml-1 text-amber-600" title="низкая уверенность">⚠</span>
              )}
            </div>
          ))}
          {asSubject.length > 12 && (
            <div className="text-[9px] text-stone-400">+{asSubject.length - 12} ещё…</div>
          )}
        </div>
      </div>
      <div>
        <div className="text-[9px] uppercase tracking-wider text-stone-500 mb-1">
          Объект ({asObject.length})
        </div>
        <div className="space-y-0.5 max-h-24 overflow-y-auto lit-scroll">
          {asObject.slice(0, 12).map((t, i) => (
            <div key={i} className="text-[10px] text-stone-600 leading-tight">
              <span className="text-stone-400">←</span>{" "}
              <span className="font-medium">{t.verb}</span>{" "}
              <span className="text-stone-500">{t.subject}</span>
              {t.confidence !== undefined && t.confidence < 0.6 && (
                <span className="ml-1 text-amber-600" title="низкая уверенность">⚠</span>
              )}
            </div>
          ))}
          {asObject.length > 12 && (
            <div className="text-[9px] text-stone-400">+{asObject.length - 12} ещё…</div>
          )}
        </div>
      </div>
    </div>
  );
}

// Архетип + Эмоциональный вектор — два select'а, пишут в node.data.meta.
function DynamisArchetypeAndEmotional({
  archetype,
  emotionalVector,
  onChange,
}: {
  archetype: string | undefined;
  emotionalVector: string | undefined;
  onChange: (field: string, value: string) => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-2">
      <div className="space-y-1">
        <label className="text-[9px] uppercase tracking-wider text-stone-500">Архетип</label>
        <select
          value={archetype ?? "—"}
          onChange={(e) => onChange("archetype", e.target.value)}
          className="w-full h-7 rounded-md border border-stone-200 bg-white px-1.5 text-[11px]"
        >
          {ARCHETYPES.map((a) => <option key={a} value={a}>{a}</option>)}
        </select>
      </div>
      <div className="space-y-1">
        <label className="text-[9px] uppercase tracking-wider text-stone-500">Эмоция</label>
        <select
          value={emotionalVector ?? "—"}
          onChange={(e) => onChange("emotionalVector", e.target.value)}
          className="w-full h-7 rounded-md border border-stone-200 bg-white px-1.5 text-[11px]"
        >
          {EMOTIONAL_VECTORS.map((e) => <option key={e} value={e}>{e}</option>)}
        </select>
      </div>
    </div>
  );
}

// ====== Инспектор ноды (keyed by id) ======
function NodeInspector({ node, onFindInText }: { node: LitNode; onFindInText: (id: string) => void }) {
  const allNodes = useLitStore((s) => s.nodes);
  const allEdges = useLitStore((s) => s.edges);
  const setEditingNode = useLitStore((s) => s.setEditingNode);
  const deleteNode = useLitStore((s) => s.deleteNode);
  const duplicateNode = useLitStore((s) => s.duplicateNode);
  const sourceMarkdown = useLitStore((s) => s.sourceMarkdown);
  const updateNode = useLitStore((s) => s.updateNode);
  // S1-D добавляет slice `svoTriplets` параллельно; defensive fallback на []
  // чтобы Inspector компилировался и до, и после интеграции S1-D.
  const svoTriplets: SvoTriplet[] = useLitStore((s: any) => s.svoTriplets ?? []) ?? [];

  const cfg = NODE_TYPES[node.type];
  const Icon = (Lucide as any)[cfg.icon] as Lucide.LucideIcon | undefined;
  const Ico = Icon ?? Lucide.Square;

  // связи этой ноды
  const incoming = allEdges.filter((e) => e.target === node.id);
  const outgoing = allEdges.filter((e) => e.source === node.id);

  // DYNAMIS: SVO-история — фильтруем triplets по совпадению названия ноды
  // с subject (нода действует) или object (нода подвергается действию).
  const nodeTitle = node.data.title.toLowerCase();
  const asSubject = svoTriplets
    .filter((t) => t.subject.toLowerCase() === nodeTitle)
    .map((t) => ({ verb: t.verb, object: t.object, confidence: t.confidence }));
  const asObject = svoTriplets
    .filter((t) => t.object.toLowerCase() === nodeTitle)
    .map((t) => ({ verb: t.verb, subject: t.subject, confidence: t.confidence }));

  const nodeName = (id: string) =>
    allNodes.find((n) => n.id === id)?.data.title ?? "???";

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <div
            className="flex items-center justify-center w-7 h-7 rounded-md shrink-0"
            style={{ background: cfg.color, color: "#fff" }}
          >
            <Ico className="w-4 h-4" />
          </div>
          <span
            className="text-xs font-semibold uppercase tracking-wider"
            style={{ color: cfg.color }}
          >
            {cfg.singular}
          </span>
        </div>
        <h3 className="text-base font-semibold text-stone-800 leading-snug">
          {node.data.title || "Без названия"}
        </h3>
      </div>

      {/* Body preview */}
      {node.data.body && (
        <div>
          <div className="text-[10px] uppercase tracking-wider text-stone-400 mb-1">
            Содержание
          </div>
          <p className="text-xs text-stone-600 leading-relaxed whitespace-pre-wrap line-clamp-6">
            {node.data.body}
          </p>
        </div>
      )}

      {/* v0.5.0: Source Text Moments button — открывает диалог с фрагментами
          исходного markdown, где упоминается сущность узла. */}
      {sourceMarkdown.trim().length > 0 && (
        <Button
          size="sm"
          variant="outline"
          onClick={() => onFindInText(node.id)}
          className="w-full bg-violet-50 hover:bg-violet-100 border-violet-200 text-violet-700"
        >
          <Lucide.Search className="w-3.5 h-3.5 mr-1.5" />
          Найти в тексте
        </Button>
      )}

      {/* Tags */}
      {node.data.tags && node.data.tags.length > 0 && (
        <div>
          <div className="text-[10px] uppercase tracking-wider text-stone-400 mb-1">
            Теги
          </div>
          <div className="flex flex-wrap gap-1">
            {node.data.tags.map((t) => (
              <Badge
                key={t}
                variant="secondary"
                className="text-[10px]"
                style={{ background: `${cfg.color}18`, color: cfg.color }}
              >
                #{t}
              </Badge>
            ))}
          </div>
        </div>
      )}

      {/* Meta */}
      {node.data.meta && Object.keys(node.data.meta).length > 0 && (
        <div>
          <div className="text-[10px] uppercase tracking-wider text-stone-400 mb-1">
            Детали
          </div>
          <dl className="space-y-1">
            {Object.entries(node.data.meta)
              .filter(([, v]) => v !== undefined && v !== "")
              .map(([k, v]) => (
                <div key={k} className="flex justify-between gap-2 text-xs">
                  <dt className="text-stone-500">{k}:</dt>
                  <dd className="text-stone-700 font-medium text-right">{String(v)}</dd>
                </div>
              ))}
          </dl>
        </div>
      )}

      {/* === DYNAMIS (Sprint 1) ===
          Аналитический блок: ε (энергия/значимость), SVO-история по этой ноде,
          архетип и эмоциональный вектор. Все данные выводятся из Reasoning Engine
          (ε, SVO) или ручной разметки (archetype, emotionalVector). */}
      <div className="space-y-3 rounded-md border border-stone-200 bg-stone-50 p-3">
        <div className="flex items-center gap-1.5">
          <Lucide.Activity className="w-3 h-3 text-violet-600" />
          <span className="text-[10px] uppercase tracking-wider text-violet-700 font-semibold">
            DYNAMIS
          </span>
        </div>

        {/* ε (epsilon) — radial progress bar */}
        <DynamisEpsilon epsilon={node.data.meta?.epsilon as number | undefined} />

        {/* SVO-History — два столбца: субъект / объект */}
        <DynamisSvoHistory asSubject={asSubject} asObject={asObject} />

        {/* Archetype + Emotional Vector selects — пишут в node.data.meta */}
        <DynamisArchetypeAndEmotional
          archetype={node.data.meta?.archetype as string | undefined}
          emotionalVector={node.data.meta?.emotionalVector as string | undefined}
          onChange={(field, value) => {
            updateNode(node.id, {
              data: {
                ...node.data,
                meta: { ...(node.data.meta ?? {}), [field]: value },
              },
            });
          }}
        />
      </div>

      {/* Connections */}
      {(incoming.length > 0 || outgoing.length > 0) && (
        <div>
          <div className="text-[10px] uppercase tracking-wider text-stone-400 mb-1">
            Связи ({incoming.length + outgoing.length})
          </div>
          <div className="space-y-1 text-xs max-h-32 overflow-y-auto lit-scroll">
            {incoming.map((e) => {
              const k = EDGE_TYPES[e.data?.kind ?? "flow"];
              return (
                <div key={e.id} className="flex items-center gap-1 text-stone-600">
                  <span className="text-stone-400">←</span>
                  <span className="truncate">{nodeName(e.source)}</span>
                  <span
                    className="ml-auto text-[9px] px-1 rounded shrink-0"
                    style={{ background: `${k.color}20`, color: k.color }}
                  >
                    {k.label}
                  </span>
                </div>
              );
            })}
            {outgoing.map((e) => {
              const k = EDGE_TYPES[e.data?.kind ?? "flow"];
              return (
                <div key={e.id} className="flex items-center gap-1 text-stone-600">
                  <span className="text-stone-400">→</span>
                  <span className="truncate">{nodeName(e.target)}</span>
                  <span
                    className="ml-auto text-[9px] px-1 rounded shrink-0"
                    style={{ background: `${k.color}20`, color: k.color }}
                  >
                    {k.label}
                  </span>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* Actions */}
      <div className="flex flex-col gap-2 pt-2 border-t">
        <Button
          size="sm"
          onClick={() => setEditingNode(node.id)}
          className="w-full"
        >
          <Lucide.Pencil className="w-3.5 h-3.5 mr-1.5" />
          Редактировать
        </Button>
        <div className="grid grid-cols-2 gap-2">
          <Button
            size="sm"
            variant="outline"
            onClick={() => duplicateNode(node.id)}
          >
            <Lucide.Copy className="w-3.5 h-3.5 mr-1.5" />
            Копия
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => {
              if (confirm("Удалить эту ноду? Все её связи тоже будут удалены.")) {
                deleteNode(node.id);
              }
            }}
            className="text-red-600 hover:text-red-700 hover:bg-red-50"
          >
            <Lucide.Trash2 className="w-3.5 h-3.5 mr-1.5" />
            Удалить
          </Button>
        </div>
      </div>
    </div>
  );
}

// ====== Инспектор ребра (keyed by id) ======
function EdgeInspector({ edge }: { edge: LitEdge }) {
  const allNodes = useLitStore((s) => s.nodes);
  const deleteEdge = useLitStore((s) => s.deleteEdge);
  const updateEdge = useLitStore((s) => s.updateEdge);

  const cfg = EDGE_TYPES[edge.data?.kind ?? "flow"];
  const src = allNodes.find((n) => n.id === edge.source);
  const tgt = allNodes.find((n) => n.id === edge.target);

  // Локальный стейт инициализируется один раз при монтировании (key=edge.id снаружи)
  const [note, setNote] = useState(edge?.data?.note ?? "");

  return (
    <div className="space-y-4">
      <div className="space-y-2">
        <div className="flex items-center gap-2">
          <div
            className="w-3 h-3 rounded-full"
            style={{ background: cfg.color }}
          />
          <span
            className="text-xs font-semibold uppercase tracking-wider"
            style={{ color: cfg.color }}
          >
            {cfg.label}
          </span>
        </div>
        <p className="text-xs text-stone-500">{cfg.description}</p>
      </div>

      <div className="space-y-2">
        <div className="text-[10px] uppercase tracking-wider text-stone-400">
          Откуда → куда
        </div>
        <div className="space-y-1 text-xs">
          <div className="text-stone-700">
            <span className="text-stone-400">из:</span>{" "}
            {src?.data.title ?? "???"}
          </div>
          <div className="text-stone-700">
            <span className="text-stone-400">в:</span>{" "}
            {tgt?.data.title ?? "???"}
          </div>
        </div>
      </div>

      <div className="space-y-1.5">
        <label className="text-xs text-stone-500">Заметка к связи</label>
        <Textarea
          value={note}
          onChange={(e) => setNote(e.target.value)}
          onBlur={() =>
            updateEdge(edge.id, {
              data: { ...edge.data, note },
            })
          }
          placeholder="зачем эта связь? что она значит для сюжета?"
          className="min-h-[60px] text-sm"
        />
      </div>

      <div className="space-y-1.5">
        <label className="text-xs text-stone-500">Тип связи</label>
        <select
          value={edge.data?.kind ?? "flow"}
          onChange={(e) => {
            const kind = e.target.value as keyof typeof EDGE_TYPES;
            const k = EDGE_TYPES[kind];
            updateEdge(edge.id, {
              data: { ...edge.data, kind },
              animated: k.animated,
            });
          }}
          className="w-full h-9 rounded-md border border-stone-200 bg-white px-2 text-sm"
        >
          {Object.values(EDGE_TYPES).map((k) => (
            <option key={k.kind} value={k.kind}>
              {k.label}
            </option>
          ))}
        </select>
      </div>

      <div className="pt-2 border-t">
        <Button
          size="sm"
          variant="outline"
          onClick={() => deleteEdge(edge.id)}
          className="w-full text-red-600 hover:text-red-700 hover:bg-red-50"
        >
          <Lucide.Trash2 className="w-3.5 h-3.5 mr-1.5" />
          Удалить связь
        </Button>
      </div>
    </div>
  );
}

// ====== Инспектор фонового слоя ======
function BackgroundInspector() {
  const bg = useLitStore((s) => s.backgroundLayer);
  const updateBackgroundLayer = useLitStore((s) => s.updateBackgroundLayer);
  const clearBackgroundLayer = useLitStore((s) => s.clearBackgroundLayer);
  const toggleBackgroundVisibility = useLitStore((s) => s.toggleBackgroundVisibility);

  if (!bg) return null;

  // Размеры отмасштабированного изображения в мировых координатах
  const dw = Math.round(bg.naturalWidth * bg.scale);
  const dh = Math.round(bg.naturalHeight * bg.scale);

  return (
    <div className="space-y-3">
      {/* Header */}
      <div className="space-y-1.5">
        <div className="flex items-center gap-2">
          <div className="flex items-center justify-center w-7 h-7 rounded-md bg-emerald-100 shrink-0">
            <Lucide.Image className="w-4 h-4 text-emerald-700" />
          </div>
          <span className="text-xs font-semibold uppercase tracking-wider text-emerald-700">
            Фоновый слой
          </span>
        </div>
        <h3 className="text-sm font-semibold text-stone-800 leading-snug truncate" title={bg.name}>
          {bg.name}
        </h3>
        <div className="flex items-center gap-1.5 text-[10px] text-stone-500">
          <Badge variant="secondary" className="text-[9px] uppercase">{bg.format}</Badge>
          <span>{bg.naturalWidth}×{bg.naturalHeight}px</span>
          <span className="text-stone-400">·</span>
          <span>{dw}×{dh} в холсте</span>
        </div>
      </div>

      {/* Видимость */}
      <div className="flex items-center gap-2">
        <Button
          size="sm"
          variant={bg.visible ? "default" : "outline"}
          onClick={toggleBackgroundVisibility}
          className="h-7 text-xs flex-1"
          style={bg.visible ? { background: "#059669" } : undefined}
        >
          {bg.visible ? (
            <>
              <Lucide.Eye className="w-3.5 h-3.5 mr-1.5" />Видим
            </>
          ) : (
            <>
              <Lucide.EyeOff className="w-3.5 h-3.5 mr-1.5" />Скрыт
            </>
          )}
        </Button>
        <Button
          size="sm"
          variant={bg.locked ? "default" : "outline"}
          onClick={() => updateBackgroundLayer({ locked: !bg.locked })}
          className="h-7 text-xs"
          title={bg.locked ? "Разблокировать перемещение" : "Зафиксировать позицию"}
          style={bg.locked ? { background: "#92400E" } : undefined}
        >
          {bg.locked ? <Lucide.Lock className="w-3.5 h-3.5" /> : <Lucide.Unlock className="w-3.5 h-3.5" />}
        </Button>
      </div>

      {/* Непрозрачность */}
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <Label className="text-[10px] uppercase tracking-wider text-stone-400">
            Непрозрачность
          </Label>
          <span className="text-xs text-stone-600 font-mono">
            {Math.round(bg.opacity * 100)}%
          </span>
        </div>
        <input
          type="range"
          min={0}
          max={1}
          step={0.05}
          value={bg.opacity}
          onChange={(e) => updateBackgroundLayer({ opacity: parseFloat(e.target.value) })}
          className="w-full accent-emerald-600"
        />
      </div>

      {/* Масштаб */}
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <Label className="text-[10px] uppercase tracking-wider text-stone-400">
            Масштаб
          </Label>
          <span className="text-xs text-stone-600 font-mono">
            {(bg.scale * 100).toFixed(0)}%
          </span>
        </div>
        <input
          type="range"
          min={0.05}
          max={4}
          step={0.05}
          value={bg.scale}
          onChange={(e) => updateBackgroundLayer({ scale: parseFloat(e.target.value) })}
          className="w-full accent-emerald-600"
        />
        <div className="flex gap-1">
          <Button
            size="sm"
            variant="outline"
            onClick={() => updateBackgroundLayer({ scale: 0.5 })}
            className="h-6 text-[10px] flex-1"
          >50%</Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => updateBackgroundLayer({ scale: 1 })}
            className="h-6 text-[10px] flex-1"
          >100%</Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => updateBackgroundLayer({ scale: 2 })}
            className="h-6 text-[10px] flex-1"
          >200%</Button>
        </div>
      </div>

      {/* Поворот */}
      <div className="space-y-1">
        <div className="flex items-center justify-between">
          <Label className="text-[10px] uppercase tracking-wider text-stone-400">
            Поворот
          </Label>
          <span className="text-xs text-stone-600 font-mono">
            {Math.round(bg.rotation)}°
          </span>
        </div>
        <input
          type="range"
          min={-180}
          max={180}
          step={1}
          value={bg.rotation}
          onChange={(e) => updateBackgroundLayer({ rotation: parseFloat(e.target.value) })}
          className="w-full accent-emerald-600"
        />
        <Button
          size="sm"
          variant="outline"
          onClick={() => updateBackgroundLayer({ rotation: 0 })}
          className="h-6 text-[10px] w-full"
        >Сбросить поворот</Button>
      </div>

      {/* Позиция */}
      <div className="space-y-1.5">
        <Label className="text-[10px] uppercase tracking-wider text-stone-400">
          Позиция в холсте
        </Label>
        <div className="grid grid-cols-2 gap-2">
          <div>
            <div className="text-[9px] text-stone-400 mb-0.5">X</div>
            <input
              type="number"
              value={Math.round(bg.x)}
              onChange={(e) => updateBackgroundLayer({ x: parseFloat(e.target.value) || 0 })}
              className="w-full h-7 rounded-md border border-stone-200 px-2 text-xs"
            />
          </div>
          <div>
            <div className="text-[9px] text-stone-400 mb-0.5">Y</div>
            <input
              type="number"
              value={Math.round(bg.y)}
              onChange={(e) => updateBackgroundLayer({ y: parseFloat(e.target.value) || 0 })}
              className="w-full h-7 rounded-md border border-stone-200 px-2 text-xs"
            />
          </div>
        </div>
      </div>

      {/* Центрировать в viewport */}
      <Button
        size="sm"
        variant="outline"
        onClick={() => {
          // Центрируем в (0, 0) мировых координат
          const newScale = bg.scale;
          updateBackgroundLayer({
            x: -(bg.naturalWidth * newScale) / 2,
            y: -(bg.naturalHeight * newScale) / 2,
          });
        }}
        className="h-7 text-xs w-full"
      >
        <Lucide.Crosshair className="w-3.5 h-3.5 mr-1.5" />
        Центрировать в начале координат
      </Button>

      {/* Удалить */}
      <div className="pt-2 border-t">
        <Button
          size="sm"
          variant="outline"
          onClick={() => {
            if (confirm(`Удалить фоновый слой "${bg.name}"?`)) {
              clearBackgroundLayer();
            }
          }}
          className="w-full text-red-600 hover:text-red-700 hover:bg-red-50"
        >
          <Lucide.Trash2 className="w-3.5 h-3.5 mr-1.5" />
          Удалить фоновый слой
        </Button>
      </div>

      {/* Подсказка */}
      <div className="rounded-md bg-emerald-50 border border-emerald-200 p-2 text-[10px] text-emerald-800 leading-relaxed">
        <Lucide.Lightbulb className="w-3 h-3 inline mr-1" />
        Перетаскивайте фон мышью прямо по холсту (если не залочен).
        SVG масштабируется без потерь качества; для TIFF/PNG используйте
        высокий scale для чтения деталей.
      </div>
    </div>
  );
}

// ====== Главный инспектор: выбирает что показать ======
export function Inspector({ onFindInText }: { onFindInText?: (id: string) => void }) {
  const selectedNodeId = useLitStore((s) => s.selectedNodeId);
  const selectedEdgeId = useLitStore((s) => s.selectedEdgeId);
  const node = useLitStore((s) =>
    s.selectedNodeId ? s.nodes.find((n) => n.id === s.selectedNodeId) ?? null : null
  );
  const edge = useLitStore((s) =>
    s.selectedEdgeId ? s.edges.find((e) => e.id === s.selectedEdgeId) ?? null : null
  );
  const backgroundLayer = useLitStore((s) => s.backgroundLayer);

  const handleFindInText = (id: string) => {
    onFindInText?.(id);
  };

  if (selectedNodeId && node) {
    return <NodeInspector key={node.id} node={node} onFindInText={handleFindInText} />;
  }
  if (selectedEdgeId && edge) {
    return <EdgeInspector key={edge.id} edge={edge} />;
  }

  // Ничего не выбрано — подсказки + секция фона (если есть)
  return (
    <div className="space-y-3 text-xs text-stone-500">
      {backgroundLayer && <BackgroundInspector />}
      <div className="rounded-lg bg-stone-100 p-3">
        <div className="font-medium text-stone-700 mb-1">Подсказки</div>
        <ul className="space-y-1 list-disc list-inside leading-relaxed">
          <li>Кликните по ноде — увидеть детали справа.</li>
          <li>Двойной клик по ноде — открыть редактор.</li>
          <li>Тяните от правого кружка к левому — создать связь.</li>
          <li>Кликните по связи — поменять тип или удалить.</li>
          <li>Del — удалить выбранное. Ctrl+D — дублировать.</li>
          <li>Перетаскивайте фоновый слой (если есть) мышью по холсту.</li>
        </ul>
      </div>
      <div className="rounded-lg bg-amber-50 border border-amber-200 p-3">
        <div className="font-medium text-amber-800 mb-1">Проект сохраняется</div>
        <p className="leading-relaxed text-amber-700">
          Все изменения автоматически сохраняются в браузере. Можно закрыть
          вкладку — при следующем открытии граф восстановится.
        </p>
      </div>
    </div>
  );
}
