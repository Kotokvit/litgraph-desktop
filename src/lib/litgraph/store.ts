import { create } from "zustand";
import { persist, createJSONStorage } from "zustand/middleware";
import type {
  LitNode,
  LitEdge,
  LitNodeType,
  LitProject,
  EdgeKind,
  ChapterVersion,
  BackgroundLayer,
} from "./types";
import { NODE_TYPES } from "./types";

/**
 * Цель перехода в Reader mode.
 *
 * Хранит ВСЕ моменты узла (а не только выбранный), чтобы пользователь мог
 * листать prev/next прямо в Reader без возврата в TextMomentsDialog.
 */
export interface ReaderMomentRef {
  /** Позиция совпадения (byte offset в исходном тексте). */
  position: number;
  /** Конец окна фрагмента. */
  end: number;
  /** Глава, в которой находится момент (для заголовка в навигации). */
  chapterTitle: string;
  /** Какое ключевое слово совпало. */
  matchedKeyword: string;
}

export interface ReaderTarget {
  /** ID узла, для которого открыт Reader. */
  nodeId: string;
  /** Заголовок узла (для шапки Reader). */
  nodeTitle: string;
  /** Ключевые слова узла (для подсветки всех упоминаний в тексте). */
  keywords: string[];
  /** Все моменты узла в тексте (отсортированы по позиции). */
  moments: ReaderMomentRef[];
  /** Индекс в moments[], на который нужно прокрутить при открытии. */
  currentIndex: number;
}

/**
 * SVO triplet, опубликованный ReasoningEngine'ом в общий store.
 *
 * S1-D: ReasoningDialog публикует сюда case-validated triplets после
 * запуска Full Pipeline (v0.7+), а Inspector.tsx (S1-B) читает их, чтобы
 * показать SVO-history по выбранной ноде. Формат намеренно упрощён
 * относительно Rust-side `ValidatedTriplet` (actor/target/confidence/caseValidation):
 * здесь только то, что нужно UI — subject/verb/object + confidence +
 * caseValid (boolean-проекция из CaseValidationResult.overall) +
 * опциональное исходное предложение.
 */
export interface SvoTriplet {
  /** кто действует (Rust: actor) */
  subject: string;
  /** что делает (Rust: verb) */
  verb: string;
  /** на ком/чём (Rust: target, может быть null) */
  object: string;
  /** 0..1, из Rust ValidatedTriplet.confidence */
  confidence?: number;
  /** прошла ли case-validation (true если caseValidation.overall === "Valid") */
  caseValid?: boolean;
  /** исходное предложение (опционально, Rust пока не заполняет) */
  sentence?: string;
}

// Утилита: сгенерировать id
function uid(prefix = "n"): string {
  return `${prefix}_${Date.now().toString(36)}_${Math.random()
    .toString(36)
    .slice(2, 8)}`;
}

