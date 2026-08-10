"use client";

/**
 * LlmHypothesesTab — 4th tab of PolerPanel (Phase 3.7 / G.3.7).
 *
 * Lets the user pick a paradox from the Paradox Feed, generate 4 canonical
 * LLM hypotheses (Flashback / DreamSequence / UnrecordedResurrection /
 * DisguisedIdentity), generate full chapter text for a chosen hypothesis,
 * and validate the proposed text against the deterministic Layer E
 * ParadoxDetector.
 *
 * Extracted into its own component (per H.6 mitigation) so PolerPanel.tsx
 * stays under 800 LOC. The tab receives the paradox list from PolerPanel
 * (which loads it via cmdDetectParadoxes) and dispatches LLM calls via
 * `@/lib/llm-bridge/api`.
 *
 * User-in-the-loop Layer G workflow:
 *   1. User clicks "🧪 Hypothesize" on a paradox.
 *   2. Tab calls `generateHypothesesForParadox(paradox)` → 4 hypothesis cards.
 *   3. User clicks "Generate Full Text" on a card.
 *   4. Tab calls `generateResolution(hypothesis)` → card shows proposed text.
 *   5. User clicks "Validate".
 *   6. Tab calls `validateResolution(text, [paradox])` → badge shows accept/reject/retry.
 *   7. If reject, card shows feedbackPrompt + "Regenerate with feedback" button.
 */

import React, { useState } from "react";
import { Loader2, FlaskConical, FileText, CheckCircle2, AlertTriangle, RotateCcw, Sparkles } from "lucide-react";
import type { ParadoxDto, HypothesisDto, ValidationOutcomeDto } from "@/lib/tauri-commands";
import {
  generateHypothesesForParadox,
  generateResolution,
  validateResolution,
  regenerateWithFeedback,
} from "@/lib/llm-bridge/api";
import { useLitStore } from "@/lib/litgraph/store";
import { toast } from "sonner";

interface LlmHypothesesTabProps {
  paradoxes: ParadoxDto[];
}

const KIND_LABELS: Record<string, string> = {
  flashback: "Flashback (спогад)",
  dreamSequence: "Dream Sequence (сон)",
  unrecordedResurrection: "Unrecorded Resurrection (воскресіння)",
  disguisedIdentity: "Disguised Identity (самозванець)",
};

const KIND_COLORS: Record<string, string> = {
  flashback: "from-amber-500/20 to-amber-700/10 border-amber-700/40",
  dreamSequence: "from-purple-500/20 to-purple-700/10 border-purple-700/40",
  unrecordedResurrection: "from-emerald-500/20 to-emerald-700/10 border-emerald-700/40",
  disguisedIdentity: "from-rose-500/20 to-rose-700/10 border-rose-700/40",
};

