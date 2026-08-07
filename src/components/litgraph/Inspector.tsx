"use client";

import * as Lucide from "lucide-react";
import { useLitStore } from "@/lib/litgraph/store";
import { NODE_TYPES, EDGE_TYPES } from "@/lib/litgraph/types";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
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
  const Icon = Lucide[cfg.icon] as Lucide.LucideIcon | undefined;
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

  if (selectedNodeId && node) {
    return <NodeInspector key={node.id} node={node} />;
  }
  if (selectedEdgeId && edge) {
    return <EdgeInspector key={edge.id} edge={edge} />;
  }

  // Ничего не выбрано — подсказки
  return (
    <div className="space-y-3 text-xs text-stone-500">
      <div className="rounded-lg bg-stone-100 p-3">
        <div className="font-medium text-stone-700 mb-1">Подсказки</div>
        <ul className="space-y-1 list-disc list-inside leading-relaxed">
          <li>Кликните по ноде — увидеть детали справа.</li>
          <li>Двойной клик по ноде — открыть редактор.</li>
          <li>Тяните от правого кружка к левому — создать связь.</li>
          <li>Кликните по связи — поменять тип или удалить.</li>
          <li>Del — удалить выбранное. Ctrl+D — дублировать.</li>
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
