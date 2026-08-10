"use client";

/**
 * PolerPanel — POLER Engine Ψ Visualizer (Layer F.2 React Frontend).
 *
 * Modal dialog with three tabs that surface the canonical POLER v7.5-LEM
 * symbolic engine (Layers A–E, exposed via Tauri IPC in Layer F.1):
 *
 *   1. ε-Climax Heatmap  — ε value, Frobenius norm ‖A_POS‖_F (Ω_conf),
 *                          spectral radius ρ(A_POS), node/edge counts,
 *                          CLIMAX / NOISE / NORMAL status badge, plus a
 *                          SVO-highlighted chapter reader.
 *   2. SVO Inspector     — table of all extracted SVO triplets with
 *                          actor / verb / target / polarity / confidence.
 *   3. Paradox Feed      — list of temporal paradoxes (dead_speaking,
 *                          spatial_teleportation) with chapter provenance.
 *
 * All three IPC calls fire in parallel on modal open via Promise.all —
 * they're pure & deterministic, so there's no rate limiting or caching.
 *
 * Web-preview fallback: if `window.__TAURI_INTERNALS__` is absent (running
 * in Vite dev server without Tauri), the panel renders a friendly notice
 * instead of attempting invoke() (which would throw).
 *
 * Architectural spec:
 *   POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md
 *
 * Source of truth for DTOs:
 *   src-tauri/src/commands/poler.rs (EpsilonClimaxDto, SvoTripletDto,
 *   ParadoxDto, ChapterBreakdownDto, ParadoxReportDto — all camelCase via
 *   #[serde(rename_all = "camelCase")]).
 */

import React, { useState, useEffect, useCallback } from "react";
import {
  cmdComputeEpsilonClimax,
  cmdExtractSvo,
  cmdDetectParadoxes,
  type EpsilonClimaxDto,
  type SvoTripletDto,
  type ParadoxReportDto,
} from "@/lib/tauri-commands";
import { SvoHighlighter } from "./SvoHighlighter";

// ============================================================================
// Props
// ============================================================================

interface PolerPanelProps {
  isOpen: boolean;
  onClose: () => void;
  /** Text of the currently selected chapter (or first chapter, or full text). */
  chapterText: string;
  /** Full manuscript text (all chapters joined). Used for paradox detection. */
  fullManuscriptText: string;
  /** 0-based chapter index for display purposes. */
  chapterIndex?: number;
}

type ActiveTab = "heatmap" | "svo" | "paradoxes";

// ============================================================================
// Helpers
// ============================================================================

/** Detect whether the Tauri IPC bridge is available (i.e. we're inside Tauri). */
function isTauriEnv(): boolean {
  if (typeof window === "undefined") return false;
  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}

/**
 * Map a paradox `kind` string from the Rust DTO to a human-readable label.
 * Falls back to the raw kind string for forward compatibility (when Layer E
 * adds new paradox types, the UI doesn't break).
 */
function paradoxKindLabel(kind: string): string {
  switch (kind) {
    case "dead_speaking":
      return "Dead-Speaking";
    case "spatial_teleportation":
      return "Spatial Teleportation";
    default:
      return kind;
  }
}

// ============================================================================
// Component
// ============================================================================

