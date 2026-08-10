/**
 * @file llm-bridge/api.ts — High-level Layer G API.
 *
 * Wraps the typed Tauri IPC commands in `tauri-commands.ts` with a simpler
 * interface that auto-reads `aiProviderConfig` from the Zustand store. This
 * is the primary entry point for React components that want to use Layer G.
 *
 * Usage:
 *   ```ts
 *   import { generateHypothesesForParadox } from "@/lib/llm-bridge/api";
 *   const hypotheses = await generateHypothesesForParadox(paradox);
 *   ```
 *
 * Phase 3.6 / G.3.6.
 */

import {
  cmdGenerateLlmHypotheses,
  cmdGenerateResolutionText,
  cmdValidateLlmResponse,
  type HypothesisDto,
  type ParadoxDto,
  type ValidationOutcomeDto,
} from "@/lib/tauri-commands";
import { useLitStore, type AiProviderConfig } from "@/lib/litgraph/store";

/**
 * Read the current AI provider config from the Zustand store. Throws a
 * friendly error if no provider is configured — callers should catch this
 * and prompt the user to open AiSettingsDialog.
 */
function getProviderOrThrow(): AiProviderConfig {
  const cfg = useLitStore.getState().aiProviderConfig;
  if (!cfg) {
    throw new Error(
      "AI-провайдер не налаштований. Відкрийте «Налаштування AI» (⚙ кнопка в тулбарі) та оберіть Ollama / OpenAI-compat / Z.ai."
    );
  }
  return cfg;
}

/**
 * Generate 4 canonical LLM hypotheses for a single paradox.
 *
 * Reads `aiProviderConfig` from the Zustand store and forwards to
 * `cmdGenerateLlmHypotheses`. If no provider is configured, throws an error
 * with a friendly message.
 *
 * @param paradox The paradox to resolve (must have `id`, `kind`, `character`).
 * @returns Exactly 4 hypotheses — one of each `HypothesisKind` variant.
 */
export async function generateHypothesesForParadox(
  paradox: ParadoxDto
): Promise<HypothesisDto[]> {
  const provider = getProviderOrThrow();
  return cmdGenerateLlmHypotheses(paradox, provider);
}

/**
 * Generate full resolution text for a chosen hypothesis.
 *
 * Calls the LLM with the hypothesis kind + summary + rationale and asks for
 * a 500-1500 word chapter section that resolves the paradox.
 *
 * @param hypothesis The chosen hypothesis (must have `kind`, `summary`,
 *                   `rationale`).
 * @returns A new `HypothesisDto` with `proposedText` populated.
 */
export async function generateResolution(
  hypothesis: HypothesisDto
): Promise<HypothesisDto> {
  const provider = getProviderOrThrow();
  return cmdGenerateResolutionText(hypothesis, provider);
}

/**
 * Validate LLM-proposed text against the deterministic Layer E
 * ParadoxDetector.
 *
 * This is a pure symbolic check — no LLM call. It re-runs the paradox
 * detector on the proposed text and compares against the original paradoxes.
 *
 * @param text The LLM-generated chapter text.
 * @param originalParadoxes The paradoxes that the LLM was asked to resolve
 *                          (typically the full `ParadoxReportDto.paradoxes`
 *                          list, or a filtered subset for the specific
 *                          character/kind being addressed).
 */
export async function validateResolution(
  text: string,
  originalParadoxes: ParadoxDto[]
): Promise<ValidationOutcomeDto> {
  return cmdValidateLlmResponse(text, originalParadoxes);
}

/**
 * Regenerate resolution text with feedback from a previous rejection.
 *
 * This is a convenience wrapper that takes a previously-rejected hypothesis
 * and a feedback prompt (from `ValidationOutcomeDto.Reject.feedbackPrompt`)
 * and asks the LLM to regenerate. The regenerated text is then re-validated
 * automatically.
 *
 * @param hypothesis The previously-rejected hypothesis (must have
 *                   `proposedText` populated).
 * @param feedbackPrompt The feedback from the previous validation rejection.
 * @param originalParadoxes The original paradoxes (for re-validation).
 * @returns An object with the regenerated hypothesis and the new validation
 *          outcome.
 */
export async function regenerateWithFeedback(
  hypothesis: HypothesisDto,
  feedbackPrompt: string,
  originalParadoxes: ParadoxDto[]
): Promise<{ hypothesis: HypothesisDto; outcome: ValidationOutcomeDto }> {
  const provider = getProviderOrThrow();
  // Append the feedback to the rationale so the LLM sees it in the prompt.
  const hypothesisWithFeedback: HypothesisDto = {
    ...hypothesis,
    rationale: `${hypothesis.rationale}\n\nFeedback from previous attempt:\n${feedbackPrompt}`,
  };
  const regenerated = await cmdGenerateResolutionText(hypothesisWithFeedback, provider);
  const outcome = await cmdValidateLlmResponse(
    regenerated.proposedText ?? "",
    originalParadoxes
  );
  return { hypothesis: regenerated, outcome };
}

// Re-export the DTOs so callers can import everything from one place.
export type { HypothesisDto, ParadoxDto, ValidationOutcomeDto } from "@/lib/tauri-commands";
export type { HypothesisKind } from "@/lib/tauri-commands";
