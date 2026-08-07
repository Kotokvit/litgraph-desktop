"use client";

import { useState, useEffect, useRef } from "react";
import { useLitStore } from "@/lib/litgraph/store";
import { NODE_TYPES, NODE_TYPE_ORDER, type LitNodeType } from "@/lib/litgraph/types";
import {
  Pencil, Copy, Trash2, ArrowUpCircle, X,
} from "lucide-react";

interface ContextMenuState {
  x: number;
  y: number;
  nodeId: string;
}

export function NodeContextMenu() {
  const setEditingNode = useLitStore((s) => s.setEditingNode);
  const deleteNode = useLitStore((s) => s.deleteNode);
  const duplicateNode = useLitStore((s) => s.duplicateNode);
  const updateNode = useLitStore((s) => s.updateNode);
  const nodes = useLitStore((s) => s.nodes);
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);
  const [showTypeChanger, setShowTypeChanger] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  // Слушатель правого клика из CanvasRenderer
  useEffect(() => {
    function handleContextMenu(e: Event) {
      const detail = (e as CustomEvent).detail;
      if (detail?.nodeId) {
        setContextMenu({ x: detail.x, y: detail.y, nodeId: detail.nodeId });
        setShowTypeChanger(false);
      }
    }
    window.addEventListener("litgraph:contextmenu", handleContextMenu as EventListener);
    return () => window.removeEventListener("litgraph:contextmenu", handleContextMenu as EventListener);
  }, []);

  // Закрытие меню при клике вне его
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (menuRef.current && !menuRef.current.contains(e.target as HTMLElement)) {
        setContextMenu(null);
        setShowTypeChanger(false);
      }
    }
    if (contextMenu) {
      document.addEventListener("mousedown", handleClickOutside);
      return () => document.removeEventListener("mousedown", handleClickOutside);
    }
  }, [contextMenu]);

  if (!contextMenu) return null;

  const node = nodes.find((n) => n.id === contextMenu.nodeId);
  if (!node) return null;

  const cfg = NODE_TYPES[node.type as keyof typeof NODE_TYPES] || NODE_TYPES.idea;

  function handleDelete() {
    if (confirm("Удалить эту ноду? Все её связи тоже будут удалены.")) {
      deleteNode(contextMenu!.nodeId);
    }
    setContextMenu(null);
  }

  function handleDuplicate() {
    duplicateNode(contextMenu!.nodeId);
    setContextMenu(null);
  }

  function handleEdit() {
    setEditingNode(contextMenu!.nodeId);
    setContextMenu(null);
  }

  function handleChangeType(newType: LitNodeType) {
    updateNode(contextMenu!.nodeId, { type: newType });
    setContextMenu(null);
    setShowTypeChanger(false);
  }

  return (
    <div
      ref={menuRef}
      className="fixed z-50 bg-white rounded-lg shadow-xl border border-stone-200 py-1 min-w-[180px]"
      style={{ left: contextMenu.x, top: contextMenu.y }}
    >
      {/* Заголовок */}
      <div className="px-3 py-1.5 border-b border-stone-100 flex items-center gap-2">
        <div
          className="w-2 h-2 rounded-full"
          style={{ background: cfg.color }}
        />
        <span className="text-xs font-medium text-stone-700 truncate">
          {node.data.title}
        </span>
      </div>

      {!showTypeChanger ? (
        <>
          {/* Основные действия */}
          <button
            onClick={handleEdit}
            className="w-full px-3 py-2 flex items-center gap-2 text-sm text-stone-700 hover:bg-stone-50"
          >
            <Pencil className="w-3.5 h-3.5 text-stone-500" />
            Редактировать
          </button>
          <button
            onClick={handleDuplicate}
            className="w-full px-3 py-2 flex items-center gap-2 text-sm text-stone-700 hover:bg-stone-50"
          >
            <Copy className="w-3.5 h-3.5 text-stone-500" />
            Дублировать
          </button>
          <button
            onClick={() => setShowTypeChanger(true)}
            className="w-full px-3 py-2 flex items-center gap-2 text-sm text-stone-700 hover:bg-stone-50"
          >
            <ArrowUpCircle className="w-3.5 h-3.5 text-stone-500" />
            Изменить тип
          </button>
          <div className="border-t border-stone-100 my-1" />
          <button
            onClick={handleDelete}
            className="w-full px-3 py-2 flex items-center gap-2 text-sm text-red-600 hover:bg-red-50"
          >
            <Trash2 className="w-3.5 h-3.5" />
            Удалить
          </button>
        </>
      ) : (
        <>
          {/* Выбор типа */}
          <div className="px-3 py-1.5 text-[10px] uppercase tracking-wider text-stone-400">
            Выберите тип
          </div>
          {NODE_TYPE_ORDER.map((type) => {
            const tcfg = NODE_TYPES[type];
            const isCurrent = node.type === type;
            return (
              <button
                key={type}
                onClick={() => handleChangeType(type)}
                className={`w-full px-3 py-1.5 flex items-center gap-2 text-sm hover:bg-stone-50 ${
                  isCurrent ? "bg-stone-100" : ""
                }`}
              >
                <div
                  className="w-3 h-3 rounded shrink-0"
                  style={{ background: tcfg.color }}
                />
                <span className="text-stone-700">{tcfg.singular}</span>
                {isCurrent && (
                  <span className="ml-auto text-[10px] text-stone-400">текущий</span>
                )}
              </button>
            );
          })}
          <div className="border-t border-stone-100 my-1" />
          <button
            onClick={() => setShowTypeChanger(false)}
            className="w-full px-3 py-2 flex items-center gap-2 text-sm text-stone-500 hover:bg-stone-50"
          >
            <X className="w-3.5 h-3.5" />
            Назад
          </button>
        </>
      )}
    </div>
  );
}

