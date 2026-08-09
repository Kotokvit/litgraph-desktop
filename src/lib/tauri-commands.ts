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

export type FactValue =
  | { Bool: boolean }
  | { Str: string }
  | { Int: number }
  | { Float: number }
  | { Entity: string }
  | { List: FactValue[] }
  | { Unknown };

export interface TemporalAnchor {
  chapterNum: number;
  chapterSuffix: string | null;
  sceneIndex: number | null;
  charOffset: number;
}

export interface Provenance {
  SvoParser?: null;
  LlmHypothesis?: null;
  Manual?: null;
  Inferred?: null;
}

export type Action =
  | "Kill"
  | "Die"
  | "Resurrect"
  | "Speak"
  | "Move"
  | "Marry"
  | "Divorce"
  | "Know"
  | "Forget"
  | { Custom: { lemma: string; polarity: string } };

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
