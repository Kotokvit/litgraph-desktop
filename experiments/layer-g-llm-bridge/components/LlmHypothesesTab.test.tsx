/**
 * LlmHypothesesTab render tests (Phase 3.8 / G.3.8).
 *
 * Verifies that the Layer G UI tab:
 * - Renders the "no paradoxes" empty state when paradoxes=[].
 * - Renders the "no AI provider" prompt when aiProviderConfig is null.
 * - Renders paradox cards when paradoxes are provided.
 */

import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, cleanup } from "@testing-library/react";
import { LlmHypothesesTab } from "./LlmHypothesesTab";
import type { ParadoxDto } from "@/lib/tauri-commands";
import { useLitStore } from "@/lib/litgraph/store";

// Reset Zustand store between tests so aiProviderConfig doesn't leak.
beforeEach(() => {
  cleanup();
  useLitStore.getState().setAiProviderConfig(null);
});

const sampleParadox: ParadoxDto = {
  id: "px-test-001",
  kind: "dead_speaking",
  character: "Петро",
  chapterIdx: 5,
  originChapterIdx: 2,
  explanation: "Character 'Петро' speaks in chapter 5 but died in chapter 2.",
  evidenceText: ["…Петро помер у бою…", "…Петро сказав останнє слово…"],
};

describe("LlmHypothesesTab", () => {
  it("renders the 'no AI provider' prompt when aiProviderConfig is null", () => {
    render(<LlmHypothesesTab paradoxes={[sampleParadox]} />);
    expect(screen.getByText(/AI-провайдер не налаштований/i)).toBeInTheDocument();
    expect(screen.getByText(/Налаштувати AI/)).toBeInTheDocument();
  });

  it("renders the 'no paradoxes' empty state when paradoxes=[]", () => {
    useLitStore.getState().setAiProviderConfig({
      type: "ollama",
      url: "http://localhost:11434",
      model: "llama3.1",
    });
    render(<LlmHypothesesTab paradoxes={[]} />);
    expect(screen.getByText(/Парадоксів не виявлено/i)).toBeInTheDocument();
  });

  it("renders a paradox card with the character name when paradoxes are provided", () => {
    useLitStore.getState().setAiProviderConfig({
      type: "ollama",
      url: "http://localhost:11434",
      model: "llama3.1",
    });
    render(<LlmHypothesesTab paradoxes={[sampleParadox]} />);
    expect(screen.getByText("Петро")).toBeInTheDocument();
    expect(screen.getByText(/Гіпотези/)).toBeInTheDocument();
  });

  it("shows the paradox explanation", () => {
    useLitStore.getState().setAiProviderConfig({
      type: "ollama",
      url: "http://localhost:11434",
      model: "llama3.1",
    });
    render(<LlmHypothesesTab paradoxes={[sampleParadox]} />);
    expect(
      screen.getByText(/Character 'Петро' speaks in chapter 5/i)
    ).toBeInTheDocument();
  });

  it("shows the Layer G informational banner", () => {
    useLitStore.getState().setAiProviderConfig({
      type: "ollama",
      url: "http://localhost:11434",
      model: "llama3.1",
    });
    render(<LlmHypothesesTab paradoxes={[sampleParadox]} />);
    expect(screen.getByText(/Layer G — LLM Reasoning Bridge/i)).toBeInTheDocument();
  });
});
