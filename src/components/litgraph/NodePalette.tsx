"use client";

import * as Lucide from "lucide-react";
import { NODE_TYPES, NODE_TYPE_ORDER } from "@/lib/litgraph/types";
import { useLitStore } from "@/lib/litgraph/store";
import { useReactFlow } from "@xyflow/react";

const PALETTE_DESCRIPTION =
  "Перетащите тип ноды на холст — или кликните, чтобы добавить в центр видимой области.";

export function NodePalette() {
  const addNode = useLitStore((s) => s.addNode);
  const rf = useReactFlow();

  function handleAdd(type: keyof typeof NODE_TYPES) {
    // По возможности ставим ноду в центр видимой области
    try {
      const rfEl = document.querySelector(".react-flow") as HTMLElement | null;
      if (rfEl) {
        const rect = rfEl.getBoundingClientRect();
        const pos = rf.screenToFlowPosition({
          x: rect.left + rect.width / 2,
          y: rect.top + rect.height / 2,
        });
        addNode(type, { x: pos.x - 130, y: pos.y - 60 });
        return;
      }
    } catch {
      // ignore
    }
    addNode(type);
  }

  return (
    <div className="flex flex-col gap-3">
      <div className="text-[11px] text-stone-500 leading-relaxed">
        {PALETTE_DESCRIPTION}
      </div>
      <div className="grid grid-cols-2 gap-2">
        {NODE_TYPE_ORDER.map((type) => {
          const cfg = NODE_TYPES[type];
          // @ts-expect-error dynamic icon
          const Icon = Lucide[cfg.icon] as Lucide.LucideIcon | undefined;
          const Ico = Icon ?? Lucide.Square;
          return (
            <button
              key={type}
              onClick={() => handleAdd(type)}
              className="group flex flex-col gap-1.5 rounded-lg border border-stone-200 bg-white p-2.5 text-left transition-all hover:shadow-md hover:-translate-y-0.5"
              style={{ borderTopColor: cfg.color, borderTopWidth: 3 }}
              title={cfg.description}
            >
              <div className="flex items-center gap-2">
                <div
                  className="flex items-center justify-center w-6 h-6 rounded-md shrink-0"
                  style={{ background: cfg.color, color: "#fff" }}
                >
                  <Ico className="w-3.5 h-3.5" />
                </div>
                <span className="text-xs font-semibold text-stone-800">
                  {cfg.singular}
                </span>
              </div>
              <p className="text-[10px] text-stone-500 leading-tight line-clamp-2">
                {cfg.description}
              </p>
            </button>
          );
        })}
      </div>
    </div>
  );
}
