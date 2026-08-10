// Хелперы для вызова Tauri commands из React.
// Перенести из прототипа (заменить fetch на invoke).

import { invoke } from "@tauri-apps/api/core";

// ====== Парсер .md ======
export async function parseMd(markdown: string, projectTitle: string, author: string) {
  return invoke("parse_md", {
    params: { markdown, projectTitle, author },
  });
}

// v0.4.0: Полный авто-пайплайн — Rust + NER merge.
// Запускается при импорте .md, возвращает ParseResult + NER metadata.
export interface FullParseResult {
  parseResult: unknown;
  nerEntities: unknown | null;
  nerMerged: boolean;
  pipelineVersion: string;
}

export async function parseMdFull(
  markdown: string,
  projectTitle: string,
  author: string
): Promise<FullParseResult> {
  return invoke("parse_md_full", {
    params: { markdown, projectTitle, author },
  });
}

// ====== Проекты ======
export async function listProjects() {
  return invoke("list_projects");
}

export async function loadProject(id: string) {
  return invoke("load_project", { id });
}

export async function saveProject(id: string, project: unknown) {
  return invoke("save_project", { id, project });
}

export async function deleteProject(id: string) {
  return invoke("delete_project", { id });
}

// ====== Версии ======
export async function saveVersion(projectId: string, nodeId: string, label?: string, source?: string) {
  return invoke("save_version", { projectId, nodeId, label, source });
}

export async function restoreVersion(projectId: string, nodeId: string, versionId: string) {
  return invoke("restore_version", { projectId, nodeId, versionId });
}

export async function deleteVersion(projectId: string, nodeId: string, versionId: string) {
  return invoke("delete_version", { projectId, nodeId, versionId });
}

export async function listVersions(projectId: string, nodeId: string) {
  return invoke("list_versions", { projectId, nodeId });
}

// ====== Экспорт ======
export async function exportProject(project: unknown, format: "json" | "text" | "markdown", path: string) {
  return invoke("export_project", { project, format, path });
}

// ====== AI ======
export async function aiAssistant(
  project: unknown,
  message: string,
  history: unknown[],
  selectedNodeId: string | null,
  provider: unknown
) {
  return invoke("ai_assistant", { project, message, history, selectedNodeId, provider });
}

export async function aiContinueChapter(
  project: unknown,
  fromChapterId: string | null,
  customPrompt: string | null,
  provider: unknown
) {
  return invoke("ai_continue_chapter", { project, fromChapterId, customPrompt, provider });
}

export async function aiAnalyzePlot(project: unknown, focus: string, provider: unknown) {
  return invoke("ai_analyze_plot", { project, focus, provider });
}

export async function aiTestConnection(provider: unknown) {
  return invoke("ai_test_connection", { provider });
}

export async function aiListOllamaModels(url: string) {
  return invoke("ai_list_ollama_models", { url });
}

// ====== Reasoning Engine (Wave 5) ======

// Примитивные типы reasoning engine (зеркалируют Rust-структуры).
// Полные типы см. в src-tauri/src/reasoning/.

// Rust enum FactValue сериализуется как externally tagged:
//   - unit-вариант Unknown → строка "Unknown"
//   - newtype-варианты (Bool/Str/Int/Float/EntityRef) → { Tag: value }
//   - List → { List: [...] }
// EntityRef помечен #[serde(rename = "Entity", alias = "EntityRef")] в Rust,
// поэтому на проводе ключ "Entity".
export type FactValue =
  | { Bool: boolean }
  | { Str: string }
  | { Int: number }
  | { Float: number }
  | { Entity: string }
  | { List: FactValue[] }
  | "Unknown";

export interface TemporalAnchor {
  chapterNum: number;
  chapterSuffix: string | null;
  sceneIndex: number | null;
  charOffset: number;
}

// Rust enum Provenance — все варианты unit, поэтому сериализуются как
// bare strings: "SvoParser", "RustParser", "LlmSuggested", "Verified", "User".
export type Provenance =
  | "SvoParser"
  | "RustParser"
  | "LlmSuggested"
  | "Verified"
  | "User";

// Rust enum Action сериализуется как externally tagged:
//   - unit-варианты → строка "Kill", "Die", ...
//   - struct-варианты → { "Variant": { field: value } }
export type Action =
  | "Kill"
  | "Wound"
  | "Hit"
  | "Capture"
  | "Imprison"
  | "Free"
  | "Heal"
  | "Touch"
  | "Die"
  | "Resurrect"
  | { Move: { destination: string } }
  | { Arrive: { destination: string } }
  | { Leave: { source: string } }
  | { Speak: { topic: string | null } }
  | { Ask: { topic: string } }
  | { Tell: { topic: string; to: string } }
  | { Marry: { partner: string } }
  | { Betray: { victim: string } }
  | { Ally: { partner: string } }
  | { Know: { fact: string } }
  | { Forget: { fact: string } }
  | { Want: { goal: string } }
  | { Plan: { goal: string } }
  | { FallInLove: { partner: string } }
  | { Hate: { target: string } }
  | { Discover: { fact: string } }
  | { Transform: { newForm: string } }
  | { Custom: { verbLemma: string; polarity: string } };