// Дефолтный проект (демо-данные при первом запуске)
function createDefaultProject(): { title: string; author: string; description: string; nodes: LitNode[]; edges: LitEdge[] } {
  const chapterId = uid("ch");
  const scene1 = uid("sc");
  const scene2 = uid("sc");
  const hero = uid("ch");
  const vill = uid("ch");
  const loc = uid("loc");
  const plot = uid("pp");
  const conflict = uid("cf");

  const nodes: LitNode[] = [
    {
      id: chapterId,
      type: "chapter",
      position: { x: 80, y: 60 },
      data: {
        title: "Глава 1. Прибытие",
        body: "Герой приезжает в город, где ему предстоит раскрыть тайну. Тон задаётся интригующий.",
        type: "chapter",
        tags: ["введение"],
        meta: { wordTarget: 5000 },
      },
    },
    {
      id: scene1,
      type: "scene",
      position: { x: 80, y: 260 },
      data: {
        title: "Сцена: Вокзал",
        body: "Герой сходит с поезда. Толпа, пар, запах угля. Он замечает незнакомца, который за ним наблюдает.",
        type: "scene",
        tags: [],
        meta: { pov: "Герой", mood: "тревожное", timeOfDay: "Вечер", wordTarget: 1500 },
      },
    },
    {
      id: scene2,
      type: "scene",
      position: { x: 460, y: 260 },
      data: {
        title: "Сцена: Гостиница",
        body: "Герой снимает номер. Находит в ящике стола записку, адресованную ему по имени.",
        type: "scene",
        tags: ["загадка"],
        meta: { pov: "Герой", mood: "мистическое", timeOfDay: "Ночь", wordTarget: 1800 },
      },
    },
    {
      id: hero,
      type: "character",
      position: { x: 80, y: 540 },
      data: {
        title: "Антон",
        body: "Детектив 35 лет. Возвращается в родной город после 15 лет отсутствия. Скрывает прошлое.",
        type: "character",
        tags: ["главный герой"],
        meta: { characterArc: "От избегания прошлого — к его принятию.", importance: "high" },
      },
    },
    {
      id: vill,
      type: "character",
      position: { x: 460, y: 540 },
      data: {
        title: "Незнакомец",
        body: "Человек на вокзале. Кто он? Чего хочет? Пока неизвестно.",
        type: "character",
        tags: ["антагонист?"],
        meta: { importance: "medium" },
      },
    },
    {
      id: loc,
      type: "location",
      position: { x: 820, y: 260 },
      data: {
        title: "Старый город",
        body: "Узкие улочки, фонари, туман с реки. Атмосфера раннего осеннего вечера.",
        type: "location",
        tags: [],
        meta: {},
      },
    },
    {
      id: plot,
      type: "plotpoint",
      position: { x: 820, y: 60 },
      data: {
        title: "Завязка: записка",
        body: "Герой получает записку с собственным именем. С этого момента тайна начинает его преследовать.",
        type: "plotpoint",
        tags: [],
        meta: { importance: "high" },
      },
    },
    {
      id: conflict,
      type: "conflict",
      position: { x: 820, y: 540 },
      data: {
        title: "Конфликт: прошлое против настоящего",
        body: "Герой хочет начать новую жизнь, но прошлое не отпускает. Внутренний конфликт главы.",
        type: "conflict",
        tags: ["внутренний"],
        meta: { importance: "high" },
      },
    },
  ];

  const edges: LitEdge[] = [
    { id: uid("e"), source: chapterId, target: scene1, type: "smoothstep", data: { kind: "flow" }, animated: true },
    { id: uid("e"), source: scene1, target: scene2, type: "smoothstep", data: { kind: "flow" }, animated: true },
    { id: uid("e"), source: scene2, target: plot, type: "smoothstep", data: { kind: "cause" } },
    { id: uid("e"), source: hero, target: scene1, type: "smoothstep", data: { kind: "character" } },
    { id: uid("e"), source: hero, target: scene2, type: "smoothstep", data: { kind: "character" } },
    { id: uid("e"), source: vill, target: scene1, type: "smoothstep", data: { kind: "character" } },
    { id: uid("e"), source: loc, target: scene1, type: "smoothstep", data: { kind: "location" } },
    { id: uid("e"), source: loc, target: scene2, type: "smoothstep", data: { kind: "location" } },
    { id: uid("e"), source: hero, target: conflict, type: "smoothstep", data: { kind: "conflict" } },
    { id: uid("e"), source: vill, target: conflict, type: "smoothstep", data: { kind: "conflict" } },
  ];

  return {
    title: "Мой первый проект",
    author: "Автор",
    description: "Демонстрационный проект — чтобы сразу видеть, как всё устроено. Можно удалить и начать с чистого холста.",
    nodes,
    edges,
  };
}

interface LitStore {
  // ====== Состояние ======
  title: string;
  author: string;
  description: string;
  nodes: LitNode[];
  edges: LitEdge[];
  selectedNodeId: string | null;
  selectedEdgeId: string | null;
  editingNodeId: string | null;       // нода, открытая в модалке редактирования
  defaultEdgeKind: EdgeKind;
  searchQuery: string;
  hideTag: string | null;             // если задано — скрывать ноды с этим тегом
  focusNodeId: string | null;         // если задано — focus-режим: затемняются все ноды, кроме этой и её соседей
  focusEnabled: boolean;              // включён ли focus-режим глобально
  backgroundLayer: BackgroundLayer | null;  // фоновый слой (карта / схема / диаграмма)
  backgroundMoving: boolean;          // в данный момент перетаскивается фон (для cursor)
  sourceMarkdown: string;             // исходный .md текст (для "Text Moments" поиска по тексту)
  // ====== Reader mode ======
  // Полноэкранный читатель исходного текста с подсветкой выбранного фрагмента.
  // Открывается из TextMomentsDialog по клику на момент.
  readerOpen: boolean;
  readerTarget: ReaderTarget | null;