export const LlmHypothesesTab: React.FC<LlmHypothesesTabProps> = ({ paradoxes }) => {
  const aiProviderConfig = useLitStore((s) => s.aiProviderConfig);
  const openDialog = useLitStore((s) => s.openDialog);

  // Track hypotheses generated per paradox (keyed by paradox.id).
  const [hypothesesByParadox, setHypothesesByParadox] = useState<
    Record<string, HypothesisDto[]>
  >({});
  const [loadingParadox, setLoadingParadox] = useState<string | null>(null);
  const [loadingResolution, setLoadingResolution] = useState<string | null>(null);
  const [validating, setValidating] = useState<string | null>(null);
  const [validationResults, setValidationResults] = useState<
    Record<string, ValidationOutcomeDto>
  >({});
  const [error, setError] = useState<string | null>(null);

  async function handleHypothesize(paradox: ParadoxDto) {
    setError(null);
    setLoadingParadox(paradox.id);
    try {
      const hyps = await generateHypothesesForParadox(paradox);
      setHypothesesByParadox((prev) => ({ ...prev, [paradox.id]: hyps }));
      toast.success(`Згенеровано ${hyps.length} гіпотез для парадоксу «${paradox.character}»`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      toast.error(`Помилка генерації гіпотез: ${msg}`);
    } finally {
      setLoadingParadox(null);
    }
  }

  async function handleGenerateText(paradox: ParadoxDto, hypothesis: HypothesisDto) {
    setError(null);
    setLoadingResolution(hypothesis.id);
    try {
      const resolved = await generateResolution(hypothesis);
      // Update the hypothesis in state.
      setHypothesesByParadox((prev) => ({
        ...prev,
        [paradox.id]: (prev[paradox.id] ?? []).map((h) =>
          h.id === hypothesis.id ? resolved : h
        ),
      }));
      toast.success(`Згенеровано текст для гіпотези «${KIND_LABELS[hypothesis.kind] ?? hypothesis.kind}»`);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      toast.error(`Помилка генерації тексту: ${msg}`);
    } finally {
      setLoadingResolution(null);
    }
  }

  async function handleValidate(paradox: ParadoxDto, hypothesis: HypothesisDto) {
    if (!hypothesis.proposedText) return;
    setError(null);
    setValidating(hypothesis.id);
    try {
      const outcome = await validateResolution(hypothesis.proposedText, [paradox]);
      setValidationResults((prev) => ({ ...prev, [hypothesis.id]: outcome }));
      if (outcome.kind === "accept") {
        toast.success("Валідація пройшла — текст узгоджений зі світом твору");
      } else if (outcome.kind === "reject") {
        toast.warning("Валідація відхилила — спробуйте регенерувати з фідбеком");
      } else {
        toast.warning("Валідація просить повторити — текст порожній або нерозпізнаний");
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      toast.error(`Помилка валідації: ${msg}`);
    } finally {
      setValidating(null);
    }
  }

  async function handleRegenerate(paradox: ParadoxDto, hypothesis: HypothesisDto) {
    if (!hypothesis.proposedText) return;
    const outcome = validationResults[hypothesis.id];
    if (!outcome || outcome.kind !== "reject") return;
    setError(null);
    setLoadingResolution(hypothesis.id);
    try {
      const { hypothesis: regenerated, outcome: newOutcome } = await regenerateWithFeedback(
        hypothesis,
        outcome.feedbackPrompt,
        [paradox]
      );
      setHypothesesByParadox((prev) => ({
        ...prev,
        [paradox.id]: (prev[paradox.id] ?? []).map((h) =>
          h.id === hypothesis.id ? regenerated : h
        ),
      }));
      setValidationResults((prev) => ({ ...prev, [hypothesis.id]: newOutcome }));
      toast.success("Регенеровано з фідбеком");
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      toast.error(`Помилка регенерації: ${msg}`);
    } finally {
      setLoadingResolution(null);
    }
  }

  // ====== Render ======

  if (!aiProviderConfig) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-center space-y-4 py-12">
        <div className="text-5xl opacity-40">⚙</div>
        <div className="text-amber-300 text-sm max-w-md leading-relaxed">
          AI-провайдер не налаштований. Layer G потребує LLM для генерації гіпотез.
          Натисніть кнопку нижче, щоб обрати Ollama / OpenAI-compat / Z.ai.
        </div>
        <button
          onClick={() => openDialog("aiSettings")}
          className="px-4 py-2 bg-purple-600 hover:bg-purple-500 text-white text-sm rounded-md transition-colors"
        >
          ⚙ Налаштувати AI
        </button>
      </div>
    );
  }

  if (paradoxes.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-center space-y-4 py-12">
        <div className="text-5xl opacity-40">✓</div>
        <div className="text-slate-300 text-sm max-w-md leading-relaxed">
          Парадоксів не виявлено — Layer G не має що вирішувати.
          Запустіть детекцію парадоксів у вкладці «Paradox Feed».
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="text-xs text-slate-400 leading-relaxed bg-slate-900/60 border border-slate-800 p-3 rounded">
        <strong className="text-slate-300">Layer G — LLM Reasoning Bridge.</strong>{" "}
        Оберіть парадокс і згенеруйте 4 каноничні гіпотези (спогад / сон / воскресіння /
        самозванець). Потім згенеруйте повний текст глави і перевірте його на узгодженість
        із детермінованим Layer E ParadoxDetector.
      </div>

      {error && (
        <div className="rounded-md bg-red-950/40 border border-red-900/60 p-3 text-sm text-red-300">
          ❌ {error}
        </div>
      )}

      {paradoxes.map((paradox) => {
        const hyps = hypothesesByParadox[paradox.id] ?? [];
        const isLoadingThis = loadingParadox === paradox.id;
        return (
          <div
            key={paradox.id}
            className="bg-slate-900/60 border border-slate-800 rounded-lg p-4 space-y-3"
          >
            {/* Paradox header */}
            <div className="flex items-start justify-between gap-3">
              <div className="flex-1 min-w-0">
                <div className="flex items-center space-x-2 mb-1">
                  <span className="text-xs font-bold px-2 py-0.5 bg-rose-900/60 text-rose-200 rounded">
                    {paradox.kind === "dead_speaking"
                      ? "Dead-Speaking"
                      : paradox.kind === "spatial_teleportation"
                      ? "Spatial-Teleportation"
                      : paradox.kind}
                  </span>
                  <span className="text-sm font-semibold text-slate-200">
                    {paradox.character}
                  </span>
                </div>
                <p className="text-xs text-slate-400 leading-relaxed">
                  {paradox.explanation}
                </p>
                {paradox.evidenceText.length > 0 && (
                  <div className="mt-2 text-[11px] text-slate-500 space-y-0.5">
                    {paradox.evidenceText.slice(0, 2).map((e, i) => (
                      <div key={i} className="font-mono italic opacity-80">
                        {e}
                      </div>
                    ))}
                  </div>
                )}
              </div>
              <button
                onClick={() => handleHypothesize(paradox)}
                disabled={isLoadingThis}
                className="shrink-0 px-3 py-1.5 bg-purple-600 hover:bg-purple-500 disabled:opacity-50 text-white text-xs rounded-md transition-colors flex items-center gap-1.5"
              >
                {isLoadingThis ? (
                  <Loader2 className="w-3 h-3 animate-spin" />
                ) : (
                  <FlaskConical className="w-3 h-3" />
                )}
                {hyps.length > 0 ? "Регенерувати" : "🧪 Гіпотези"}
              </button>
            </div>

            {/* Hypothesis cards */}
            {hyps.length > 0 && (
              <div className="space-y-2 pt-2 border-t border-slate-800">
                {hyps.map((h) => {
                  const outcome = validationResults[h.id];
                  const isGeneratingText = loadingResolution === h.id;
                  const isValidating = validating === h.id;
                  const colorClass = KIND_COLORS[h.kind] ?? KIND_COLORS.flashback;
                  return (
                    <div
                      key={h.id}
                      className={`bg-gradient-to-br ${colorClass} border rounded-md p-3 space-y-2`}
                    >
                      <div className="flex items-start justify-between gap-2">
                        <div className="flex-1 min-w-0">
                          <div className="text-xs font-bold text-slate-100">
                            {KIND_LABELS[h.kind] ?? h.kind}
                          </div>
                          <div className="text-xs text-slate-300 mt-1 leading-relaxed">
                            {h.summary}
                          </div>
                          <div className="text-[11px] text-slate-400 mt-1 leading-relaxed italic">
                            {h.rationale}
                          </div>
                          <div className="text-[10px] text-slate-500 mt-1">
                            Confidence:{" "}
                            <span className="font-mono">
                              {(h.confidence * 100).toFixed(0)}%
                            </span>
                          </div>
                        </div>
                      </div>

                      {/* Proposed text */}
                      {h.proposedText && (
                        <div className="bg-slate-950/60 border border-slate-800 rounded p-2.5 mt-2">
                          <div className="text-[10px] text-slate-500 mb-1 uppercase tracking-wider">
                            Proposed chapter text
                          </div>
                          <div className="text-xs text-slate-300 max-h-40 overflow-y-auto lit-scroll whitespace-pre-wrap leading-relaxed">
                            {h.proposedText}
                          </div>
                        </div>
                      )}

                      {/* Validation result */}
                      {outcome && (
                        <div
                          className={`rounded p-2 text-xs ${
                            outcome.kind === "accept"
                              ? "bg-emerald-950/40 border border-emerald-800/60 text-emerald-200"
                              : outcome.kind === "reject"
                              ? "bg-red-950/40 border border-red-800/60 text-red-200"
                              : "bg-amber-950/40 border border-amber-800/60 text-amber-200"
                          }`}
                        >
                          <div className="flex items-center gap-1.5 font-semibold mb-1">
                            {outcome.kind === "accept" ? (
                              <>
                                <CheckCircle2 className="w-3.5 h-3.5" />
                                Accept — текст узгоджений
                              </>
                            ) : outcome.kind === "reject" ? (
                              <>
                                <AlertTriangle className="w-3.5 h-3.5" />
                                Reject — текст не вирішує парадокс
                              </>
                            ) : (
                              <>
                                <RotateCcw className="w-3.5 h-3.5" />
                                Retry — {outcome.reason}
                              </>
                            )}
                          </div>
                          {outcome.kind === "reject" && (
                            <div className="space-y-1">
                              <div className="text-[11px] opacity-80">
                                Порушення:
                              </div>
                              <ul className="text-[11px] opacity-80 list-disc list-inside space-y-0.5">
                                {outcome.violations.slice(0, 3).map((v, i) => (
                                  <li key={i}>{v}</li>
                                ))}
                              </ul>
                              <div className="text-[11px] opacity-80 mt-1">
                                Фідбек для LLM:
                              </div>
                              <div className="text-[11px] opacity-70 italic font-mono">
                                {outcome.feedbackPrompt.slice(0, 200)}
                                {outcome.feedbackPrompt.length > 200 ? "…" : ""}
                              </div>
                            </div>
                          )}
                        </div>
                      )}

                      {/* Action buttons */}
                      <div className="flex gap-2 flex-wrap">
                        {!h.proposedText && (
                          <button
                            onClick={() => handleGenerateText(paradox, h)}
                            disabled={isGeneratingText}
                            className="px-2.5 py-1 bg-cyan-700/60 hover:bg-cyan-600/60 disabled:opacity-50 text-cyan-100 text-[11px] rounded transition-colors flex items-center gap-1"
                          >
                            {isGeneratingText ? (
                              <Loader2 className="w-3 h-3 animate-spin" />
                            ) : (
                              <FileText className="w-3 h-3" />
                            )}
                            Згенерувати текст
                          </button>
                        )}
                        {h.proposedText && !outcome && (
                          <button
                            onClick={() => handleValidate(paradox, h)}
                            disabled={isValidating}
                            className="px-2.5 py-1 bg-emerald-700/60 hover:bg-emerald-600/60 disabled:opacity-50 text-emerald-100 text-[11px] rounded transition-colors flex items-center gap-1"
                          >
                            {isValidating ? (
                              <Loader2 className="w-3 h-3 animate-spin" />
                            ) : (
                              <Sparkles className="w-3 h-3" />
                            )}
                            Валідувати
                          </button>
                        )}
                        {h.proposedText && (
                          <button
                            onClick={() => handleGenerateText(paradox, h)}
                            disabled={isGeneratingText}
                            className="px-2.5 py-1 bg-slate-700/60 hover:bg-slate-600/60 disabled:opacity-50 text-slate-200 text-[11px] rounded transition-colors flex items-center gap-1"
                          >
                            <RotateCcw className="w-3 h-3" />
                            Регенерувати
                          </button>
                        )}
                        {outcome?.kind === "reject" && (
                          <button
                            onClick={() => handleRegenerate(paradox, h)}
                            disabled={isGeneratingText}
                            className="px-2.5 py-1 bg-amber-700/60 hover:bg-amber-600/60 disabled:opacity-50 text-amber-100 text-[11px] rounded transition-colors flex items-center gap-1"
                          >
                            {isGeneratingText ? (
                              <Loader2 className="w-3 h-3 animate-spin" />
                            ) : (
                              <RotateCcw className="w-3 h-3" />
                            )}
                            Регенерувати з фідбеком
                          </button>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
};

export default LlmHypothesesTab;
