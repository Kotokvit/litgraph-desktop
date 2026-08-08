// TypeScript-зеркало Rust-структур из src-tauri/src/commands/conflict.rs.
// Сериализация в Rust идёт с #[serde(rename_all = "camelCase")],
// поэтому тут везде camelCase.

export type ConflictRole = "aggressor" | "victim" | "neutral";

export interface ConflictNode {
  /** Lemma персонажа (например, "Алексей", "Марина Игоревна"). */
  character: string;
  /** Суммарный вес исходящих действий (aggression out). */
  outgoing: number;
  /** Суммарный вес входящих действий (aggression in). */
  incoming: number;
  /** out − in: +агрессор, −жертва. */
  balance: number;
  /** Классификация по balance. */
  role: ConflictRole;
}

export interface ConflictEdge {
  /** Кто действовал (subjectLemma). */
  from: string;
  /** На ком/чём действовали (objectLemma). */
  to: string;
  /** Суммарный вес (по полярности и negated-флагу). */
  weight: number;
  /** Число сыгравших глаголов. */
  verbCount: number;
  /** Уникальные леммы глаголов. */
  verbs: string[];
  /** negative | positive | neutral. */
  polarity: "negative" | "positive" | "neutral";
  /** true если действие было negated ("не остановил"). */
  negated: boolean;
  /** true если объект был pronoun-ом, разрешённым в PER. */
  pronounResolved: boolean;
  /** Контекст предложения (обрезан ~200 символов). */
  sentence: string;
}

export interface ConflictStats {
  nodeCount: number;
  edgeCount: number;
  rawTripletCount: number;
  /** [(character, balance)] DESC — главные агрессоры. */
  aggressors: [string, number][];
  /** [(character, balance)] ASC — главные жертвы. */
  victims: [string, number][];
  /** Персонажи с |balance| < 0.1. */
  neutral: string[];
}

export interface ConflictGraph {
  nodes: ConflictNode[];
  edges: ConflictEdge[];
  /** Антисимметричная матрица J[i,j] = +w, J[j,i] = -w. */
  matrix: number[][];
  /** Исходный порядок узлов (для индексации в matrix). */
  nodeOrder: string[];
  stats: ConflictStats;
  model: string;
  version: string;
  svoVersion: string;
  textLength: number;
}

// ── UI-константы ────────────────────────────────────────────────────────

export const CONFLICT_COLORS = {
  aggressor: "#DC2626",     // красный
  aggressorFill: "#FEE2E2",
  victim: "#1D4ED8",        // синий
  victimFill: "#DBEAFE",
  neutral: "#64748B",       // серый
  neutralFill: "#F1F5F9",
  edgeAggression: "#DC2626",
  edgeNeutral: "#64748B",
  edgePositive: "#059669",
  edgeNegated: "#475569",
} as const;

export const ROLE_LABELS: Record<ConflictRole, string> = {
  aggressor: "Агрессор",
  victim: "Жертва",
  neutral: "Нейтрал",
};

export const POLARITY_LABELS: Record<ConflictEdge["polarity"], string> = {
  negative: "Агрессия",
  positive: "Помощь",
  neutral: "Нейтрально",
};
