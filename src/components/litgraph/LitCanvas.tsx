"use client";

import { useCallback, useEffect } from "react";
import { CanvasRenderer } from "./CanvasRenderer";
import { NodeContextMenu, FloatingActions } from "./NodeActions";
import { useLitStore } from "@/lib/litgraph/store";

function CanvasInner() {
  const nodes = useLitStore((s) => s.nodes);
  const edges = useLitStore((s) => s.edges);
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

  const onEdgeClick = useCallback(
    (id: string) => setSelectedEdge(id),
    [setSelectedEdge]
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

  return (
    <>
      <div className="flex-1 relative">
        <CanvasRenderer
          nodes={nodes}
          edges={edges}
          selectedNodeId={selectedNodeId}
          selectedEdgeId={selectedEdgeId}
          focusNodeId={focusNodeId}
          focusEnabled={focusEnabled}
          onNodeClick={setSelectedNode}
          onEdgeClick={onEdgeClick}
          onPaneClick={() => {
            setSelectedNode(null);
            setSelectedEdge(null);
          }}
          onNodeDoubleClick={setEditingNode}
        />
        {/* Плавающая панель действий для выбранной ноды */}
        <FloatingActions />
      </div>
      {/* Контекстное меню (правый клик) */}
      <NodeContextMenu />
    </>
  );
}

export function LitCanvas() {
  return <CanvasInner />;
}
