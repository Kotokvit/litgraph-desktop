"use client";

import { useCallback, useMemo, useEffect } from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  Background,
  BackgroundVariant,
  Controls,
  MiniMap,
  type NodeMouseHandler,
  type EdgeMouseHandler,
  type OnNodesChange,
  type OnEdgesChange,
  type Connection,
  MarkerType,
} from "@xyflow/react";
import { useLitStore } from "@/lib/litgraph/store";
import { NODE_TYPES, EDGE_TYPES } from "@/lib/litgraph/types";
import LitNodeView, { type LitFlowNode } from "./LitNodeView";
import LitEdgeView from "./LitEdgeView";

const nodeTypes = { default: LitNodeView } as unknown as Record<string, typeof LitNodeView>;
const edgeTypes = { default: LitEdgeView } as unknown as Record<string, typeof LitEdgeView>;

function CanvasInner() {
  const nodes = useLitStore((s) => s.nodes);
  const edges = useLitStore((s) => s.edges);
  const onNodesChangeRaw = useLitStore((s) => s.onNodesChange);
  const onEdgesChangeRaw = useLitStore((s) => s.onEdgesChange);
  const onConnectRaw = useLitStore((s) => s.onConnect);
  const setSelectedNode = useLitStore((s) => s.setSelectedNode);
  const setSelectedEdge = useLitStore((s) => s.setSelectedEdge);
  const setEditingNode = useLitStore((s) => s.setEditingNode);
  const selectedNodeId = useLitStore((s) => s.selectedNodeId);
  const selectedEdgeId = useLitStore((s) => s.selectedEdgeId);
  const focusNodeId = useLitStore((s) => s.focusNodeId);
  const focusEnabled = useLitStore((s) => s.focusEnabled);
  const deleteNode = useLitStore((s) => s.deleteNode);
  const duplicateNode = useLitStore((s) => s.duplicateNode);
  const deleteEdge = useLitStore((s) => s.deleteEdge);

  // Множество ID нод, которые должны быть в фокусе
  const focusSet = useMemo(() => {
    if (!focusEnabled || !focusNodeId) return null;
    const set = new Set<string>([focusNodeId]);
    for (const e of edges) {
      if (e.source === focusNodeId) set.add(e.target);
      if (e.target === focusNodeId) set.add(e.source);
    }
    return set;
  }, [focusNodeId, focusEnabled, edges]);

  // Преобразуем ноды в формат React Flow
  // useMemo с правильными зависимостями — не пересоздаётся при перемещении
  const rfNodes = useMemo(
    () =>
      nodes.map((n) => {
        const inFocus = focusSet === null || focusSet.has(n.id);
        return {
          id: n.id,
          type: "default",
          position: n.position,
          data: { ...n.data, dimmed: !inFocus },
          selected: n.id === selectedNodeId,
          style: {
            opacity: inFocus ? 1 : 0.15,
            filter: inFocus ? undefined : "grayscale(80%)",
            pointerEvents: inFocus ? undefined : ("none" as const),
          },
        };
      }) as LitFlowNode[],
    [nodes, selectedNodeId, focusSet]
  );

  const rfEdges = useMemo(
    () =>
      edges.map((e) => {
        const kind = e.data?.kind ?? "flow";
        const cfg = EDGE_TYPES[kind];
        const inFocus =
          focusSet === null ||
          (focusSet.has(e.source) && focusSet.has(e.target));
        return {
          id: e.id,
          source: e.source,
          target: e.target,
          sourceHandle: e.sourceHandle ?? undefined,
          targetHandle: e.targetHandle ?? undefined,
          type: "default",
          animated: e.animated ?? cfg.animated,
          data: { ...e.data, dimmed: !inFocus },
          selected: e.id === selectedEdgeId,
          markerEnd: {
            type: MarkerType.ArrowClosed,
            color: cfg.color,
            width: 18,
            height: 18,
          },
        };
      }),
    [edges, selectedEdgeId, focusSet]
  );

  const onNodesChange: OnNodesChange = useCallback(
    (changes) => onNodesChangeRaw(changes),
    [onNodesChangeRaw]
  );
  const onEdgesChange: OnEdgesChange = useCallback(
    (changes) => onEdgesChangeRaw(changes),
    [onEdgesChangeRaw]
  );
  const onConnect = useCallback(
    (conn: Connection) => onConnectRaw(conn),
    [onConnectRaw]
  );

  const onNodeClick: NodeMouseHandler = useCallback(
    (_, node) => setSelectedNode(node.id),
    [setSelectedNode]
  );

  const onEdgeClick: EdgeMouseHandler = useCallback(
    (_, edge) => setSelectedEdge(edge.id),
    [setSelectedEdge]
  );

  const onPaneClick = useCallback(() => {
    setSelectedNode(null);
    setSelectedEdge(null);
  }, [setSelectedNode, setSelectedEdge]);

  const onNodeDoubleClick: NodeMouseHandler = useCallback(
    (_, node) => setEditingNode(node.id),
    [setEditingNode]
  );

  // Горячие клавиши
  useEffect(() => {
    function handler(e: KeyboardEvent) {
      const tag = (e.target as HTMLElement)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || tag === "SELECT") return;
      const isMeta = e.ctrlKey || e.metaKey;

      if ((e.key === "Delete" || e.key === "Backspace") && selectedNodeId) {
        e.preventDefault();
        deleteNode(selectedNodeId);
      } else if ((e.key === "Delete" || e.key === "Backspace") && selectedEdgeId) {
        e.preventDefault();
        deleteEdge(selectedEdgeId);
      } else if (isMeta && e.key.toLowerCase() === "d" && selectedNodeId) {
        e.preventDefault();
        duplicateNode(selectedNodeId);
      }
    }
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [selectedNodeId, selectedEdgeId, deleteNode, deleteEdge, duplicateNode]);

  const nodeColor = useCallback((node: LitFlowNode) => {
    return NODE_TYPES[node.data.type].color;
  }, []);

  return (
    <div className="flex-1 relative lit-canvas-bg">
      <ReactFlow
        nodes={rfNodes}
        edges={rfEdges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onNodeClick={onNodeClick}
        onEdgeClick={onEdgeClick}
        onPaneClick={onPaneClick}
        onNodeDoubleClick={onNodeDoubleClick}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        // ====== ОПТИМИЗАЦИИ ПРОИЗВОДИТЕЛЬНОСТИ ======
        // Рендерить только видимые ноды (виртуализация)
        onlyRenderVisibleElements
        // Не поднимать ноды по z-index при выборе (меньше repaint)
        elevateNodesOnSelect={false}
        // Не двигать холст при соединении нод
        autoPanOnConnect={false}
        // Минимальный порог движения для drag (1px)
        nodeDragThreshold={1}
        // Радиус соединения (меньше = точнее)
        connectionRadius={30}
        // Разрешить интерактивность
        nodesDraggable
        nodesConnectable
        nodesFocusable
        elementsSelectable
        // fitView только при первой загрузке
        fitView
        fitViewOptions={{ padding: 0.25, maxZoom: 1 }}
        minZoom={0.1}
        maxZoom={2.5}
        defaultEdgeOptions={{
          type: "default",
          markerEnd: { type: MarkerType.ArrowClosed, color: "#8B5A2B", width: 18, height: 18 },
        }}
        proOptions={{ hideAttribution: true }}
        className="bg-transparent"
      >
        <Background
          variant={BackgroundVariant.Dots}
          gap={20}
          size={1.5}
          color="#B8A88C"
        />
        <Controls
          showInteractive={false}
          position="bottom-right"
          className="!shadow-md"
        />
        <MiniMap
          nodeColor={nodeColor}
          nodeStrokeColor="#fff"
          nodeStrokeWidth={2}
          nodeBorderRadius={6}
          maskColor="rgba(245, 239, 225, 0.6)"
          pannable
          zoomable
          position="bottom-left"
          className="!bg-white"
        />
      </ReactFlow>

      {/* Индикатор focus-режима */}
      {focusEnabled && focusNodeId && (
        <div className="absolute top-3 left-1/2 -translate-x-1/2 z-10 pointer-events-none">
          <div className="bg-stone-800/85 text-white text-xs px-3 py-1.5 rounded-full shadow-lg flex items-center gap-2 pointer-events-auto">
            <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
            Focus-режим: видна только выбранная нода и её связи
            <button
              onClick={() => setSelectedNode(null)}
              className="ml-1 hover:text-amber-300"
              title="Выйти из фокуса (Esc)"
            >
              ✕
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

export function LitCanvas() {
  return (
    <ReactFlowProvider>
      <CanvasInner />
    </ReactFlowProvider>
  );
}