  // ====== S1-D: SVO triplets cache ======
  // ReasoningDialog публикует сюда case-validated triplets из Full Pipeline
  // (v0.7+), чтобы Inspector.tsx (S1-B) мог показать SVO-history без
  // повторного вызова Tauri. НЕ персистится в localStorage — это
  // runtime-кеш, пере-вычисляется при следующем запуске reasoning.
  svoTriplets: SvoTriplet[];

  // ====== Действия ======
  addNode: (type: LitNodeType, position?: { x: number; y: number }) => string;
  updateNode: (id: string, patch: Partial<LitNode>) => void;
  updateNodeData: (id: string, patch: Partial<LitNode["data"]>) => void;
  updateNodeMeta: (id: string, patch: Record<string, unknown>) => void;
  deleteNode: (id: string) => void;
  duplicateNode: (id: string) => void;
  setNodes: (nodes: LitNode[]) => void;
  setEdges: (edges: LitEdge[]) => void;
  onNodesChange: (changes: any) => void;       // прокси для React Flow
  onEdgesChange: (changes: any) => void;
  onConnect: (conn: any) => void;
  addEdge: (edge: LitEdge) => void;
  updateEdge: (id: string, patch: Partial<LitEdge>) => void;
  deleteEdge: (id: string) => void;
  setSelectedNode: (id: string | null) => void;
  setSelectedEdge: (id: string | null) => void;
  setEditingNode: (id: string | null) => void;
  setDefaultEdgeKind: (k: EdgeKind) => void;
  setSearchQuery: (q: string) => void;
  setHideTag: (tag: string | null) => void;
  setFocusNode: (id: string | null) => void;
  setFocusEnabled: (enabled: boolean) => void;
  // ====== Фоновый слой ======
  setBackgroundLayer: (layer: BackgroundLayer | null) => void;
  updateBackgroundLayer: (patch: Partial<BackgroundLayer>) => void;
  clearBackgroundLayer: () => void;
  toggleBackgroundVisibility: () => void;
  setBackgroundMoving: (moving: boolean) => void;
  setProjectMeta: (patch: Partial<Pick<LitStore, "title" | "author" | "description">>) => void;
  setSourceMarkdown: (text: string) => void;
  // ====== Reader mode ======
  openReader: (target: ReaderTarget) => void;
  closeReader: () => void;
  setReaderIndex: (index: number) => void;
  // ====== S1-D: SVO triplets cache ======
  setSvoTriplets: (t: SvoTriplet[]) => void;
  newProject: () => void;
  loadProject: (p: LitProject, sourceMarkdown?: string) => void;
  exportProject: () => LitProject;
  getVisibleNodes: () => LitNode[];
  getAllTags: () => string[];
  // Версионирование
  saveVersion: (nodeId: string, label?: string, source?: ChapterVersion["source"]) => void;
  restoreVersion: (nodeId: string, versionId: string) => void;
  deleteVersion: (nodeId: string, versionId: string) => void;
  getVersions: (nodeId: string) => ChapterVersion[];
}

