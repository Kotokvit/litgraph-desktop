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

// ====== Инспектор ноды (keyed by id) ======
function NodeInspector({ node }: { node: LitNode }) {
  const allNodes = useLitStore((s) => s.nodes);
  const allEdges = useLitStore((s) => s.edges);
  const setEditingNode = useLitStore((s) => s.setEditingNode);
  const deleteNode = useLitStore((s) => s.deleteNode);
  const duplicateNode = useLitStore((s) => s.duplicateNode);

  const cfg = NODE_TYPES[node.type];
  const Icon = (Lucide as any)[cfg.icon] as Lucide.LucideIcon | undefined;
  const Ico = Icon ?? Lucide.Square;

  // связи этой ноды
  const incoming = allEdges.filter((e) => e.target === node.id);
  const outgoing = allEdges.filter((e) => e.source === node.id);

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
export function Inspector() {
  const selectedNodeId = useLitStore((s) => s.selectedNodeId);
  const selectedEdgeId = useLitStore((s) => s.selectedEdgeId);
  const node = useLitStore((s) =>
    s.selectedNodeId ? s.nodes.find((n) => n.id === s.selectedNodeId) ?? null : null
  );
  const edge = useLitStore((s) =>
    s.selectedEdgeId ? s.edges.find((e) => e.id === s.selectedEdgeId) ?? null : null
  );
  const backgroundLayer = useLitStore((s) => s.backgroundLayer);

  if (selectedNodeId && node) {
    return <NodeInspector key={node.id} node={node} />;
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
