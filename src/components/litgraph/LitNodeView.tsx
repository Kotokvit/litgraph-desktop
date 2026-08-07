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

function LitNodeViewComponent({ data, selected }: NodeProps<LitFlowNode>) {
  const cfg = NODE_TYPES[data.type];
  const Icon = (Lucide as any)[cfg.icon] as Lucide.LucideIcon | undefined;
  const Ico = Icon ?? Lucide.Square;

  const dimmed = data.dimmed === true;

  const body = data.body || "";
  const bodyPreview = body.length > 110 ? body.slice(0, 110) + "…" : body;

  const fullText = data.fullText || "";
  const hasFullText = fullText.length > 0;
  const wordCount = hasFullText ? fullText.split(/\s+/).length : 0;

  const tags = data.tags || [];
  const visibleTags = tags.slice(0, 4);
  const extraTagsCount = tags.length > 4 ? tags.length - 4 : 0;

  return (
    <div
      className={`lit-node-enter relative rounded-xl bg-white shadow-md ${
        selected ? "ring-2 ring-offset-1" : "hover:shadow-lg"
      }`}
      style={{
        width: 260,
        borderLeft: `4px solid ${cfg.color}`,
        boxShadow: selected ? `0 0 0 2px ${cfg.color}40` : undefined,
        opacity: dimmed ? 0.15 : 1,
        filter: dimmed ? "grayscale(100%)" : undefined,
        pointerEvents: dimmed ? "none" : undefined,
      }}
    >
      <Handle
        type="target"
        position={Position.Left}
        style={{ color: cfg.color }}
        className="react-flow__handle"
      />

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

      <div className="px-3 pt-2 pb-1">
        <div className="text-sm font-semibold text-stone-800 leading-snug line-clamp-2">
          {data.title || "Без названия"}
        </div>
      </div>

      {bodyPreview && (
        <div className="px-3 pb-2">
          <p className="text-xs text-stone-500 leading-relaxed line-clamp-3 whitespace-pre-wrap">
            {bodyPreview}
          </p>
        </div>
      )}

      {visibleTags.length > 0 && (
        <div className="px-3 pb-2 flex flex-wrap gap-1">
          {visibleTags.map((t) => (
            <span
              key={t}
              className="text-[10px] px-1.5 py-0.5 rounded-full"
              style={{ background: `${cfg.color}15`, color: cfg.color }}
            >
              #{t}
            </span>
          ))}
          {extraTagsCount > 0 && (
            <span className="text-[10px] text-stone-400">+{extraTagsCount}</span>
          )}
        </div>
      )}

      <Handle
        type="source"
        position={Position.Right}
        style={{ color: cfg.color }}
        className="react-flow__handle"
      />
    </div>
  );
}

// Кастомный comparator для memo — сравниваем только значимые поля
// Это критично: data объект пересоздаётся каждый рендер, но если
// значения не изменились — нода не ре-рендерится
function areEqual(
  prev: NodeProps<LitFlowNode>,
  next: NodeProps<LitFlowNode>,
) {
  return (
    prev.selected === next.selected &&
    prev.data?.title === next.data?.title &&
    prev.data?.body === next.data?.body &&
    prev.data?.dimmed === next.data?.dimmed &&
    prev.data?.fullText === next.data?.fullText &&
    prev.data?.tags === next.data?.tags
  );
}

export default memo(LitNodeViewComponent, areEqual);