export interface Event {
  id: number;
  actor: string;
  action: Action;
  target: string | null;
  instrument: string | null;
  time: TemporalAnchor;
  sourceText: string;
  confidence: number;
  provenance: Provenance;
}

export interface ConstraintViolation {
  // Поля зависят от конкретного нарушения — оставляем как unknown.
  // Фронтенд рендерит как JSON / Debug.
  [key: string]: unknown;
}

export interface TemporalParadox {
  description: string;
  [key: string]: unknown;
}

export interface CycleReport {
  eventsProcessed: number;
  factsAsserted: number;
  violations: ConstraintViolation[];
  temporalParadoxes: TemporalParadox[];
  hypothesesGenerated: number;
  hypothesesAccepted: number;
  finalStateSnapshot: {
    current: Record<string, Record<string, FactValue>>;
    now: TemporalAnchor;
  };
}

export interface CharacterState {
  id: string;
  title: string;
  attributes: Record<string, FactValue>;
  isAlive: boolean | null;
  location: string | null;
}

export interface WorldStateView {
  now: TemporalAnchor;
  snapshot: {
    current: Record<string, Record<string, FactValue>>;
    now: TemporalAnchor;
  };
  characters: CharacterState[];
  events: Event[];
  history: unknown[];
  violationCount: number;
  paradoxCount: number;
}

export type ValidationResultDto =
  | {
      kind: "accept";
      events: Event[];
      violations: ConstraintViolation[];
      paradoxes: TemporalParadox[];
    }
  | {
      kind: "reject";
      violations: ConstraintViolation[];
      feedbackPrompt: string;
    }
  | {
      kind: "retry";
      reason: string;
    };

// Команды

export async function reasoningExtractEvents(
  text: string,
  project: unknown
): Promise<Event[]> {
  return invoke("reasoning_extract_events", { text, project });
}

export async function reasoningRunCycle(
  project: unknown,
  events: Event[]
): Promise<CycleReport> {
  return invoke("reasoning_run_cycle", { project, events });
}

export async function reasoningGetWorldState(
  project: unknown,
  events: Event[]
): Promise<WorldStateView> {
  return invoke("reasoning_get_world_state", { project, events });
}

export async function reasoningValidateText(
  project: unknown,
  events: Event[],
  proposedText: string
): Promise<ValidationResultDto> {
  return invoke("reasoning_validate_text", { project, events, proposedText });
}

// ====== POLER Layer F — v7.5-LEM Canonical Symbolic Engine ======
//
// Tauri IPC wrappers for the Rust-native POLER commands defined in
// `src-tauri/src/commands/poler.rs`. These mirror the camelCase Rust DTOs
// (#[serde(rename_all = "camelCase")]) so no field-name conversion is
// needed on the JS side.
//
// All three commands are **pure & deterministic** — no I/O, no LLM, no
// global mutable state. Safe to invoke in tight loops.
//
// Source of truth: src-tauri/src/commands/poler.rs (EpsilonClimaxDto,
// SvoTripletDto, ParadoxDto, ChapterBreakdownDto, ParadoxReportDto).

/**
 * Per-chapter ε_climax result with Layer E conflict metrics.
 *
 * Fields mirror Rust `EpsilonClimaxDto` exactly (camelCase via serde rename).
 */
export interface EpsilonClimaxDto {
  /** Raw ε_climax value (unnormalized). */
  epsilon: number;
  /** Normalized to 0–100 scale (relative to max chapter). 0 for single-chapter calls. */
  normalized: number;
  wordCount: number;
  uniqueWords: number;
  emotionCount: number;
  kwCount: number;
  canonCount: number;
  actionCount: number;
  /** Adaptive noise threshold θ_rel = 3.5/κ. */
  thetaRel: number;
  /** True if ε < θ_rel (chapter is "noise"). */
  isNoise: boolean;
  /** True if ε ≥ CLIMAX_THRESHOLD (chapter is a climax). */
  isClimax: boolean;
  formulaVariant: string;
  /** Conflict magnitude Ω_conf from Layer E (Frobenius norm ‖A_POS‖_F). 0.0 if no characters interact. */
  omegaConf: number;
  /** Spectral radius ρ(A_POS) — largest eigenvalue of character adjacency. */
  spectralRadius: number;
  /** Number of character nodes in the conflict graph for this chapter. */
  nodeCount: number;
  /** Number of directed edges in the conflict graph for this chapter. */
  edgeCount: number;
}