export const useLitStore = create<LitStore>()(
  persist(
    (set, get) => ({
      title: "Мой первый проект",
      author: "Автор",
      description: "",
      nodes: [],
      edges: [],
      selectedNodeId: null,
      selectedEdgeId: null,
      editingNodeId: null,
      defaultEdgeKind: "flow",
      searchQuery: "",
      hideTag: null,
      focusNodeId: null,
      focusEnabled: true,  // focus-режим включён по умолчанию
      backgroundLayer: null,
      backgroundMoving: false,
      sourceMarkdown: "",
      readerOpen: false,
      readerTarget: null,

      // ====== S1-D: SVO triplets cache ======
      svoTriplets: [],
      setSvoTriplets: (t) => set({ svoTriplets: t }),

      addNode: (type, position) => {
        const cfg = NODE_TYPES[type];
        const id = uid(type.slice(0, 2));
        const pos =
          position ??
          {
            x: 200 + Math.random() * 300,
            y: 200 + Math.random() * 200,
          };
        const node: LitNode = {
          id,
          type,
          position: pos,
          data: {
            title: `${cfg.singular} без названия`,
            body: cfg.defaultBody,
            type,
            tags: [],
            meta: {},
          },
        };
        set((s) => ({ nodes: [...s.nodes, node], selectedNodeId: id }));
        return id;
      },

      updateNode: (id, patch) =>
        set((s) => ({
          nodes: s.nodes.map((n) => (n.id === id ? { ...n, ...patch } : n)),
        })),

      updateNodeData: (id, patch) =>
        set((s) => ({
          nodes: s.nodes.map((n) =>
            n.id === id ? { ...n, data: { ...n.data, ...patch } } : n
          ),
        })),

      updateNodeMeta: (id, patch) =>
        set((s) => ({
          nodes: s.nodes.map((n) =>
            n.id === id
              ? { ...n, data: { ...n.data, meta: { ...n.data.meta, ...patch } } }
              : n
          ),
        })),

      deleteNode: (id) =>
        set((s) => ({
          nodes: s.nodes.filter((n) => n.id !== id),
          edges: s.edges.filter((e) => e.source !== id && e.target !== id),
          selectedNodeId: s.selectedNodeId === id ? null : s.selectedNodeId,
          editingNodeId: s.editingNodeId === id ? null : s.editingNodeId,
        })),

      duplicateNode: (id) => {
        const n = get().nodes.find((x) => x.id === id);
        if (!n) return;
        const newId = uid(n.type.slice(0, 2));
        const newNode: LitNode = {
          ...n,
          id: newId,
          position: { x: n.position.x + 40, y: n.position.y + 40 },
          data: {
            ...n.data,
            title: `${n.data.title} (копия)`,
          },
        };
        set((s) => ({ nodes: [...s.nodes, newNode], selectedNodeId: newId }));
      },

      setNodes: (nodes) => set({ nodes }),
      setEdges: (edges) => set({ edges }),

      onNodesChange: (changes) => {
        // Применяем изменения от React Flow
        set((s) => {
          let nodes = s.nodes;
          for (const ch of changes) {
            if (ch.type === "position" && ch.position) {
              nodes = nodes.map((n) =>
                n.id === ch.id
                  ? {
                      ...n,
                      position: ch.position!,
                      // dragging флаг не храним
                    }
                  : n
              );
            } else if (ch.type === "remove") {
              nodes = nodes.filter((n) => n.id !== ch.id);
            } else if (ch.type === "replace" && ch.item) {
              nodes = nodes.map((n) => (n.id === ch.id ? (ch.item as LitNode) : n));
            }
          }
          // Удаляем также рёбра, связанные с удалёнными нодами
          const removedIds = changes
            .filter((c: any) => c.type === "remove")
            .map((c: any) => c.id);
          const edges =
            removedIds.length > 0
              ? s.edges.filter(
                  (e) => !removedIds.includes(e.source) && !removedIds.includes(e.target)
                )
              : s.edges;
          return { nodes, edges };
        });
      },

      onEdgesChange: (changes) => {
        set((s) => {
          let edges = s.edges;
          for (const ch of changes) {
            if (ch.type === "remove") {
              edges = edges.filter((e) => e.id !== ch.id);
            }
          }
          return { edges };
        });
      },

      onConnect: (conn) => {
        const kind = get().defaultEdgeKind;
        const edge: LitEdge = {
          id: uid("e"),
          source: conn.source,
          target: conn.target,
          sourceHandle: conn.sourceHandle ?? null,
          targetHandle: conn.targetHandle ?? null,
          type: "smoothstep",
          animated: kind === "flow" || kind === "foreshadow",
          data: { kind },
        };
        set((s) => ({ edges: [...s.edges, edge] }));
      },

      addEdge: (edge) => set((s) => ({ edges: [...s.edges, edge] })),
      updateEdge: (id, patch) =>
        set((s) => ({
          edges: s.edges.map((e) => (e.id === id ? { ...e, ...patch } : e)),
        })),
      deleteEdge: (id) =>
        set((s) => ({
          edges: s.edges.filter((e) => e.id !== id),
          selectedEdgeId: s.selectedEdgeId === id ? null : s.selectedEdgeId,
        })),

      setSelectedNode: (id) => set({
        selectedNodeId: id,
        selectedEdgeId: null,
        // При выборе ноды — автоматически фокусируемся на ней (если focus включён)
        focusNodeId: id,
      }),
      setSelectedEdge: (id) => set({ selectedEdgeId: id, selectedNodeId: null }),
      setEditingNode: (id) => set({ editingNodeId: id }),
      setDefaultEdgeKind: (k) => set({ defaultEdgeKind: k }),
      setSearchQuery: (q) => set({ searchQuery: q }),
      setHideTag: (tag) => set({ hideTag: tag }),
      setFocusNode: (id) => set({ focusNodeId: id }),
      setFocusEnabled: (enabled) => set({ focusEnabled: enabled }),
      setProjectMeta: (patch) => set(patch),

      setSourceMarkdown: (text) => set({ sourceMarkdown: text }),

      // ====== Reader mode ======
      openReader: (target) =>
        set({ readerOpen: true, readerTarget: target }),
      closeReader: () => set({ readerOpen: false }),
      setReaderIndex: (index) =>
        set((s) =>
          s.readerTarget && index >= 0 && index < s.readerTarget.moments.length
            ? { readerTarget: { ...s.readerTarget, currentIndex: index } }
            : {}
        ),

      // ====== Фоновый слой ======
      setBackgroundLayer: (layer) => set({ backgroundLayer: layer }),

      updateBackgroundLayer: (patch) =>
        set((s) =>
          s.backgroundLayer
            ? { backgroundLayer: { ...s.backgroundLayer, ...patch } }
            : {}
        ),

      clearBackgroundLayer: () => set({ backgroundLayer: null, backgroundMoving: false }),

      toggleBackgroundVisibility: () =>
        set((s) =>
          s.backgroundLayer
            ? { backgroundLayer: { ...s.backgroundLayer, visible: !s.backgroundLayer.visible } }
            : {}
        ),

      setBackgroundMoving: (moving) => set({ backgroundMoving: moving }),

      newProject: () => {
        const empty = createDefaultProject();
        set({
          title: "Новый проект",
          author: get().author,
          description: "",
          nodes: [],
          edges: [],
          selectedNodeId: null,
          selectedEdgeId: null,
          editingNodeId: null,
          backgroundLayer: null,
          backgroundMoving: false,
          sourceMarkdown: "",
          readerOpen: false,
          readerTarget: null,
        });
        void empty; // заглушка, чтобы TS не ругался
      },

      loadProject: (p, sourceMarkdown) =>
        set({
          title: p.title,
          author: p.author,
          description: p.description,
          nodes: p.nodes,
          edges: p.edges,
          selectedNodeId: null,
          selectedEdgeId: null,
          editingNodeId: null,
          sourceMarkdown: sourceMarkdown ?? "",
        }),

      exportProject: () => {
        const s = get();
        return {
          title: s.title,
          author: s.author,
          description: s.description,
          nodes: s.nodes,
          edges: s.edges,
          createdAt: Date.now(),
          updatedAt: Date.now(),
        };
      },

      getVisibleNodes: () => {
        const { nodes, searchQuery, hideTag } = get();
        const q = searchQuery.trim().toLowerCase();
        return nodes.filter((n) => {
          if (hideTag && n.data.tags?.includes(hideTag)) return false;
          if (!q) return true;
          return (
            n.data.title.toLowerCase().includes(q) ||
            n.data.body.toLowerCase().includes(q) ||
            n.data.tags?.some((t) => t.toLowerCase().includes(q))
          );
        });
      },

      getAllTags: () => {
        const tags = new Set<string>();
        get().nodes.forEach((n) => n.data.tags?.forEach((t) => tags.add(t)));
        return Array.from(tags).sort();
      },

      // ====== Версионирование ======
      saveVersion: (nodeId, label, source = "manual") => {
        const node = get().nodes.find((n) => n.id === nodeId);
        if (!node) return;
        const fullText = node.data.fullText || "";
        if (!fullText.trim()) return; // не сохраняем пустые

        const wordCount = fullText.split(/\s+/).filter(Boolean).length;
        const version: ChapterVersion = {
          id: uid("v"),
          timestamp: Date.now(),
          fullText,
          wordCount,
          label: label || `Версия от ${new Date().toLocaleString("ru-RU")}`,
          source,
        };

        set((s) => ({
          nodes: s.nodes.map((n) =>
            n.id === nodeId
              ? {
                  ...n,
                  data: {
                    ...n.data,
                    versions: [version, ...(n.data.versions || [])].slice(0, 50), // максимум 50 версий
                  },
                }
              : n
          ),
        }));
      },

      restoreVersion: (nodeId, versionId) => {
        const node = get().nodes.find((n) => n.id === nodeId);
        if (!node) return;
        const version = node.data.versions?.find((v) => v.id === versionId);
        if (!version) return;

        // Сначала сохраним текущее состояние как версию (чтобы можно было откатиться обратно)
        const currentText = node.data.fullText || "";
        if (currentText.trim()) {
          get().saveVersion(nodeId, `Перед откатом к версии от ${new Date(version.timestamp).toLocaleString("ru-RU")}`, "restore");
        }

        // Применяем выбранную версию
        set((s) => ({
          nodes: s.nodes.map((n) =>
            n.id === nodeId
              ? {
                  ...n,
                  data: {
                    ...n.data,
                    fullText: version.fullText,
                    body: (version.fullText.slice(0, 400) + (version.fullText.length > 400 ? "…" : "")),
                    meta: {
                      ...(n.data.meta || {}),
                      wordCount: version.wordCount,
                    },
                  },
                }
              : n
          ),
        }));
      },

      deleteVersion: (nodeId, versionId) => {
        set((s) => ({
          nodes: s.nodes.map((n) =>
            n.id === nodeId && n.data.versions
              ? {
                  ...n,
                  data: {
                    ...n.data,
                    versions: n.data.versions.filter((v) => v.id !== versionId),
                  },
                }
              : n
          ),
        }));
      },

      getVersions: (nodeId) => {
        const node = get().nodes.find((n) => n.id === nodeId);
        return node?.data.versions || [];
      },
    }),
    {
      name: "litgraph-store-v1",
      // Хранилище: в Tauri используем localStorage (через persistedState),
      // потом синхронизируем с Tauri store при необходимости
      storage: typeof window !== "undefined" && window.localStorage
        ? createJSONStorage(() => window.localStorage)
        : undefined,
      // Не персистим UI-состояние
      partialize: (s) => ({
        title: s.title,
        author: s.author,
        description: s.description,
        nodes: s.nodes,
        edges: s.edges,
        defaultEdgeKind: s.defaultEdgeKind,
        focusEnabled: s.focusEnabled,
        // Фон сохраняем только если src не слишком большой (<5 MB base64),
        // чтобы не переполнять localStorage (обычно ~5-10 MB лимит).
        // Очень большие растры пользователь должен ре-импортировать.
        backgroundLayer:
          s.backgroundLayer && s.backgroundLayer.src.length < 5_000_000
            ? s.backgroundLayer
            : null,
      }),
      // При первом запуске загружаем демо-данные
      onRehydrateStorage: () => (state) => {
        if (state && state.nodes.length === 0) {
          const demo = createDefaultProject();
          state.title = demo.title;
          state.author = demo.author;
          state.description = demo.description;
          state.nodes = demo.nodes;
          state.edges = demo.edges;
        }
      },
    }
  )
);

// Экспортируем утилиту uid для использования в компонентах
export { uid };