export const PolerPanel: React.FC<PolerPanelProps> = ({
  isOpen,
  onClose,
  chapterText,
  fullManuscriptText,
  chapterIndex = 0,
}) => {
  const [activeTab, setActiveTab] = useState<ActiveTab>("heatmap");
  const [epsilonData, setEpsilonData] = useState<EpsilonClimaxDto | null>(null);
  const [triplets, setTriplets] = useState<SvoTripletDto[]>([]);
  const [paradoxReport, setParadoxReport] = useState<ParadoxReportDto | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedTriplet, setSelectedTriplet] = useState<SvoTripletDto | null>(null);
  const [svoFilter, setSvoFilter] = useState<"all" | "affirmative" | "negated">("all");

  // ====== Load all POLER data on open via parallel IPC calls ======
  const loadData = useCallback(async () => {
    if (!chapterText || chapterText.trim().length === 0) {
      setError("Глава порожня — POLER не має вхідних даних для аналізу.");
      return;
    }

    if (!isTauriEnv()) {
      setError(
        "Tauri IPC недоступний у цьому середовищі (веб-превью). " +
          "Запустіть додаток через `bun tauri dev` або `cargo tauri dev`, щоб активувати POLER Engine Ψ."
      );
      return;
    }

    setLoading(true);
    setError(null);

    try {
      // Three pure & deterministic IPC calls — fire in parallel.
      // paradox detection uses the full manuscript (multi-chapter) so it
      // can correlate death markers across chapters; epsilon & SVO use the
      // currently selected chapter.
      const manuscriptForParadox =
        fullManuscriptText && fullManuscriptText.trim().length > 0
          ? fullManuscriptText
          : chapterText;

      const [eps, svoList, pdxReport] = await Promise.all([
        cmdComputeEpsilonClimax(chapterText),
        cmdExtractSvo(chapterText),
        cmdDetectParadoxes(manuscriptForParadox),
      ]);

      setEpsilonData(eps);
      setTriplets(svoList);
      setParadoxReport(pdxReport);
    } catch (err) {
      console.error("[PolerPanel] Tauri IPC failed:", err);
      setError(
        `Помилка POLER IPC: ${err instanceof Error ? err.message : String(err)}. ` +
          "Перевірте, що src-tauri/src/commands/poler.rs зареєстрований у invoke_handler()."
      );
    } finally {
      setLoading(false);
    }
  }, [chapterText, fullManuscriptText]);

  useEffect(() => {
    if (!isOpen) return;
    void loadData();
  }, [isOpen, loadData]);

  // Reset state when modal closes — prevents stale data flashing on reopen.
  useEffect(() => {
    if (!isOpen) {
      setEpsilonData(null);
      setTriplets([]);
      setParadoxReport(null);
      setError(null);
      setSelectedTriplet(null);
      setActiveTab("heatmap");
      setSvoFilter("all");
    }
  }, [isOpen]);

  // Close on Escape key — standard modal UX.
  useEffect(() => {
    if (!isOpen) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  // ====== Derived display values ======
  const paradoxCount = paradoxReport?.paradoxes.length ?? 0;
  const filteredTriplets = triplets.filter((t) => {
    if (svoFilter === "affirmative") return t.polarity;
    if (svoFilter === "negated") return !t.polarity;
    return true;
  });

  // ====== Render ======
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="poler-panel-title"
    >
      <div className="bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl w-full max-w-5xl h-[85vh] flex flex-col overflow-hidden">
        {/* ====== Header ====== */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900/50">
          <div className="flex items-center space-x-3">
            <span
              id="poler-panel-title"
              className="text-xl font-bold bg-gradient-to-r from-purple-400 via-amber-400 to-cyan-400 bg-clip-text text-transparent"
            >
              POLER Engine Ψ
            </span>
            <span className="text-xs px-2 py-0.5 rounded bg-purple-950/60 text-purple-300 border border-purple-800/40">
              UA-LP v7.5-LEM
            </span>
            {chapterIndex !== undefined && (
              <span className="text-xs text-slate-500">
                Chapter {chapterIndex + 1}
              </span>
            )}
          </div>

          {/* Tab Controls */}
          <div className="flex bg-slate-800/80 p-1 rounded-lg border border-slate-700/50">
            <button
              onClick={() => setActiveTab("heatmap")}
              className={`px-4 py-1.5 text-xs font-medium rounded-md transition-all ${
                activeTab === "heatmap"
                  ? "bg-purple-600 text-white shadow-lg"
                  : "text-slate-400 hover:text-slate-200"
              }`}
            >
              ε-Climax Heatmap
            </button>
            <button
              onClick={() => setActiveTab("svo")}
              className={`px-4 py-1.5 text-xs font-medium rounded-md transition-all ${
                activeTab === "svo"
                  ? "bg-purple-600 text-white shadow-lg"
                  : "text-slate-400 hover:text-slate-200"
              }`}
            >
              SVO Inspector ({triplets.length})
            </button>
            <button
              onClick={() => setActiveTab("paradoxes")}
              className={`px-4 py-1.5 text-xs font-medium rounded-md transition-all flex items-center space-x-1 ${
                activeTab === "paradoxes"
                  ? "bg-purple-600 text-white shadow-lg"
                  : "text-slate-400 hover:text-slate-200"
              }`}
            >
              <span>Paradox Feed</span>
              {paradoxCount > 0 && (
                <span className="ml-1.5 px-1.5 py-0.2 bg-rose-500 text-white rounded-full text-[10px] font-bold">
                  {paradoxCount}
                </span>
              )}
            </button>
          </div>

          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white transition-colors p-1 rounded hover:bg-slate-800"
            aria-label="Close POLER panel"
          >
            ✕
          </button>
        </div>

        {/* ====== Content Body ====== */}
        <div className="flex-1 overflow-y-auto p-6 bg-slate-950/40">
          {error ? (
            <div className="flex flex-col items-center justify-center h-full text-center space-y-4">
              <div className="text-5xl">⚠️</div>
              <div className="text-amber-300 text-sm max-w-md leading-relaxed">
                {error}
              </div>
              <button
                onClick={() => void loadData()}
                className="px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white text-sm rounded-md transition-colors"
              >
                Спробувати знову
              </button>
            </div>
          ) : loading ? (
            <div className="flex items-center justify-center h-full text-slate-400 space-x-3">
              <div className="w-5 h-5 border-2 border-purple-500 border-t-transparent rounded-full animate-spin" />
              <span>Analyzing Ukrainian Symbolic Physics...</span>
            </div>
          ) : (
            <>
              {/* ====== TAB 1: ε-Climax Heatmap & Metrics ====== */}
              {activeTab === "heatmap" && epsilonData && (
                <div className="space-y-6">
                  {/* Metric cards row */}
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                    {/* ε Climax */}
                    <div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">
                      <div className="text-xs text-slate-400">
                        Epsilon Climax (ε)
                      </div>
                      <div className="text-2xl font-bold text-amber-400">
                        {epsilonData.epsilon.toFixed(3)}
                      </div>
                      <div className="text-[10px] text-slate-500">
                        Threshold: ≥ 7.50 | θ_rel: {epsilonData.thetaRel.toFixed(2)}
                      </div>
                    </div>

                    {/* Ω_conf (Frobenius norm) */}
                    <div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">
                      <div className="text-xs text-slate-400">
                        Frobenius Norm ‖A‖<sub>F</sub>
                      </div>
                      <div className="text-2xl font-bold text-cyan-400">
                        {epsilonData.omegaConf.toFixed(3)}
                      </div>
                      <div className="text-[10px] text-slate-500">
                        Ω_conf — conflict magnitude
                      </div>
                    </div>

                    {/* Spectral radius */}
                    <div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">
                      <div className="text-xs text-slate-400">
                        Spectral Radius ρ(A)
                      </div>
                      <div className="text-2xl font-bold text-purple-400">
                        {epsilonData.spectralRadius.toFixed(3)}
                      </div>
                      <div className="text-[10px] text-slate-500">
                        Nodes: {epsilonData.nodeCount} | Edges: {epsilonData.edgeCount}
                      </div>
                    </div>

                    {/* Status badge */}
                    <div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">
                      <div className="text-xs text-slate-400">Status</div>
                      <div className="mt-1">
                        {epsilonData.isClimax ? (
                          <span className="px-2.5 py-1 bg-red-950 text-red-300 border border-red-800/60 text-xs font-bold rounded">
                            CLIMAX PEAK
                          </span>
                        ) : epsilonData.isNoise ? (
                          <span className="px-2.5 py-1 bg-slate-800 text-slate-400 text-xs rounded">
                            NOISE FILTERED
                          </span>
                        ) : (
                          <span className="px-2.5 py-1 bg-cyan-950 text-cyan-300 border border-cyan-800/60 text-xs rounded">
                            NORMAL TENSION
                          </span>
                        )}
                      </div>
                      <div className="text-[10px] text-slate-500 mt-1">
                        {epsilonData.formulaVariant}
                      </div>
                    </div>
                  </div>

                  {/* Linguistic feature counts */}
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
                    <div className="bg-slate-900/60 border border-slate-800 px-3 py-2 rounded">
                      <span className="text-slate-400">Words:</span>{" "}
                      <span className="text-slate-200 font-mono">
                        {epsilonData.wordCount}
                      </span>
                    </div>
                    <div className="bg-slate-900/60 border border-slate-800 px-3 py-2 rounded">
                      <span className="text-slate-400">Unique:</span>{" "}
                      <span className="text-slate-200 font-mono">
                        {epsilonData.uniqueWords}
                      </span>
                    </div>
                    <div className="bg-slate-900/60 border border-slate-800 px-3 py-2 rounded">
                      <span className="text-slate-400">Emotions:</span>{" "}
                      <span className="text-slate-200 font-mono">
                        {epsilonData.emotionCount}
                      </span>
                    </div>
                    <div className="bg-slate-900/60 border border-slate-800 px-3 py-2 rounded">
                      <span className="text-slate-400">Canon anchors:</span>{" "}
                      <span className="text-slate-200 font-mono">
                        {epsilonData.canonCount}
                      </span>
                    </div>
                    <div className="bg-slate-900/60 border border-slate-800 px-3 py-2 rounded">
                      <span className="text-slate-400">Action verbs:</span>{" "}
                      <span className="text-slate-200 font-mono">
                        {epsilonData.actionCount}
                      </span>
                    </div>
                    <div className="bg-slate-900/60 border border-slate-800 px-3 py-2 rounded">
                      <span className="text-slate-400">Keywords:</span>{" "}
                      <span className="text-slate-200 font-mono">
                        {epsilonData.kwCount}
                      </span>
                    </div>
                    <div className="bg-slate-900/60 border border-slate-800 px-3 py-2 rounded">
                      <span className="text-slate-400">Normalized:</span>{" "}
                      <span className="text-slate-200 font-mono">
                        {epsilonData.normalized.toFixed(2)}
                      </span>
                    </div>
                  </div>

                  {/* ε Climax bar visualization */}
                  <div className="bg-slate-900/60 border border-slate-800 p-4 rounded-lg">
                    <div className="flex items-center justify-between mb-2">
                      <h3 className="text-sm font-semibold text-slate-300">
                        ε-Climax Bar
                      </h3>
                      <span className="text-xs text-slate-500">
                        Climax threshold: 7.50
                      </span>
                    </div>
                    <div className="relative h-6 bg-slate-950 rounded overflow-hidden">
                      {/* Threshold marker line at 7.50/14 = 53.57% */}
                      <div
                        className="absolute top-0 bottom-0 w-0.5 bg-rose-500/60"
                        style={{ left: "53.57%" }}
                        title="Climax threshold (7.50)"
                      />
                      {/* ε bar — scale 0..14 for display (anything above 14 saturates) */}
                      <div
                        className={`h-full transition-all ${
                          epsilonData.isClimax
                            ? "bg-gradient-to-r from-amber-500 to-rose-500"
                            : epsilonData.isNoise
                            ? "bg-slate-700"
                            : "bg-gradient-to-r from-cyan-600 to-cyan-400"
                        }`}
                        style={{
                          width: `${Math.min(
                            100,
                            Math.max(2, (epsilonData.epsilon / 14) * 100)
                          )}%`,
                        }}
                      />
                    </div>
                    <div className="flex justify-between text-[10px] text-slate-500 mt-1">
                      <span>0</span>
                      <span>θ_rel = {epsilonData.thetaRel.toFixed(2)}</span>
                      <span>7.50 (climax)</span>
                      <span>14+</span>
                    </div>
                  </div>

                  {/* SVO Syntax Highlighted Reader */}
                  <div className="bg-slate-900/60 border border-slate-800 p-6 rounded-xl">
                    <div className="flex items-center justify-between mb-4">
                      <h3 className="text-sm font-semibold text-slate-300">
                        Chapter SVO Syntax Highlighting
                      </h3>
                      {selectedTriplet && (
                        <button
                          onClick={() => setSelectedTriplet(null)}
                          className="text-xs text-slate-500 hover:text-slate-300"
                        >
                          clear selection
                        </button>
                      )}
                    </div>
                    {selectedTriplet && (
                      <div className="mb-3 p-3 bg-purple-950/30 border border-purple-900/40 rounded text-xs space-y-1">
                        <div>
                          <span className="text-purple-300 font-semibold">
                            Actor:
                          </span>{" "}
                          <span className="text-slate-200">
                            {selectedTriplet.actor}
                          </span>
                        </div>
                        <div>
                          <span className="text-amber-300 font-semibold">
                            Verb:
                          </span>{" "}
                          <span className="text-slate-200">
                            {selectedTriplet.verb}{" "}
                            <span className="text-slate-500">
                              ({selectedTriplet.polarity ? "affirmative" : "negated"})
                            </span>
                          </span>
                        </div>
                        {selectedTriplet.target && (
                          <div>
                            <span className="text-cyan-300 font-semibold">
                              Target:
                            </span>{" "}
                            <span className="text-slate-200">
                              {selectedTriplet.target}
                            </span>
                          </div>
                        )}
                        {selectedTriplet.instrument && (
                          <div>
                            <span className="text-slate-400">Instrument:</span>{" "}
                            <span className="text-slate-200">
                              {selectedTriplet.instrument}
                            </span>
                          </div>
                        )}
                        {selectedTriplet.location && (
                          <div>
                            <span className="text-slate-400">Location:</span>{" "}
                            <span className="text-slate-200">
                              {selectedTriplet.location}
                            </span>
                          </div>
                        )}
                        <div>
                          <span className="text-slate-400">Confidence:</span>{" "}
                          <span className="text-slate-200 font-mono">
                            {(selectedTriplet.confidence * 100).toFixed(0)}%
                          </span>
                        </div>
                      </div>
                    )}
                    <SvoHighlighter
                      text={chapterText}
                      triplets={triplets}
                      onTripletSelect={setSelectedTriplet}
                    />
                  </div>
                </div>
              )}

              {/* ====== TAB 2: SVO Table Inspector ====== */}
              {activeTab === "svo" && (
                <div className="space-y-4">
                  {/* Filter row */}
                  <div className="flex items-center justify-between">
                    <div className="text-xs text-slate-400">
                      Showing{" "}
                      <span className="text-slate-200 font-mono">
                        {filteredTriplets.length}
                      </span>{" "}
                      of{" "}
                      <span className="text-slate-200 font-mono">
                        {triplets.length}
                      </span>{" "}
                      triplets
                    </div>
                    <div className="flex bg-slate-800/80 p-1 rounded-md border border-slate-700/50 text-xs">
                      {(["all", "affirmative", "negated"] as const).map((f) => (
                        <button
                          key={f}
                          onClick={() => setSvoFilter(f)}
                          className={`px-3 py-1 rounded transition-all ${
                            svoFilter === f
                              ? "bg-purple-600 text-white"
                              : "text-slate-400 hover:text-slate-200"
                          }`}
                        >
                          {f === "all"
                            ? "All"
                            : f === "affirmative"
                            ? "Affirmative"
                            : "Negated"}
                        </button>
                      ))}
                    </div>
                  </div>

                  {filteredTriplets.length === 0 ? (
                    <div className="text-center py-12 text-slate-500">
                      No SVO triplets match this filter.
                    </div>
                  ) : (
                    <div className="overflow-x-auto border border-slate-800 rounded-lg">
                      <table className="w-full text-left text-xs text-slate-300">
                        <thead className="bg-slate-900 text-slate-400 border-b border-slate-800">
                          <tr>
                            <th className="p-3">#</th>
                            <th className="p-3">Actor (Subject)</th>
                            <th className="p-3">Verb (Predicate)</th>
                            <th className="p-3">Target (Object)</th>
                            <th className="p-3">Instrument</th>
                            <th className="p-3">Location</th>
                            <th className="p-3">Polarity</th>
                            <th className="p-3">Confidence</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-slate-800/60">
                          {filteredTriplets.map((t, idx) => (
                            <tr
                              key={idx}
                              className={`hover:bg-slate-800/40 transition-colors cursor-pointer ${
                                selectedTriplet === t ? "bg-purple-950/30" : ""
                              }`}
                              onClick={() => setSelectedTriplet(t)}
                            >
                              <td className="p-3 text-slate-500 font-mono">
                                {idx + 1}
                              </td>
                              <td className="p-3 font-medium text-purple-300">
                                {t.actor}
                              </td>
                              <td className="p-3 font-semibold text-amber-400">
                                {t.verb}
                              </td>
                              <td className="p-3 text-cyan-300">
                                {t.target || "—"}
                              </td>
                              <td className="p-3 text-slate-400">
                                {t.instrument || "—"}
                              </td>
                              <td className="p-3 text-slate-400">
                                {t.location || "—"}
                              </td>
                              <td className="p-3">
                                {t.polarity ? (
                                  <span className="text-emerald-400">
                                    Affirmative
                                  </span>
                                ) : (
                                  <span className="text-rose-400">Negated</span>
                                )}
                              </td>
                              <td className="p-3 font-mono">
                                <div className="flex items-center gap-2">
                                  <div className="w-12 h-1.5 bg-slate-800 rounded-full overflow-hidden">
                                    <div
                                      className="h-full bg-emerald-500"
                                      style={{
                                        width: `${t.confidence * 100}%`,
                                      }}
                                    />
                                  </div>
                                  <span>{(t.confidence * 100).toFixed(0)}%</span>
                                </div>
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                  )}
                </div>
              )}

              {/* ====== TAB 3: Paradox Feed ====== */}
              {activeTab === "paradoxes" && paradoxReport && (
                <div className="space-y-4">
                  {/* Manuscript-level stats */}
                  <div className="grid grid-cols-3 gap-3">
                    <div className="bg-slate-900/80 border border-slate-800 p-3 rounded-lg text-center">
                      <div className="text-2xl font-bold text-rose-400">
                        {paradoxCount}
                      </div>
                      <div className="text-[10px] text-slate-500">
                        Paradoxes detected
                      </div>
                    </div>
                    <div className="bg-slate-900/80 border border-slate-800 p-3 rounded-lg text-center">
                      <div className="text-2xl font-bold text-purple-400">
                        {paradoxReport.totalCharacters}
                      </div>
                      <div className="text-[10px] text-slate-500">
                        Distinct characters
                      </div>
                    </div>
                    <div className="bg-slate-900/80 border border-slate-800 p-3 rounded-lg text-center">
                      <div className="text-2xl font-bold text-cyan-400">
                        {paradoxReport.totalTriplets}
                      </div>
                      <div className="text-[10px] text-slate-500">
                        Total SVO triplets
                      </div>
                    </div>
                  </div>

                  {/* Per-chapter breakdown */}
                  {paradoxReport.chapters.length > 0 && (
                    <div className="bg-slate-900/60 border border-slate-800 rounded-lg p-4">
                      <h3 className="text-sm font-semibold text-slate-300 mb-3">
                        Per-Chapter Breakdown
                      </h3>
                      <div className="grid grid-cols-1 md:grid-cols-2 gap-2">
                        {paradoxReport.chapters.map((ch) => (
                          <div
                            key={ch.chapterIdx}
                            className="flex items-center justify-between bg-slate-950/60 border border-slate-800 px-3 py-2 rounded text-xs"
                          >
                            <div className="flex-1 min-w-0">
                              <div className="font-medium text-slate-200 truncate">
                                Ch.{ch.chapterIdx + 1}: {ch.title}
                              </div>
                              <div className="text-[10px] text-slate-500 truncate">
                                {ch.characters.length > 0
                                  ? ch.characters.join(", ")
                                  : "(no characters detected)"}
                              </div>
                            </div>
                            <div className="flex gap-3 ml-3 shrink-0">
                              <span
                                title="Characters detected"
                                className="text-purple-300 font-mono"
                              >
                                👥 {ch.characterCount}
                              </span>
                              <span
                                title="SVO triplets"
                                className="text-cyan-300 font-mono"
                              >
                                ⚡ {ch.tripletCount}
                              </span>
                            </div>
                          </div>
                        ))}
                      </div>
                    </div>
                  )}

                  {/* Paradox list */}
                  {paradoxCount === 0 ? (
                    <div className="text-center py-12 text-slate-500">
                      <div className="text-4xl mb-2">✓</div>
                      <div className="text-sm">
                        No temporal paradoxes detected in manuscript.
                      </div>
                      <div className="text-xs text-slate-600 mt-1">
                        Layer E ParadoxDetector found no dead-speaking or
                        spatial-teleportation violations.
                      </div>
                    </div>
                  ) : (
                    <div className="space-y-3">
                      {paradoxReport.paradoxes.map((pdx, idx) => (
                        <div
                          key={idx}
                          className="bg-rose-950/20 border border-rose-900/40 p-4 rounded-lg flex justify-between items-start gap-4"
                        >
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center space-x-2 mb-1">
                              <span className="text-xs font-bold px-2 py-0.5 bg-rose-900/60 text-rose-200 rounded">
                                {paradoxKindLabel(pdx.kind)}
                              </span>
                              <span className="text-sm font-semibold text-slate-200">
                                {pdx.character}
                              </span>
                            </div>
                            <p className="text-xs text-slate-400 leading-relaxed">
                              {pdx.explanation}
                            </p>
                          </div>
                          <div className="text-xs text-slate-500 text-right shrink-0">
                            <div>
                              Origin:{" "}
                              <span className="text-slate-300">
                                Ch.{pdx.originChapterIdx + 1}
                              </span>
                            </div>
                            <div>
                              Manifest:{" "}
                              <span className="text-rose-300">
                                Ch.{pdx.chapterIdx + 1}
                              </span>
                            </div>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </>
          )}
        </div>

        {/* ====== Footer ====== */}
        <div className="px-6 py-3 border-t border-slate-800 bg-slate-900/50 text-[10px] text-slate-500 flex items-center justify-between">
          <span>
            POLER v7.5-LEM · Layers A–E (litgraph-core) → F.1 (Tauri IPC) → F.2
            (React)
          </span>
          <span className="font-mono">
            ε = κ·I_loc·d̄² + γ_emo·E + λ_conf·Ω_conf / ln(e + |U|)
          </span>
        </div>
      </div>
    </div>
  );
};

export default PolerPanel;
