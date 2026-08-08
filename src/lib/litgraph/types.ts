// ====== Типы нод и данных литературного графа ======

export type LitNodeType =
  | "scene"        // Сцена
  | "character"    // Персонаж
  | "plotpoint"    // Сюжетная точка / Завязка
  | "conflict"     // Конфликт
  | "dialogue"     // Диалог
  | "location"     // Локация
  | "idea"         // Идея / Заметка
  | "chapter"      // Глава / Раздел
  | "theme";       // Тема / Мотив (сквозная идея)

export interface LitNodeData {
  title: string;
  body: string;
  type: LitNodeType;
  tags: string[];
  // meta: произвольные поля по типу ноды
  meta?: {
    pov?: string;            // точка зрения (для сцены)
    mood?: string;           // настроение
    timeOfDay?: string;      // время суток
    wordTarget?: number;     // целевой объём слов
    characterArc?: string;   // дуга персонажа
    importance?: "low" | "medium" | "high"; // важность
    manifestation?: string;  // как тема проявляется (для theme)
    [key: string]: unknown;
  };
  // Полный текст главы — для глав и сцен (необязательно)
  fullText?: string;
  // История версий полного текста (для глав и сцен)
  versions?: ChapterVersion[];
  [key: string]: unknown;
}

export interface ChapterVersion {
  id: string;
  timestamp: number;
  fullText: string;
  wordCount: number;
  label?: string;          // подпись версии (auto / manual / before-restore)
  source?: "auto" | "manual" | "ai" | "restore" | "import";
}

export type LitNode = {
  id: string;
  type: LitNodeType;
  position: { x: number; y: number };
  data: LitNodeData;
};

export type LitEdge = {
  id: string;
  source: string;
  target: string;
  sourceHandle?: string | null;
  targetHandle?: string | null;
  type?: string;
  animated?: boolean;
  label?: string;
  data?: {
    kind?: EdgeKind;
    note?: string;
    [key: string]: unknown;
  };
};

export type EdgeKind =
  | "flow"          // последовательность (поток сюжета)
  | "cause"         // причинно-следственная
  | "character"     // участие персонажа
  | "location"      // действие в локации
  | "reference"     // ссылка / упоминание
  | "conflict"      // конфликт между сущностями
  | "foreshadow"    // предзнаменование
  | "alternative"   // альтернативная ветка
  | "theme";        // тема/мотив присутствует в главе/сцене

export interface LitProject {
  title: string;
  author: string;
  description: string;
  nodes: LitNode[];
  edges: LitEdge[];
  createdAt: number;
  updatedAt: number;
}

// ====== Фоновый слой (карта / схема / диаграмма как опорный рисунок) ======

/**
 * Фоновый слой для canvas.
 *
 * Используется как опорный рисунок под графом: карта мира, план города,
 * схема сюжета, диаграмма персонажей и т.д. Автор может позиционировать
 * узлы графа поверх этого рисунка, чтобы сверять структуру с визуальным
 * замыслом.
 *
 * Поддерживаемые форматы:
 *  - SVG (предпочтителен — бесконечное масштабирование без пикселизации)
 *  - PNG (высокого разрешения)
 *  - TIFF (декодируется через utif)
 *  - JPEG / WebP (нативно через Image)
 *
 * `src` хранится как data: URL (base64). Это позволяет персистить слой
 * в localStorage вместе с проектом и не зависеть от исходного файла.
 * Для очень больших растров (>5 MB) рекомендуется SVG.
 */
export type BackgroundFormat = "svg" | "png" | "tiff" | "jpeg" | "webp" | "image";

export interface BackgroundLayer {
  /** Уникальный id слоя (для ключа в React и drag-логики) */
  id: string;
  /** data: URL (base64) или object URL — то, что можно скормить new Image() */
  src: string;
  /** Формат исходного файла (для отображения в UI) */
  format: BackgroundFormat;
  /** Имя файла (для отображения) */
  name: string;
  /** Оригинальные размеры изображения в пикселях */
  naturalWidth: number;
  naturalHeight: number;
  /** Непрозрачность 0..1 (по умолчанию 0.55 — не доминирует над графом) */
  opacity: number;
  /** Видимость слоя */
  visible: boolean;
  /** Позиция в мировых координатах canvas (верхний левый угол) */
  x: number;
  y: number;
  /** Масштаб от натуральных размеров (1 = original) */
  scale: number;
  /** Поворот в градусах (по умолчанию 0) */
  rotation: number;
  /** Если true — слой не двигается мышью и не реагирует на hit-test */
  locked: boolean;
  /** Если true — пан/зум canvas игнорирует слой (всегда виден сверху) */
  pinnedToScreen: boolean;
}