/**
 * SVO triplet (Subject-Verb-Object) extracted by the Rust-native Layer C parser.
 */
export interface SvoTripletDto {
  /** Subject / Nominative — who acts. */
  actor: string;
  /** Canonical verb lemma (e.g. "вбити" not "вбив"). */
  verb: string;
  /** Direct Object / Accusative or Genitive of Negation. null if intransitive. */
  target: string | null;
  /** Instrument case. null if absent. */
  instrument: string | null;
  /** Locative with preposition. null if absent. */
  location: string | null;
  /** false if negated with "не"/"ні". */
  polarity: boolean;
  /** Confidence in [0.0, 1.0]. */
  confidence: number;
}

/**
 * Single temporal paradox detected by Layer E ParadoxDetector.
 */
export interface ParadoxDto {
  /** "dead_speaking" | "spatial_teleportation" */
  kind: string;
  character: string;
  /** Index of the chapter where the paradox manifests. */
  chapterIdx: number;
  /** Index of the chapter where the originating event was recorded. */
  originChapterIdx: number;
  /** Human-readable explanation (Ukrainian/English). */
  explanation: string;
}

/**
 * Per-chapter breakdown returned alongside the paradox list.
 */
export interface ChapterBreakdownDto {
  chapterIdx: number;
  title: string;
  /** Number of characters detected in this chapter (Layer B). */
  characterCount: number;
  /** Number of SVO triplets extracted from this chapter (Layer C). */
  tripletCount: number;
  /** Character names detected in this chapter. */
  characters: string[];
}

/**
 * Result bundle for cmd_detect_paradoxes.
 */
export interface ParadoxReportDto {
  /** All detected paradoxes, ordered by chapterIdx ASC. */
  paradoxes: ParadoxDto[];
  /** Per-chapter breakdown (characters + triplets). */
  chapters: ChapterBreakdownDto[];
  /** Distinct character count across the whole manuscript. */
  totalCharacters: number;
  /** Total SVO triplets across the whole manuscript. */
  totalTriplets: number;
}

/**
 * Compute ε_climax for a single chapter using the canonical POLER v7.5-LEM
 * formula with real Ω_conf from Layer E NarrativeGraph.
 *
 * Formula:
 *   ε_climax = (κ · I_loc · d̄² + γ_emo · E + λ_conf · Ω_conf) / ln(e + |U|)
 *
 * @param chapterText Full chapter text (Ukrainian/Russian).
 * @param keyword Optional keyword for kw_count (currently unused in climax
 *   formula but reserved). Pass null/undefined if not needed.
 * @param kappa Sector-adaptive coefficient (default 1.0). Controls θ_rel = 3.5/κ.
 */
export async function cmdComputeEpsilonClimax(
  chapterText: string,
  keyword?: string | null,
  kappa: number = 1.0
): Promise<EpsilonClimaxDto> {
  return invoke<EpsilonClimaxDto>("cmd_compute_epsilon_climax", {
    chapterText,
    keyword: keyword ?? null,
    kappa,
  });
}

/**
 * Extract SVO triplets (Subject-Verb-Object) from text using the Rust-native
 * SvoParser (Layer C, POLER v7.5-LEM).
 *
 * Does NOT require Python, spaCy, or pymorphy3 — uses the in-process
 * lemmatizer, POS-tagger, and UD_Ukrainian-IU dependency templates.
 *
 * @param text Input text (single sentence, paragraph, or full chapter).
 * @returns One SvoTripletDto per detected triplet.
 */
export async function cmdExtractSvo(text: string): Promise<SvoTripletDto[]> {
  return invoke<SvoTripletDto[]>("cmd_extract_svo", { text });
}

/**
 * Detect temporal paradoxes in a multi-chapter manuscript.
 *
 * Splits `text` into chapters via Layer A detect_chapters, runs Layer B
 * (detect_characters) and Layer C (SvoParser) on each chapter, then feeds
 * the full ManuscriptAnalysis to Layer E ParadoxDetector.
 *
 * Currently detected paradox types:
 * - "dead_speaking" — character marked as deceased in chapter N acts/speaks in any later chapter M > N.
 * - "spatial_teleportation" — placeholder (not yet implemented in Layer E).
 *
 * @param text Full manuscript text (markdown with `^Глава N` headers or plain text).
 */
export async function cmdDetectParadoxes(text: string): Promise<ParadoxReportDto> {
  return invoke<ParadoxReportDto>("cmd_detect_paradoxes", { text });
}
