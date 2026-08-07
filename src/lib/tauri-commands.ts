// Хелперы для вызова Tauri commands из React.
// Перенести из прототипа (заменить fetch на invoke).

import { invoke } from "@tauri-apps/api/core";

// ====== Парсер .md ======
export async function parseMd(markdown: string, projectTitle: string, author: string) {
  return invoke("parse_md", {
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
