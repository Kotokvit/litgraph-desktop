"use client";

import { useState, useMemo } from "react";
import * as Lucide from "lucide-react";
import { NodePalette } from "./NodePalette";
import { Inspector } from "./Inspector";
import { useLitStore } from "@/lib/litgraph/store";
import { EDGE_TYPES, NODE_TYPES, NODE_TYPE_ORDER } from "@/lib/litgraph/types";

type Tab = "palette" | "inspector" | "legend";

export function Sidebar() {
  const [tab, setTab] = useState<Tab>("palette");
  const selectedNodeId = useLitStore((s) => s.selectedNodeId);
  const selectedEdgeId = useLitStore((s) => s.selectedEdgeId);
  const nodes = useLitStore((s) => s.nodes);
  const hideTag = useLitStore((s) => s.hideTag);
  const setHideTag = useLitStore((s) => s.setHideTag);

  // Считаем теги через useMemo, чтобы не пересоздавать массив каждый рендер
  const allTags = useMemo(() => {
    const tags = new Set<string>();
    nodes.forEach((n) => n.data.tags?.forEach((t) => tags.add(t)));
    return Array.from(tags).sort();
  }, [nodes]);

  const hasSelection = selectedNodeId || selectedEdgeId;

  return (
    <aside className="w-80 shrink-0 bg-white border-l border-stone-200 flex flex-col">
      {/* Табы */}
      <div className="flex border-b border-stone-200 bg-stone-50">
        <TabBtn
          active={tab === "palette"}
          onClick={() => setTab("palette")}
          icon={<Lucide.Plus className="w-3.5 h-3.5" />}
          label="Ноды"
        />
        <TabBtn
          active={tab === "inspector"}
          onClick={() => setTab("inspector")}
          icon={<Lucide.Info className="w-3.5 h-3.5" />}
          label="Инспектор"
          dot={!!hasSelection}
        />
        <TabBtn
          active={tab === "legend"}
          onClick={() => setTab("legend")}
          icon={<Lucide.HelpCircle className="w-3.5 h-3.5" />}
          label="Легенда"
        />
      </div>

      <div className="flex-1 overflow-y-auto lit-scroll p-3">
        {tab === "palette" && (
          <div className="space-y-4">
            <NodePalette />

            {/* Фильтр по тегам */}
            {allTags.length > 0 && (
              <div className="pt-3 border-t">
                <div className="text-[10px] uppercase tracking-wider text-stone-400 mb-1.5">
                  Теги в проекте
                </div>
                <p className="text-[10px] text-stone-500 mb-2 leading-tight">
                  Клик — скрыть ноды с этим тегом.
                </p>
                <div className="flex flex-wrap gap-1">
                  {allTags.map((t) => (
                    <button
                      key={t}
                      onClick={() => setHideTag(hideTag === t ? null : t)}
                      className={`text-[10px] px-1.5 py-0.5 rounded-full transition-all ${
                        hideTag === t
                          ? "bg-stone-700 text-white line-through"
                          : "bg-stone-100 text-stone-600 hover:bg-stone-200"
                      }`}
                    >
                      #{t}
                    </button>
                  ))}
                </div>
                {hideTag && (
                  <button
                    onClick={() => setHideTag(null)}
                    className="mt-2 text-[10px] text-amber-700 hover:underline"
                  >
                    сбросить фильтр
                  </button>
                )}
              </div>
            )}
          </div>
        )}

        {tab === "inspector" && <Inspector />}

        {tab === "legend" && (
          <div className="space-y-4 text-xs">
            <div>
              <div className="text-[10px] uppercase tracking-wider text-stone-400 mb-2">
                Типы нод
              </div>
              <div className="space-y-1.5">
                {NODE_TYPE_ORDER.map((t) => {
                  const cfg = NODE_TYPES[t];
                  return (
                    <div key={t} className="flex items-start gap-2">
                      <div
                        className="w-3 h-3 rounded mt-0.5 shrink-0"
                        style={{ background: cfg.color }}
                      />
                      <div className="flex-1 min-w-0">
                        <div className="font-medium text-stone-700">
                          {cfg.label}
                        </div>
                        <div className="text-[10px] text-stone-500 leading-tight">
                          {cfg.description}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>

            <div className="pt-3 border-t">
              <div className="text-[10px] uppercase tracking-wider text-stone-400 mb-2">
                Типы связей
              </div>
              <div className="space-y-1.5">
                {Object.values(EDGE_TYPES).map((k) => (
                  <div key={k.kind} className="flex items-start gap-2">
                    <svg width="24" height="8" className="mt-1.5 shrink-0">
                      <line
                        x1="0"
                        y1="4"
                        x2="24"
                        y2="4"
                        stroke={k.color}
                        strokeWidth="2"
                        strokeDasharray={k.dashed ? "4 2" : undefined}
                      />
                    </svg>
                    <div className="flex-1 min-w-0">
                      <div className="font-medium text-stone-700">{k.label}</div>
                      <div className="text-[10px] text-stone-500 leading-tight">
                        {k.description}
                      </div>
                    </div>
                  </div>
                ))}
              </div>
            </div>

            <div className="pt-3 border-t">
              <div className="text-[10px] uppercase tracking-wider text-stone-400 mb-2">
                Горячие клавиши
              </div>
              <ul className="space-y-1 text-stone-600">
                <li className="flex justify-between">
                  <span>Удалить выбранное</span>
                  <kbd className="text-[10px] bg-stone-100 px-1 rounded">Del</kbd>
                </li>
                <li className="flex justify-between">
                  <span>Дублировать ноду</span>
                  <kbd className="text-[10px] bg-stone-100 px-1 rounded">Ctrl+D</kbd>
                </li>
                <li className="flex justify-between">
                  <span>Зум — колесо мыши</span>
                  <kbd className="text-[10px] bg-stone-100 px-1 rounded">Wheel</kbd>
                </li>
                <li className="flex justify-between">
                  <span>Перемещение холста</span>
                  <kbd className="text-[10px] bg-stone-100 px-1 rounded">ЛКМ+drag</kbd>
                </li>
                <li className="flex justify-between">
                  <span>Создать связь</span>
                  <kbd className="text-[10px] bg-stone-100 px-1 rounded">от кружка →</kbd>
                </li>
              </ul>
            </div>
          </div>
        )}
      </div>
    </aside>
  );
}

function TabBtn({
  active,
  onClick,
  icon,
  label,
  dot,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
  dot?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`flex-1 flex items-center justify-center gap-1.5 py-2.5 text-xs font-medium transition-colors relative ${
        active
          ? "text-stone-900 bg-white border-b-2 border-amber-600"
          : "text-stone-500 hover:text-stone-700 hover:bg-stone-100"
      }`}
    >
      {icon}
      {label}
      {dot && (
        <span className="absolute top-2 right-3 w-1.5 h-1.5 rounded-full bg-amber-500" />
      )}
    </button>
  );
}