// ====== Конфигурация типов нод ======

export interface NodeTypeConfig {
  type: LitNodeType;
  label: string;          // русское название
  singular: string;       // "Сцена"
  plural: string;         // "Сцены"
  description: string;
  icon: string;           // имя lucide-иконки
  color: string;          // hex основной цвет
  accent: string;         // hex акцент
  defaultBody: string;    // шаблон содержимого
  fields: NodeField[];    // дополнительные поля
}

export interface NodeField {
  key: string;
  label: string;
  type: "text" | "textarea" | "select" | "number";
  options?: string[];
  placeholder?: string;
}

export const NODE_TYPES: Record<LitNodeType, NodeTypeConfig> = {
  scene: {
    type: "scene",
    label: "Сцена",
    singular: "Сцена",
    plural: "Сцены",
    description: "Отдельная сцена произведения: место и время действия.",
    icon: "Clapperboard",
    color: "#8B5A2B",
    accent: "#A87545",
    defaultBody:
      "Где и когда происходит сцена? Кто в ней участвует? Что меняется к концу сцены?",
    fields: [
      { key: "pov", label: "Точка зрения", type: "text", placeholder: "От чьего лица идёт повествование" },
      { key: "mood", label: "Настроение", type: "text", placeholder: "тревожное, лиричное, напряжённое…" },
      { key: "timeOfDay", label: "Время суток", type: "select", options: ["Утро", "День", "Вечер", "Ночь", "Не указано"] },
      { key: "wordTarget", label: "Цель по словам", type: "number", placeholder: "1500" },
    ],
  },
  character: {
    type: "character",
    label: "Персонаж",
    singular: "Персонаж",
    plural: "Персонажи",
    description: "Действующее лицо: герой, второстепенный, антагонист.",
    icon: "User",
    color: "#3D7068",
    accent: "#5A9489",
    defaultBody:
      "Имя, возраст, внешность. Что хочет? Что скрывает? Какая дуга персонажа?",
    fields: [
      { key: "characterArc", label: "Дуга персонажа", type: "textarea", placeholder: "Как меняется персонаж от начала к концу истории?" },
      { key: "importance", label: "Важность", type: "select", options: ["low", "medium", "high"] },
    ],
  },
  plotpoint: {
    type: "plotpoint",
    label: "Сюжетная точка",
    singular: "Сюжетная точка",
    plural: "Сюжетные точки",
    description: "Ключевое событие: завязка, перипетия, кульминация, развязка.",
    icon: "Flag",
    color: "#B8463F",
    accent: "#D26056",
    defaultBody:
      "Что происходит? Почему это важно для сюжета? Что было до и что будет после?",
    fields: [
      { key: "importance", label: "Важность", type: "select", options: ["low", "medium", "high"] },
    ],
  },
  conflict: {
    type: "conflict",
    label: "Конфликт",
    singular: "Конфликт",
    plural: "Конфликты",
    description: "Противостояние: внутреннее, межличностное, с обществом, природой.",
    icon: "Swords",
    color: "#9333EA",
    accent: "#A855F7",
    defaultBody:
      "Кто с кем конфликтует? Из-за чего? Какая ставка? К чему приведёт разрешение?",
    fields: [
      { key: "importance", label: "Важность", type: "select", options: ["low", "medium", "high"] },
    ],
  },
  dialogue: {
    type: "dialogue",
    label: "Диалог",
    singular: "Диалог",
    plural: "Диалоги",
    description: "Ключевой разговор: реплики, подтекст, цель разговора.",
    icon: "MessagesSquare",
    color: "#2563A6",
    accent: "#3B82C4",
    defaultBody:
      "Кто говорит? О чём формально? Что на самом деле обсуждают (подтекст)?",
    fields: [],
  },
  location: {
    type: "location",
    label: "Локация",
    singular: "Локация",
    plural: "Локации",
    description: "Место действия: комната, город, страна, вымышленный мир.",
    icon: "MapPin",
    color: "#65A30D",
    accent: "#84CC16",
    defaultBody:
      "Где это? Какая атмосфера? Какие детали делают место живым?",
    fields: [],
  },
  idea: {
    type: "idea",
    label: "Идея",
    singular: "Идея",
    plural: "Идеи",
    description: "Заметка, образ, тема, размышление — без жёсткой структуры.",
    icon: "Lightbulb",
    color: "#CA8A04",
    accent: "#EAB308",
    defaultBody: "Что пришло в голову? К чему это может привести?",
    fields: [],
  },
  chapter: {
    type: "chapter",
    label: "Глава",
    singular: "Глава",
    plural: "Главы",
    description: "Структурная единица произведения: объединяет несколько сцен.",
    icon: "BookOpen",
    color: "#4B5563",
    accent: "#6B7280",
    defaultBody: "О чём глава? Какие сцены в неё входят? Что меняется к концу?",
    fields: [
      { key: "wordTarget", label: "Цель по словам", type: "number", placeholder: "5000" },
    ],
  },
  theme: {
    type: "theme",
    label: "Тема",
    singular: "Тема",
    plural: "Темы и мотивы",
    description: "Сквозная тема или мотив произведения: тишина, память, предательство, взросление…",
    icon: "Sparkle",
    color: "#0D9488",
    accent: "#14B8A6",
    defaultBody:
      "Какая тема? Как она проявляется в разных главах? Какие образы её выражают? Как развивается от начала к концу?",
    fields: [
      { key: "importance", label: "Важность", type: "select", options: ["low", "medium", "high"] },
      { key: "manifestation", label: "Как проявляется", type: "textarea", placeholder: "через образы, повторяющиеся слова, поступки героев, символику…" },
    ],
  },
};

