"use client";

import { memo } from "react";
import { Handle, Position, NodeProps, type Node } from "@xyflow/react";
import * as Lucide from "lucide-react";
import { NODE_TYPES, type LitNodeType } from "@/lib/litgraph/types";

export type LitFlowNode = Node<{
  title: string;
  body: string;
  type: LitNodeType;
  tags: string[];
  meta?: Record<string, unknown>;
  fullText?: string;
  dimmed?: boolean;
}>;

function LitNodeView({ data, selected }: NodeProps<LitFlowNode>) {
  const cfg = NODE_TYPES[data.type];
  // @ts-expect-error: dynamic icon
  const Icon = Lucide[cfg.icon] as Lucide.LucideIcon | undefined;
  const Ico = Icon ?? Lucide.Square;

  const dimmed = data.dimmed === true;

  const bodyPreview =
    (data.body || "").length > 110
      ? (data.body || "").slice(0, 110) + "…"
      : data.body || "";

  // Если есть полный текст — индикатор
  const hasFullText = !!data.fullText && data.fullText.length > 0;
  const wordCount = hasFullText ? data.fullText!.split(/\s+/).length : 0;

  return (
    <div
      className={`lit-node-enter relative rounded-xl bg-white shadow-md transition-all ${
        selected
          ? "ring-2 ring-offset-1"
          : "hover:shadow-lg"
      }`}
      style={{
        width: 260,
        borderLeft: `4px solid ${cfg.color}`,
        // @ts-expect-error css var
        "--tw-ring-color": cfg.color,
        boxShadow: selected
          ? `0 0 0 2px ${cfg.color}40`
          : undefined,
        opacity: dimmed ? 0.15 : 1,
        filter: dimmed ? "grayscale(100%)" : undefined,
        pointerEvents: dimmed ? "none" : undefined,
      }}
    >
      {/* Входной хендл (слева) */}
      <Handle
        type="target"
        position={Position.Left}
        style={{ color: cfg.color }}
        className="react-flow__handle"
      />

      {/* Шапка ноды */}
      <div
        className="flex items-center gap-2 px-3 py-2 rounded-t-[11px]"
        style={{ background: `${cfg.color}18` }}
      >
        <div
          className="flex items-center justify-center w-6 h-6 rounded-md shrink-0"
          style={{ background: cfg.color, color: "#fff" }}
        >
          <Ico className="w-3.5 h-3.5" />
        </div>
        <span
          className="text-[10px] font-semibold uppercase tracking-wider"
          style={{ color: cfg.color }}
        >
          {cfg.singular}
        </span>
        {hasFullText && (
          <span
            className="ml-auto text-[9px] px-1.5 py-0.5 rounded-full"
            style={{ background: `${cfg.color}25`, color: cfg.color }}
            title={`Полный текст: ${wordCount} слов`}
          >
            {wordCount} сл.
          </span>
        )}
      </div>

      {/* Заголовок */}
      <div className="px-3 pt-2 pb-1">
        <div className="text-sm font-semibold text-stone-800 leading-snug line-clamp-2">
          {data.title || "Без названия"}
        </div>
      </div>

      {/* Тело */}
      {bodyPreview && (
        <div className="px-3 pb-2">
          <p className="text-xs text-stone-500 leading-relaxed line-clamp-3 whitespace-pre-wrap">
            {bodyPreview}
          </p>
        </div>
      )}

      {/* Теги */}
      {data.tags && data.tags.length > 0 && (
        <div className="px-3 pb-2 flex flex-wrap gap-1">
          {data.tags.slice(0, 4).map((t) => (
            <span
              key={t}
              className="text-[10px] px-1.5 py-0.5 rounded-full"
              style={{ background: `${cfg.color}15`, color: cfg.color }}
            >
              #{t}
            </span>
          ))}
          {data.tags.length > 4 && (
            <span className="text-[10px] text-stone-400">
              +{data.tags.length - 4}
            </span>
          )}
        </div>
      )}

      {/* Выходной хендл (справа) */}
      <Handle
        type="source"
        position={Position.Right}
        style={{ color: cfg.color }}
        className="react-flow__handle"
      />
    </div>
  );
}

export default memo(LitNodeView);
