"use client";

import { useRef, useEffect, useState, useCallback } from "react";
import type { LitNode, LitEdge } from "@/lib/litgraph/types";
import { NODE_TYPES, EDGE_TYPES } from "@/lib/litgraph/types";

interface Viewport {
  x: number; // pan offset
  y: number;
  zoom: number;
}

interface CanvasRendererProps {
  nodes: LitNode[];
  edges: LitEdge[];
  selectedNodeId: string | null;
  selectedEdgeId: string | null;
  focusNodeId: string | null;
  focusEnabled: boolean;
  onNodeClick: (id: string) => void;
  onEdgeClick: (id: string) => void;
  onPaneClick: () => void;
  onNodeDoubleClick: (id: string) => void;
  width?: number;
  height?: number;
}

const NODE_WIDTH = 260;
const NODE_HEIGHT = 90; // приближённая высота

export function CanvasRenderer({
  nodes,
  edges,
  selectedNodeId,
  selectedEdgeId,
  focusNodeId,
  focusEnabled,
  onNodeClick,

  onPaneClick,
  onNodeDoubleClick,
  width = 1200,
  height = 800,
}: CanvasRendererProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [viewport, setViewport] = useState<Viewport>({ x: 0, y: 0, zoom: 1 });
  const [isDragging, setIsDragging] = useState(false);
  const [dragStart, setDragStart] = useState({ x: 0, y: 0 });
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [actualSize, setActualSize] = useState({ width, height });

  // Resize observer для заполнения контейнера
  useEffect(() => {
    if (!containerRef.current) return;
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (entry) {
        setActualSize({
          width: Math.floor(entry.contentRect.width),
          height: Math.floor(entry.contentRect.height),
        });
      }
    });
    ro.observe(containerRef.current);
    return () => ro.disconnect();
  }, []);

  // Слушатель события fitView (из Toolbar)
  useEffect(() => {
    function handleFitView() {
      if (nodes.length === 0) return;
      const minX = Math.min(...nodes.map((n) => n.position.x));
      const minY = Math.min(...nodes.map((n) => n.position.y));
      const maxX = Math.max(...nodes.map((n) => n.position.x + NODE_WIDTH));
      const maxY = Math.max(...nodes.map((n) => n.position.y + NODE_HEIGHT));
      const padding = 40;
      const zoomX = (actualSize.width - padding * 2) / (maxX - minX);
      const zoomY = (actualSize.height - padding * 2) / (maxY - minY);
      const zoom = Math.min(zoomX, zoomY, 1);
      setViewport({
        x: padding - minX * zoom,
        y: padding - minY * zoom,
        zoom,
      });
    }
    window.addEventListener("litgraph:fitview", handleFitView);
    return () => window.removeEventListener("litgraph:fitview", handleFitView);
  }, [nodes, actualSize]);

  // Авто-fit при первой загрузке
  useEffect(() => {
    if (nodes.length > 0 && viewport.x === 0 && viewport.y === 0 && viewport.zoom === 1) {
      window.dispatchEvent(new CustomEvent("litgraph:fitview"));
    }
  }, [nodes.length]);

  // Слушатель add-center (из NodePalette)
  useEffect(() => {
    function handleAddCenter(e: Event) {
      const type = (e as CustomEvent).detail?.type;
      if (!type) return;
      // Вычисляем центр видимой области
      const centerX = (-viewport.x + actualSize.width / 2) / viewport.zoom;
      const centerY = (-viewport.y + actualSize.height / 2) / viewport.zoom;
      // Импортируем store напрямую чтобы не прокидывать через props
      import("@/lib/litgraph/store").then(({ useLitStore }) => {
        useLitStore.getState().addNode(type, { x: centerX - 130, y: centerY - 45 });
      });
    }
    window.addEventListener("litgraph:add-center", handleAddCenter);
    return () => window.removeEventListener("litgraph:add-center", handleAddCenter);
  }, [viewport, actualSize]);

  // Focus set
  const focusSet = useCallback(() => {
    if (!focusEnabled || !focusNodeId) return null;
    const set = new Set<string>([focusNodeId]);
    for (const e of edges) {
      if (e.source === focusNodeId) set.add(e.target);
      if (e.target === focusNodeId) set.add(e.source);
    }
    return set;
  }, [focusEnabled, focusNodeId, edges]);

  // Рендеринг на canvas
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const W = actualSize.width;
    const H = actualSize.height;
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    canvas.style.width = `${W}px`;
    canvas.style.height = `${H}px`;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    // Очистка
    ctx.fillStyle = "#f5efe1";
    ctx.fillRect(0, 0, W, H);

    // Сетка (точки)
    ctx.fillStyle = "#B8A88C";
    const dotGap = 20 * viewport.zoom;
    const offsetX = viewport.x % dotGap;
    const offsetY = viewport.y % dotGap;
    for (let x = offsetX; x < W; x += dotGap) {
      for (let y = offsetY; y < H; y += dotGap) {
        ctx.fillRect(x, y, 1.5, 1.5);
      }
    }

    // Применяем viewport transform
    ctx.save();
    ctx.translate(viewport.x, viewport.y);
    ctx.scale(viewport.zoom, viewport.zoom);

    const fSet = focusSet();

    // Рисуем рёбра (сначала под нодами)
    for (const edge of edges) {
      const source = nodes.find((n) => n.id === edge.source);
      const target = nodes.find((n) => n.id === edge.target);
      if (!source || !target) continue;

      const kind = edge.data?.kind ?? "flow";
      const cfg = EDGE_TYPES[kind as keyof typeof EDGE_TYPES] || EDGE_TYPES.flow;
      const inFocus = !fSet || (fSet.has(edge.source) && fSet.has(edge.target));
      const isSelected = edge.id === selectedEdgeId;

      // Culling: не рисовать рёбра вне видимой области
      const sx = source.position.x + NODE_WIDTH;
      const sy = source.position.y + NODE_HEIGHT / 2;
      const tx = target.position.x;
      const ty = target.position.y + NODE_HEIGHT / 2;

      const minX = Math.min(sx, tx) - 50;
      const maxX = Math.max(sx, tx) + 50;
      const minY = Math.min(sy, ty) - 50;
      const maxY = Math.max(sy, ty) + 50;

      const viewLeft = -viewport.x / viewport.zoom;
      const viewTop = -viewport.y / viewport.zoom;
      const viewRight = viewLeft + W / viewport.zoom;
      const viewBottom = viewTop + H / viewport.zoom;

      if (maxX < viewLeft || minX > viewRight || maxY < viewTop || minY > viewBottom) continue;

      // Bezier curve
      const dx = tx - sx;
      const cp1x = sx + dx * 0.5;
      const cp1y = sy;
      const cp2x = tx - dx * 0.5;
      const cp2y = ty;

      ctx.beginPath();
      ctx.moveTo(sx, sy);
      ctx.bezierCurveTo(cp1x, cp1y, cp2x, cp2y, tx, ty);

      ctx.strokeStyle = cfg.color;
      ctx.lineWidth = isSelected ? 3 : 2;
      ctx.globalAlpha = inFocus ? (isSelected ? 1 : 0.85) : 0.15;
      if (cfg.dashed) {
        ctx.setLineDash([6, 4]);
      } else {
        ctx.setLineDash([]);
      }
      ctx.stroke();
      ctx.setLineDash([]);

      // Label (только для видимых и не-dimmed)
      if (inFocus && viewport.zoom > 0.5) {
        const labelX = (sx + tx) / 2;
        const labelY = (sy + ty) / 2;
        ctx.font = "10px sans-serif";
        const metrics = ctx.measureText(cfg.label);
        const padding = 6;
        const labelW = metrics.width + padding * 2;
        const labelH = 14;
        ctx.fillStyle = "#fff";
        ctx.strokeStyle = cfg.color + "40";
        ctx.globalAlpha = isSelected ? 1 : 0.85;
        ctx.beginPath();
        ctx.roundRect(labelX - labelW / 2, labelY - labelH / 2, labelW, labelH, 9999);
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = cfg.color;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(cfg.label, labelX, labelY);
      }
      ctx.globalAlpha = 1;
    }

    // Рисуем ноды
    for (const node of nodes) {
      const cfg = NODE_TYPES[node.type as keyof typeof NODE_TYPES] || NODE_TYPES.idea;
      const inFocus = !fSet || fSet.has(node.id);
      const isSelected = node.id === selectedNodeId;
      const isHovered = node.id === hoveredNodeId;

      // Culling
      const nx = node.position.x;
      const ny = node.position.y;
      const viewLeft = -viewport.x / viewport.zoom;
      const viewTop = -viewport.y / viewport.zoom;
      const viewRight = viewLeft + W / viewport.zoom;
      const viewBottom = viewTop + H / viewport.zoom;

      if (nx + NODE_WIDTH < viewLeft || nx > viewRight || ny + NODE_HEIGHT < viewTop || ny > viewBottom) continue;

      // Тень
      if (isSelected || isHovered) {
        ctx.shadowColor = cfg.color + "60";
        ctx.shadowBlur = 12;
        ctx.shadowOffsetX = 0;
        ctx.shadowOffsetY = 4;
      }

      // Фон ноды
      ctx.globalAlpha = inFocus ? 1 : 0.15;
      ctx.fillStyle = "#fff";
      ctx.beginPath();
      ctx.roundRect(nx, ny, NODE_WIDTH, NODE_HEIGHT, 11);
      ctx.fill();

      ctx.shadowColor = "transparent";
      ctx.shadowBlur = 0;

      // Левая цветная полоса
      ctx.fillStyle = cfg.color;
      ctx.beginPath();
      ctx.roundRect(nx, ny, 4, NODE_HEIGHT, [11, 0, 0, 11]);
      ctx.fill();

      // Шапка (цветной фон)
      ctx.fillStyle = cfg.color + "18";
      ctx.beginPath();
      ctx.roundRect(nx + 4, ny, NODE_WIDTH - 4, 28, [0, 11, 0, 0]);
      ctx.fill();

      // Иконка (круг с буквой)
      ctx.fillStyle = cfg.color;
      ctx.beginPath();
      ctx.arc(nx + 20, ny + 14, 9, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = "#fff";
      ctx.font = "bold 10px sans-serif";
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(cfg.singular[0] || "?", nx + 20, ny + 14);

      // Тип ноды
      ctx.fillStyle = cfg.color;
      ctx.font = "600 10px sans-serif";
      ctx.textAlign = "left";
      ctx.fillText(cfg.singular.toUpperCase(), nx + 35, ny + 14);

      // Заголовок
      ctx.fillStyle = "#292524";
      ctx.font = "600 13px sans-serif";
      ctx.textAlign = "left";
      ctx.textBaseline = "top";
      const title = node.data.title || "Без названия";
      const titleMaxWidth = NODE_WIDTH - 24;
      let displayTitle = title;
      if (ctx.measureText(title).width > titleMaxWidth) {
        let cutTitle = title;
        while (ctx.measureText(cutTitle + "…").width > titleMaxWidth && cutTitle.length > 0) {
          cutTitle = cutTitle.slice(0, -1);
        }
        displayTitle = cutTitle + "…";
      }
      ctx.fillText(displayTitle, nx + 12, ny + 34);

      // Тело (превью)
      if (node.data.body && viewport.zoom > 0.4) {
        ctx.fillStyle = "#78716c";
        ctx.font = "11px sans-serif";
        const body = node.data.body.slice(0, 120);
        const bodyMaxWidth = NODE_WIDTH - 24;
        // Разбиваем на строки
        const words = body.split(/\s+/);
        let line = "";
        let lineY = ny + 54;
        for (const word of words) {
          const testLine = line ? line + " " + word : word;
          if (ctx.measureText(testLine).width > bodyMaxWidth && line) {
            ctx.fillText(line, nx + 12, lineY);
            line = word;
            lineY += 14;
            if (lineY > ny + NODE_HEIGHT - 10) break;
          } else {
            line = testLine;
          }
        }
        if (line && lineY <= ny + NODE_HEIGHT - 10) {
          ctx.fillText(line, nx + 12, lineY);
        }
      }

      // Selection ring
      if (isSelected) {
        ctx.strokeStyle = cfg.color;
        ctx.lineWidth = 2;
        ctx.globalAlpha = 0.4;
        ctx.beginPath();
        ctx.roundRect(nx - 2, ny - 2, NODE_WIDTH + 4, NODE_HEIGHT + 4, 13);
        ctx.stroke();
      }

      ctx.globalAlpha = 1;

      // Handle circles
      ctx.fillStyle = "#fff";
      ctx.strokeStyle = cfg.color;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(nx, ny + NODE_HEIGHT / 2, 6, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
      ctx.beginPath();
      ctx.arc(nx + NODE_WIDTH, ny + NODE_HEIGHT / 2, 6, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    }

    ctx.restore();
  }, [nodes, edges, viewport, actualSize, selectedNodeId, selectedEdgeId, hoveredNodeId, focusSet]);

  // Hit testing — найти ноду по координатам экрана
  const findNodeAt = useCallback(
    (screenX: number, screenY: number): LitNode | null => {
      // Преобразуем экранные координаты в координаты холста
      const worldX = (screenX - viewport.x) / viewport.zoom;
      const worldY = (screenY - viewport.y) / viewport.zoom;

      // Идём с конца (верхние ноды в массиве = нарисованы последними)
      for (let i = nodes.length - 1; i >= 0; i--) {
        const n = nodes[i];
        if (
          worldX >= n.position.x &&
          worldX <= n.position.x + NODE_WIDTH &&
          worldY >= n.position.y &&
          worldY <= n.position.y + NODE_HEIGHT
        ) {
          return n;
        }
      }
      return null;
    },
    [nodes, viewport]
  );

  const handleDoubleClick = useCallback(
    (e: React.MouseEvent) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const node = findNodeAt(x, y);
      if (node) onNodeDoubleClick(node.id);
    },
    [findNodeAt, onNodeDoubleClick]
  );

  // Wheel zoom
  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      e.preventDefault();
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;
      const mouseX = e.clientX - rect.left;
      const mouseY = e.clientY - rect.top;

      const delta = -e.deltaY * 0.001;
      const newZoom = Math.max(0.1, Math.min(2.5, viewport.zoom * (1 + delta)));
      const scale = newZoom / viewport.zoom;

      // Zoom к позиции мыши
      setViewport((vp) => ({
        x: mouseX - (mouseX - vp.x) * scale,
        y: mouseY - (mouseY - vp.y) * scale,
        zoom: newZoom,
      }));
    },
    [viewport]
  );

  // Правый клик — контекстное меню
  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const node = findNodeAt(x, y);
      if (node) {
        onNodeClick(node.id);
        window.dispatchEvent(new CustomEvent("litgraph:contextmenu", {
          detail: { x: e.clientX, y: e.clientY, nodeId: node.id }
        }));
      }
    },
    [findNodeAt, onNodeClick]
  );

  // Перетаскивание ноды
  const [draggingNode, setDraggingNode] = useState<string | null>(null);
  const [dragNodeStart, setDragNodeStart] = useState({ x: 0, y: 0, nodeX: 0, nodeY: 0 });

  const handleMouseDownEnhanced = useCallback(
    (e: React.MouseEvent) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      const node = findNodeAt(x, y);
      if (node) {
        onNodeClick(node.id);
        // Начинаем перетаскивание ноды
        setDraggingNode(node.id);
        setDragNodeStart({
          x,
          y,
          nodeX: node.position.x,
          nodeY: node.position.y,
        });
      } else {
        onPaneClick();
        setIsDragging(true);
        setDragStart({ x: x - viewport.x, y: y - viewport.y });
      }
    },
    [findNodeAt, onNodeClick, onPaneClick, viewport]
  );

  const handleMouseMoveEnhanced = useCallback(
    (e: React.MouseEvent) => {
      const rect = canvasRef.current?.getBoundingClientRect();
      if (!rect) return;
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;

      if (draggingNode) {
        // Перетаскиваем ноду
        const dx = (x - dragNodeStart.x) / viewport.zoom;
        const dy = (y - dragNodeStart.y) / viewport.zoom;
        const newX = dragNodeStart.nodeX + dx;
        const newY = dragNodeStart.nodeY + dy;
        // Обновляем позицию ноды в store
        import("@/lib/litgraph/store").then(({ useLitStore }) => {
          useLitStore.getState().updateNode(draggingNode, {
            position: { x: newX, y: newY },
          });
        });
      } else if (isDragging) {
        setViewport((vp) => ({ ...vp, x: x - dragStart.x, y: y - dragStart.y }));
      } else {
        const node = findNodeAt(x, y);
        const newHovered = node?.id || null;
        if (newHovered !== hoveredNodeId) {
          setHoveredNodeId(newHovered);
          canvasRef.current!.style.cursor = node ? "pointer" : "grab";
        }
      }
    },
    [isDragging, dragStart, findNodeAt, hoveredNodeId, draggingNode, dragNodeStart, viewport]
  );

  const handleMouseUpEnhanced = useCallback(() => {
    setIsDragging(false);
    setDraggingNode(null);
  }, []);

  return (
    <div ref={containerRef} className="flex-1 relative lit-canvas-bg overflow-hidden">
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDownEnhanced}
        onMouseMove={handleMouseMoveEnhanced}
        onMouseUp={handleMouseUpEnhanced}
        onMouseLeave={handleMouseUpEnhanced}
        onDoubleClick={handleDoubleClick}
        onContextMenu={handleContextMenu}
        onWheel={handleWheel}
        style={{ display: "block", cursor: "grab" }}
      />

      {/* Контролы зума */}
      <div className="absolute bottom-4 right-4 flex flex-col gap-1 bg-white rounded-lg shadow-md p-1">
        <button
          onClick={() => setViewport((vp) => ({ ...vp, zoom: Math.min(2.5, vp.zoom * 1.2) }))}
          className="w-8 h-8 flex items-center justify-center hover:bg-stone-100 rounded text-stone-600"
          title="Увеличить"
        >
          +
        </button>
        <button
          onClick={() => setViewport((vp) => ({ ...vp, zoom: Math.max(0.1, vp.zoom / 1.2) }))}
          className="w-8 h-8 flex items-center justify-center hover:bg-stone-100 rounded text-stone-600"
          title="Уменьшить"
        >
          −
        </button>
        <button
          onClick={() => {
            // Fit view
            if (nodes.length === 0) return;
            const minX = Math.min(...nodes.map((n) => n.position.x));
            const minY = Math.min(...nodes.map((n) => n.position.y));
            const maxX = Math.max(...nodes.map((n) => n.position.x + NODE_WIDTH));
            const maxY = Math.max(...nodes.map((n) => n.position.y + NODE_HEIGHT));
            const padding = 40;
            const zoomX = (actualSize.width - padding * 2) / (maxX - minX);
            const zoomY = (actualSize.height - padding * 2) / (maxY - minY);
            const zoom = Math.min(zoomX, zoomY, 1);
            setViewport({
              x: padding - minX * zoom,
              y: padding - minY * zoom,
              zoom,
            });
          }}
          className="w-8 h-8 flex items-center justify-center hover:bg-stone-100 rounded text-stone-600 text-xs"
          title="Уместить в экран"
        >
          ⤢
        </button>
      </div>

      {/* Индикатор фокуса */}
      {focusEnabled && focusNodeId && (
        <div className="absolute top-3 left-1/2 -translate-x-1/2 z-10 pointer-events-none">
          <div className="bg-stone-800/85 text-white text-xs px-3 py-1.5 rounded-full shadow-lg flex items-center gap-2">
            <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
            Focus-режим
          </div>
        </div>
      )}
    </div>
  );
}