export const NODE_TYPE_ORDER: LitNodeType[] = [
  "chapter",
  "scene",
  "plotpoint",
  "conflict",
  "character",
  "dialogue",
  "location",
  "theme",
  "idea",
];

// ====== Конфигурация типов связей ======

export interface EdgeTypeConfig {
  kind: EdgeKind;
  label: string;
  description: string;
  color: string;
  dashed: boolean;
  animated: boolean;
}

export const EDGE_TYPES: Record<EdgeKind, EdgeTypeConfig> = {
  flow: {
    kind: "flow",
    label: "Поток сюжета",
    description: "Последовательность: что за чем следует.",
    color: "#8B5A2B",
    dashed: false,
    animated: true,
  },
  cause: {
    kind: "cause",
    label: "Причина → следствие",
    description: "Одно событие вызывает другое.",
    color: "#B8463F",
    dashed: false,
    animated: false,
  },
  character: {
    kind: "character",
    label: "Участие персонажа",
    description: "Персонаж участвует в сцене / событии.",
    color: "#3D7068",
    dashed: true,
    animated: false,
  },
  location: {
    kind: "location",
    label: "Место действия",
    description: "Сцена или событие происходит в локации.",
    color: "#65A30D",
    dashed: true,
    animated: false,
  },
  reference: {
    kind: "reference",
    label: "Упоминание / ссылка",
    description: "Одна часть текста ссылается на другую.",
    color: "#6B7280",
    dashed: true,
    animated: false,
  },
  conflict: {
    kind: "conflict",
    label: "Конфликт",
    description: "Противостояние между сущностями.",
    color: "#9333EA",
    dashed: false,
    animated: false,
  },
  foreshadow: {
    kind: "foreshadow",
    label: "Предзнаменование",
    description: "Подготовка будущего события.",
    color: "#CA8A04",
    dashed: true,
    animated: true,
  },
  alternative: {
    kind: "alternative",
    label: "Альтернативная ветка",
    description: "Вариант развития событий.",
    color: "#2563A6",
    dashed: true,
    animated: false,
  },
  theme: {
    kind: "theme",
    label: "Тема / мотив",
    description: "Тема проявляется в этой главе, сцене или через персонажа.",
    color: "#0D9488",
    dashed: true,
    animated: false,
  },
};
