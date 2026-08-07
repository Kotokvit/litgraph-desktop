"use client";

import { memo } from "react";
import { BaseEdge, EdgeLabelRenderer, getBezierPath, type EdgeProps } from "@xyflow/react";
import { EDGE_TYPES } from "@/lib/litgraph/types";
import type { EdgeKind } from "@/lib/litgraph/types";

export interface LitEdgeData {
  kind?: EdgeKind;
  note?: string;
  dimmed?: boolean;
  [k: string]: unknown;
}

function LitEdgeViewComponent({
  id,
  sourceX,
  sourceY,
  targetX,
  targetY,
  sourcePosition,
  targetPosition,
  data,
  selected,
  markerEnd,
}: EdgeProps & { data?: LitEdgeData }) {
  const kind = data?.kind ?? "flow";
  const cfg = EDGE_TYPES[kind];
  const dimmed = data?.dimmed === true;

  const [edgePath, labelX, labelY] = getBezierPath({
    sourceX,
    sourceY,
    sourcePosition,
    targetX,
    targetY,
    targetPosition,
  });

  const baseOpacity = dimmed ? 0.15 : selected ? 1 : 0.85;

  return (
    <>
      <BaseEdge
        id={id}
        path={edgePath}
        markerEnd={markerEnd}
        style={{
          stroke: cfg.color,
          strokeWidth: selected ? 3 : 2,
          strokeDasharray: cfg.dashed ? "6 4" : undefined,
          opacity: baseOpacity,
        }}
      />
      {!dimmed && (
        <EdgeLabelRenderer>
          <div
            style={{
              position: "absolute",
              transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`,
              pointerEvents: "none",
              background: "#fff",
              border: `1px solid ${cfg.color}40`,
              color: cfg.color,
              fontSize: 10,
              padding: "1px 6px",
              borderRadius: 9999,
              opacity: baseOpacity,
              whiteSpace: "nowrap",
              fontWeight: 500,
            }}
            className="lit-edge-label nodrag nopan"
          >
            {cfg.label}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}

function areEqual(
  prev: EdgeProps & { data?: LitEdgeData },
  next: EdgeProps & { data?: LitEdgeData },
) {
  return (
    prev.selected === next.selected &&
    prev.sourceX === next.sourceX &&
    prev.sourceY === next.sourceY &&
    prev.targetX === next.targetX &&
    prev.targetY === next.targetY &&
    prev.data?.kind === next.data?.kind &&
    prev.data?.dimmed === next.data?.dimmed
  );
}

export default memo(LitEdgeViewComponent, areEqual);