// Плавающая панель действий для выбранной ноды (как в Figma)
export function FloatingActions() {
  const selectedNodeId = useLitStore((s) => s.selectedNodeId);
  const setEditingNode = useLitStore((s) => s.setEditingNode);
  const deleteNode = useLitStore((s) => s.deleteNode);
  const duplicateNode = useLitStore((s) => s.duplicateNode);
  const nodes = useLitStore((s) => s.nodes);

  if (!selectedNodeId) return null;
  const node = nodes.find((n) => n.id === selectedNodeId);
  if (!node) return null;

  const cfg = NODE_TYPES[node.type as keyof typeof NODE_TYPES] || NODE_TYPES.idea;

  return (
    <div className="absolute top-3 right-3 z-10 flex items-center gap-1 bg-white rounded-lg shadow-md p-1 border border-stone-200">
      <div className="flex items-center gap-1.5 px-2">
        <div
          className="w-2 h-2 rounded-full"
          style={{ background: cfg.color }}
        />
        <span className="text-[10px] text-stone-500">
          {cfg.singular}
        </span>
      </div>
      <div className="w-px h-5 bg-stone-200" />
      <button
        onClick={() => setEditingNode(selectedNodeId)}
        className="w-7 h-7 flex items-center justify-center hover:bg-stone-100 rounded text-stone-600"
        title="Редактировать (двойной клик)"
      >
        <Pencil className="w-3.5 h-3.5" />
      </button>
      <button
        onClick={() => duplicateNode(selectedNodeId)}
        className="w-7 h-7 flex items-center justify-center hover:bg-stone-100 rounded text-stone-600"
        title="Дублировать (Ctrl+D)"
      >
        <Copy className="w-3.5 h-3.5" />
      </button>
      <button
        onClick={() => {
          if (confirm("Удалить эту ноду? Все её связи тоже будут удалены.")) {
            deleteNode(selectedNodeId);
          }
        }}
        className="w-7 h-7 flex items-center justify-center hover:bg-red-50 rounded text-red-500"
        title="Удалить (Del)"
      >
        <Trash2 className="w-3.5 h-3.5" />
      </button>
    </div>
  );
}
