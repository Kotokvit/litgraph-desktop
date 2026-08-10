# POLER Layer F: React Visualizer & Tauri IPC Architectural Master Specification

> **Document Version**: v8.0-CANONICAL  
> **Status**: APPROVED ARCHITECTURAL SPECIFICATION & BLUEPRINT  
> **Target System**: LitGraph Desktop v0.2.3  
> **Engine Layer**: Layer F (React Visualizer & Tauri IPC Integration)  
> **Repository Path**: `POLER_LAYER_F_FRONTEND_ARCHITECTURAL_SPECIFICATION.md`

---

## 1. Executive Summary & System Vision

### 1.1 Mission: Symbolic Ukrainian Narrative Physics (POLER Engine $\Psi$)
LitGraph Desktop's **POLER Engine** ($\Psi$) represents a state-of-the-art framework for computational narratology, mathematical textual dynamics, and narrative inconsistency analysis tailored specifically for Ukrainian literature. By combining deterministic linguistic parsing (Lemmatizer, POS Tagger, SVO Extraction) with mathematical physics formulations (Climax Epsilon $\varepsilon_{\text{climax}}$, Frobenius Matrix Norm $\|A\|_F$, Spectral Radius $\rho(A_{\text{POS}})$), POLER converts unstructured creative text into quantifiable structural metrics.

```
                  ┌────────────────────────────────────────────────────────┐
                  │                 MANUSCRIPT TEXT (.md)                  │
                  └───────────────────────────┬────────────────────────────┘
                                              │
                                              ▼
                  ┌────────────────────────────────────────────────────────┐
                  │       SYMBOLIC ENGINE CORE (litgraph-core)             │
                  │   Lemmatizer (A) → POS (B) → SVO (C) → ε v7.5 (D)      │
                  └───────────────────────────┬────────────────────────────┘
                                              │
                                              ▼
                  ┌────────────────────────────────────────────────────────┐
                  │         REASONING & NARRATIVE GRAPH (Layer E)          │
                  │   Adjacency Matrix A_POS, ||A||_F, ρ(A), Paradoxes       │
                  └───────────────────────────┬────────────────────────────┘
                                              │
                                              ▼
                  ┌────────────────────────────────────────────────────────┐
                  │          TAURI IPC BRIDGE (src-tauri Layer F.1)        │
                  │   cmd_compute_epsilon_climax | cmd_extract_svo | ...   │
                  └───────────────────────────┬────────────────────────────┘
                                              │
                                              ▼
                  ┌────────────────────────────────────────────────────────┐
                  │        REACT FRONT-END VISUALIZER (src/ Layer F.2)     │
                  │   PolerPanel (Heatmap, SVO, Paradox) | SvoHighlighter   │
                  └────────────────────────────────────────────────────────┘
```

### 1.2 The "Validate by Using" Imperative
The development of Layer F (React Visualizer) prior to Layer G (LLM Reasoning Bridge) is a fundamental software engineering requirement. The rationales for this ordering are:

1. **User-in-the-Loop Visibility**: A symbolic physics engine is useless if literary analysts cannot inspect its outputs. Visualizing narrative metrics ($\varepsilon$, SVO triplets, temporal paradoxes) allows human inspection of parsing quality.
2. **Empirical Calibration of Anomaly Thresholds**: Before prompting LLMs to resolve temporal paradoxes (Layer G), humans must see which text patterns trigger false positives versus true narrative errors (*Dead-Speaking*, *Spatial Teleportation*).
3. **API Ergonomics Validation**: Integrating Tauri Rust DTOs directly into React components verifies data payload structures, serialization speed, camelCase conversion accuracy, and component state flows.

---

## 2. End-to-End System Architecture & Data Flow

### 2.1 Complete Architectural Layer Breakdown

```mermaid
graph TD
    subgraph Layer A: Symbolic UA Lemmatizer
        UA_DICT[dict_uk 227k Lemmas] --> LEM[Lemmatizer Engine]
        UD_BANK[UD-Ukrainian 7k Sentences] --> LEM
    end

    subgraph Layer B: 3-Pass Rule-Based POS Tagger
        LEM --> POS_PASS1[Pass 1: 450 LanguageTool XML Pattern Rules]
        POS_PASS1 --> POS_PASS2[Pass 2: Case Government Consistency 37k Frames]
        POS_PASS2 --> POS_PASS3[Pass 3: Capitalization & Acronym Heuristics]
    end

    subgraph Layer C: Ukrainian SVO Triplet Extraction
        POS_PASS3 --> SVO_GRAMMAR[UD-IU Grammar Patterns]
        SVO_GRAMMAR --> SVO_PARSER[SvoParser Engine]
        SVO_PARSER --> SVO_TRIPLETS[Vec SvoTriplet]
    end

    subgraph Layer D: Epsilon Climax Formula v7.5-LEM
        SVO_TRIPLETS --> EPS_ENGINE[compute_epsilon_climax_with_analyzer]
        EPS_ENGINE --> EPS_RESULT[EpsilonResult DTO]
    end

    subgraph Layer E: Narrative Graph & Paradox Detector
        SVO_TRIPLETS --> NARR_GRAPH[NarrativeGraph Matrix Builder A_POS]
        NARR_GRAPH --> SPECTRAL[Power Iteration Spectral Radius ρ A_POS]
        NARR_GRAPH --> FROBENIUS[Frobenius Norm ||A||_F]
        SVO_TRIPLETS --> PARADOX_DET[ParadoxDetector Engine]
        PARADOX_DET --> PARADOX_VEC[Vec Paradox]
    end

    subgraph Layer F.1: Tauri IPC Commands (Rust)
        EPS_RESULT --> CMD_EPS[cmd_compute_epsilon_climax]
        FROBENIUS --> CMD_EPS
        SPECTRAL --> CMD_EPS
        SVO_TRIPLETS --> CMD_SVO[cmd_extract_svo]
        PARADOX_VEC --> CMD_PARADOX[cmd_detect_paradoxes]
    end

    subgraph Layer F.2: React Visualizer (TypeScript/React)
        CMD_EPS --> TS_API[src/lib/tauri-commands.ts]
        CMD_SVO --> TS_API
        CMD_PARADOX --> TS_API
        TS_API --> PANEL[PolerPanel.tsx]
        TS_API --> HIGHLIGHTER[SvoHighlighter.tsx]
        PANEL --> TAB1[Tab 1: Epsilon Heatmap Bar Chart]
        PANEL --> TAB2[Tab 2: SVO Triplet Table]
        PANEL --> TAB3[Tab 3: Temporal Paradox Feed]
        HIGHLIGHTER --> UI_TEXT[Syntax Highlighted Reader Editor]
    end
```

---

## 3. Mathematical & Linguistic Foundations (Layers A–E Recap)

### 3.1 Layer A: Symbolic UA Lemmatizer
- **Linguistic Corpus**: 227,051 dictionary lemmas from `dict_uk` (VESUM, LGPL), 16 affix files, 39 paradigms generating 2,234,167 unique wordforms packed into a 16.94 MB `json.gz` index.
- **Coverage**: 56%–73% native Ukrainian vocabulary coverage on full literary texts (e.g., `sfera.md`, `kasiopia.md`).

### 3.2 Layer B: POS Tagger Architecture
1. **Pass 1 (Pattern Rules)**: Executes 450 XML rules extracted from LanguageTool `disambiguation.xml`.
2. **Pass 2 (Case Government)**: Enforces grammatical case consistency across 37,728 pre-compiled case-government frames (`case_government.txt`).
3. **Pass 3 (Fallback Heuristics)**: Resolves remaining homonymy using sentence-start capitalization, ALL-CAPS acronym rules, and punctuation boundaries.

### 3.3 Layer C: SVO Triplet Extraction
Extracts $(S, V, O)$ Subject-Verb-Object triplets based on UD-Ukrainian grammatical rules:
$$\text{SVO Triplet} = \Big(\text{Actor (Noun/PropN)}, \text{Verb (Lemma)}, \text{Target (Noun/PropN opt)}, \text{Polarity (bool)}, \text{Confidence } c \in [0, 1]\Big)$$

### 3.4 Layer D: Epsilon Climax Canonical Formula v7.5-LEM
The climax metric $\varepsilon_{\text{climax}}$ measures narrative tension in a chapter text:

$$\varepsilon_{\text{climax}} = 2.0 \cdot A_{\text{SVO}}^{\text{validated}} + 1.5 \cdot \Omega_{\text{conf}} + 1.2 \cdot I_{\text{loc}}$$

Where:
- $A_{\text{SVO}}^{\text{validated}} = 2.0 \cdot \big|\{ v \in U : \text{POS}(v) = \text{Verb}_{\text{action}} \land \text{Polarity}(v) = \text{true} \land \text{SVO}_{\text{valid}}(v) \}\big|$
- $\Omega_{\text{conf}} = \|A_{\text{POS}}\|_F = \sqrt{\sum_{i=1}^n \sum_{j=1}^n |a_{ij}|^2}$ (Frobenius norm of character interaction matrix $A_{\text{POS}}$)
- $I_{\text{loc}} = 1 + \ln(1 + \text{canon\_count})$
- Climax Threshold: $\varepsilon \ge 7.5$
- Relative Noise Threshold: $\theta_{\text{rel}} = \frac{1.5}{\sqrt{\text{word\_count}}}$

### 3.5 Layer E: Narrative Graph & Paradox Detection
- **Narrative Graph Adjacency Matrix $A_{\text{POS}}$**: $n \times n$ matrix where entry $a_{ij}$ represents interaction weight between Character $i$ and Character $j$.
- **Spectral Radius $\rho(A_{\text{POS}})$**: Calculated via **Power Iteration Method**:
  $$v^{(k+1)} = \frac{A_{\text{POS}} v^{(k)}}{\|A_{\text{POS}} v^{(k)}\|_2}, \quad \rho(A_{\text{POS}}) = \lim_{k \to \infty} \frac{(v^{(k)})^T A_{\text{POS}} v^{(k)}}{(v^{(k)})^T v^{(k)}}$$
- **Dead-Speaking Paradox Detector**: Detects when a character marked deceased in chapter $N$ acts or speaks in chapter $N+k$ ($k > 0$).

---

## 4. Tauri IPC Bridge & TypeScript DTO Schemas (Layer F.1 $\to$ F.2)

### 4.1 Rust DTO Implementation (`src-tauri/src/commands/poler.rs`)

```rust
use serde::{Deserialize, Serialize};
use litgraph_core::linguistic::svo_parser::SvoTriplet;
use litgraph_core::reasoning::paradox::{Paradox, ParadoxKind};
use litgraph_core::parser::epsilon::EpsilonResult;
use litgraph_core::reasoning::mod_traits::ConflictReport;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SvoTripletDto {
    pub actor: String,
    pub verb: String,
    pub target: Option<String>,
    pub instrument: Option<String>,
    pub location: Option<String>,
    pub polarity: bool,
    pub confidence: f64,
}

impl From<SvoTriplet> for SvoTripletDto {
    fn from(t: SvoTriplet) -> Self {
        Self {
            actor: t.actor,
            verb: t.verb,
            target: t.target,
            instrument: t.instrument,
            location: t.location,
            polarity: t.polarity,
            confidence: t.confidence,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParadoxDto {
    pub kind: String,
    pub character: String,
    pub chapterIdx: usize,
    pub originChapterIdx: usize,
    pub explanation: String,
}

impl From<Paradox> for ParadoxDto {
    fn from(p: Paradox) -> Self {
        let kind_str = match p.kind {
            ParadoxKind::DeadSpeaking => "dead_speaking",
            ParadoxKind::SpatialTeleportation => "spatial_teleportation",
        };
        Self {
            kind: kind_str.to_string(),
            character: p.character,
            chapterIdx: p.chapter_idx,
            originChapterIdx: p.origin_chapter_idx,
            explanation: p.explanation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChapterParadoxBreakdownDto {
    pub chapterIdx: usize,
    pub characterCount: usize,
    pub tripletCount: usize,
    pub paradoxCount: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParadoxReportDto {
    pub totalParadoxes: usize,
    pub paradoxes: Vec<ParadoxDto>,
    pub chapterBreakdowns: Vec<ChapterParadoxBreakdownDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpsilonClimaxDto {
    pub chapterLength: usize,
    pub wordCount: usize,
    pub actionVerbCount: usize,
    pub canonAnchorCount: usize,
    pub svoValidatedCount: usize,
    pub omegaConf: f64,
    pub frobeniusNorm: f64,
    pub spectralRadius: f64,
    pub nodeCount: usize,
    pub edgeCount: usize,
    pub rawEpsilon: f64,
    pub epsilon: f64,
    pub noiseFiltered: bool,
    pub isClimax: bool,
    pub formulaVariant: String,
}

impl EpsilonClimaxDto {
    pub fn from_layers(eps: EpsilonResult, report: &ConflictReport) -> Self {
        Self {
            chapterLength: eps.chapter_length,
            wordCount: eps.word_count,
            actionVerbCount: eps.action_verb_count,
            canonAnchorCount: eps.canon_anchor_count,
            svoValidatedCount: eps.svo_validated_count,
            omegaConf: eps.omega_conf,
            frobeniusNorm: report.frobenius_norm,
            spectralRadius: report.spectral_radius,
            nodeCount: report.node_count,
            edgeCount: report.edge_count,
            rawEpsilon: eps.raw_epsilon,
            epsilon: eps.epsilon,
            noiseFiltered: eps.noise_filtered,
            isClimax: eps.is_climax,
            formulaVariant: eps.formula_variant,
        }
    }
}
```

### 4.2 TypeScript API Binding Blueprint (`src/lib/tauri-commands.ts`)

```typescript
import { invoke } from "@tauri-apps/api/core";

// ============================================================================
// POLER UA-LP Symbolic Narrative Physics Engine DTO Types (Layer F)
// ============================================================================

export interface SvoTripletDto {
  actor: string;
  verb: string;
  target: string | null;
  instrument: string | null;
  location: string | null;
  polarity: boolean;
  confidence: number;
}

export type ParadoxKind = "dead_speaking" | "spatial_teleportation";

export interface ParadoxDto {
  kind: ParadoxKind;
  character: string;
  chapterIdx: number;
  originChapterIdx: number;
  explanation: string;
}

export interface ChapterParadoxBreakdownDto {
  chapterIdx: number;
  characterCount: number;
  tripletCount: number;
  paradoxCount: number;
}

export interface ParadoxReportDto {
  totalParadoxes: number;
  paradoxes: ParadoxDto[];
  chapterBreakdowns: ChapterParadoxBreakdownDto[];
}

export interface EpsilonClimaxDto {
  chapterLength: number;
  wordCount: number;
  actionVerbCount: number;
  canonAnchorCount: number;
  svoValidatedCount: number;
  omegaConf: number;
  frobeniusNorm: number;
  spectralRadius: number;
  nodeCount: number;
  edgeCount: number;
  rawEpsilon: number;
  epsilon: number;
  noiseFiltered: boolean;
  isClimax: boolean;
  formulaVariant: string;
}

// ============================================================================
// Tauri IPC Wrapper Invocations
// ============================================================================

/**
 * Computes canonical POLER Epsilon Climax (v7.5-LEM) for chapter text.
 */
export async function cmdComputeEpsilonClimax(
  chapterText: string,
  keyword?: string,
  kappa: number = 1.0
): Promise<EpsilonClimaxDto> {
  return invoke<EpsilonClimaxDto>("cmd_compute_epsilon_climax", {
    chapterText,
    keyword: keyword || null,
    kappa,
  });
}

/**
 * Extracts Ukrainian Subject-Verb-Object triplets natively in Rust.
 */
export async function cmdExtractSvo(text: string): Promise<SvoTripletDto[]> {
  return invoke<SvoTripletDto[]>("cmd_extract_svo", { text });
}

/**
 * Runs manuscript-wide multi-chapter temporal paradox detection.
 */
export async function cmdDetectParadoxes(text: string): Promise<ParadoxReportDto> {
  return invoke<ParadoxReportDto>("cmd_detect_paradoxes", { text });
}
```

---

## 5. Front-End UI/UX Theoretical Design System & Visual Archetypes

### 5.1 Aesthetic Palette & HSL Token Specification
To achieve a premium visual appearance adhering to LitGraph Desktop's dark glassmorphic design system:

```css
/* POLER Narrative Physics Visual Tokens */
:root {
  --poler-climax-bg: hsl(0, 84%, 60%);        /* Crimson Red #EF4444 */
  --poler-climax-glow: rgba(239, 68, 68, 0.4);
  --poler-tension-bg: hsl(38, 92%, 50%);       /* Amber #F59E0B */
  --poler-normal-bg: hsl(189, 94%, 43%);       /* Cyan #06B6D4 */
  --poler-noise-bg: hsl(215, 16%, 47%);        /* Slate Gray #6B7280 */
  
  --svo-actor-bg: rgba(139, 92, 246, 0.25);     /* Violet #8B5CF6 */
  --svo-actor-border: rgba(139, 92, 246, 0.5);
  --svo-actor-text: #DDD6FE;
  
  --svo-verb-text: #FBBF24;                    /* Amber Bold #FBBF24 */
  --svo-verb-negated: #F87171;                 /* Rose Red #F87171 */
  
  --svo-target-underline: #22D3EE;             /* Cyan Underline #22D3EE */
  
  --paradox-alert-bg: rgba(244, 63, 94, 0.15);  /* Rose Danger #F43F5E */
  --paradox-alert-border: rgba(244, 63, 94, 0.4);
}
```

---

## 6. Component Specification & Implementation Blueprints

### 6.1 `src/components/litgraph/SvoHighlighter.tsx`

```tsx
import React, { useMemo } from "react";
import { SvoTripletDto } from "../../lib/tauri-commands";

interface SvoHighlighterProps {
  text: string;
  triplets: SvoTripletDto[];
  className?: string;
  onTripletSelect?: (triplet: SvoTripletDto) => void;
}

interface TokenSpan {
  text: string;
  isActor: boolean;
  isVerb: boolean;
  isTarget: boolean;
  isNegated: boolean;
  triplet?: SvoTripletDto;
}

export const SvoHighlighter: React.FC<SvoHighlighterProps> = ({
  text,
  triplets,
  className = "",
  onTripletSelect,
}) => {
  const spans = useMemo(() => {
    if (!text || triplets.length === 0) {
      return [{ text, isActor: false, isVerb: false, isTarget: false, isNegated: false }];
    }

    const actorSet = new Map<string, SvoTripletDto>();
    const verbSet = new Map<string, SvoTripletDto>();
    const targetSet = new Map<string, SvoTripletDto>();

    for (const t of triplets) {
      if (t.actor) actorSet.set(t.actor.toLowerCase(), t);
      if (t.verb) verbSet.set(t.verb.toLowerCase(), t);
      if (t.target) targetSet.set(t.target.toLowerCase(), t);
    }

    const wordsAndSpaces = text.split(/(\s+|[.,!?;:—«»"'])/);
    const result: TokenSpan[] = [];

    for (const token of wordsAndSpaces) {
      const lower = token.trim().toLowerCase();
      if (!lower) {
        result.push({ text: token, isActor: false, isVerb: false, isTarget: false, isNegated: false });
        continue;
      }

      const actorTrip = actorSet.get(lower);
      const verbTrip = verbSet.get(lower);
      const targetTrip = targetSet.get(lower);

      if (actorTrip) {
        result.push({ text: token, isActor: true, isVerb: false, isTarget: false, isNegated: false, triplet: actorTrip });
      } else if (verbTrip) {
        result.push({ text: token, isActor: false, isVerb: true, isTarget: false, isNegated: !verbTrip.polarity, triplet: verbTrip });
      } else if (targetTrip) {
        result.push({ text: token, isActor: false, isVerb: false, isTarget: true, isNegated: false, triplet: targetTrip });
      } else {
        result.push({ text: token, isActor: false, isVerb: false, isTarget: false, isNegated: false });
      }
    }

    return result;
  }, [text, triplets]);

  return (
    <div className={`font-serif text-slate-200 leading-relaxed whitespace-pre-wrap ${className}`}>
      {spans.map((span, idx) => {
        if (span.isActor) {
          return (
            <span
              key={idx}
              onClick={() => span.triplet && onTripletSelect?.(span.triplet)}
              className="bg-purple-900/40 border border-purple-500/40 text-purple-200 px-1 py-0.5 rounded cursor-pointer hover:bg-purple-800/60 transition-colors inline-block my-0.5"
              title={`Actor: ${span.triplet?.actor}`}
            >
              {span.text}
            </span>
          );
        }
        if (span.isVerb) {
          return (
            <span
              key={idx}
              onClick={() => span.triplet && onTripletSelect?.(span.triplet)}
              className={`font-semibold cursor-pointer transition-colors ${
                span.isNegated
                  ? "text-red-400 line-through decoration-red-500"
                  : "text-amber-400 hover:text-amber-300"
              }`}
              title={`Verb: ${span.triplet?.verb} (Polarity: ${span.triplet?.polarity})`}
            >
              {span.text}
            </span>
          );
        }
        if (span.isTarget) {
          return (
            <span
              key={idx}
              onClick={() => span.triplet && onTripletSelect?.(span.triplet)}
              className="underline decoration-cyan-400 decoration-2 text-cyan-200 cursor-pointer hover:text-cyan-100 transition-colors"
              title={`Target: ${span.triplet?.target}`}
            >
              {span.text}
            </span>
          );
        }
        return <span key={idx}>{span.text}</span>;
      })}
    </div>
  );
};
```

### 6.2 `src/components/litgraph/PolerPanel.tsx`

```tsx
import React, { useState, useEffect } from "react";
import {
  cmdComputeEpsilonClimax,
  cmdExtractSvo,
  cmdDetectParadoxes,
  EpsilonClimaxDto,
  SvoTripletDto,
  ParadoxReportDto,
} from "../../lib/tauri-commands";
import { SvoHighlighter } from "./SvoHighlighter";

interface PolerPanelProps {
  isOpen: boolean;
  onClose: () => void;
  chapterText: string;
  fullManuscriptText: string;
  chapterIndex: number;
}

export const PolerPanel: React.FC<PolerPanelProps> = ({
  isOpen,
  onClose,
  chapterText,
  fullManuscriptText,
  chapterIndex,
}) => {
  const [activeTab, setActiveTab] = useState<"heatmap" | "svo" | "paradoxes">("heatmap");
  const [epsilonData, setEpsilonData] = useState<EpsilonClimaxDto | null>(null);
  const [triplets, setTriplets] = useState<SvoTripletDto[]>([]);
  const [paradoxReport, setParadoxReport] = useState<ParadoxReportDto | null>(null);
  const [loading, setLoading] = useState<boolean>(false);
  const [selectedTriplet, setSelectedTriplet] = useState<SvoTripletDto | null>(null);

  useEffect(() => {
    if (!isOpen) return;

    let isMounted = true;
    setLoading(true);

    async function loadData() {
      try {
        const [eps, svoList, pdxReport] = await Promise.all([
          cmdComputeEpsilonClimax(chapterText),
          cmdExtractSvo(chapterText),
          cmdDetectParadoxes(fullManuscriptText || chapterText),
        ]);

        if (isMounted) {
          setEpsilonData(eps);
          setTriplets(svoList);
          setParadoxReport(pdxReport);
        }
      } catch (err) {
        console.error("Failed to load POLER data via Tauri IPC:", err);
      } finally {
        if (isMounted) setLoading(false);
      }
    }

    loadData();

    return () => {
      isMounted = false;
    };
  }, [isOpen, chapterText, fullManuscriptText]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/75 backdrop-blur-sm p-4">
      <div className="bg-slate-900 border border-slate-700/80 rounded-xl shadow-2xl w-full max-w-5xl h-[85vh] flex flex-col overflow-hidden">
        
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900/50">
          <div className="flex items-center space-x-3">
            <span className="text-xl font-bold bg-gradient-to-r from-purple-400 via-amber-400 to-cyan-400 bg-clip-text text-transparent">
              POLER Engine Ψ
            </span>
            <span className="text-xs px-2 py-0.5 rounded bg-purple-950/60 text-purple-300 border border-purple-800/40">
              UA-LP v7.5-LEM
            </span>
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
              {paradoxReport && paradoxReport.totalParadoxes > 0 && (
                <span className="ml-1.5 px-1.5 py-0.2 bg-rose-500 text-white rounded-full text-[10px] font-bold">
                  {paradoxReport.totalParadoxes}
                </span>
              )}
            </button>
          </div>

          <button
            onClick={onClose}
            className="text-slate-400 hover:text-white transition-colors p-1"
          >
            ✕
          </button>
        </div>

        {/* Content Body */}
        <div className="flex-1 overflow-y-auto p-6 bg-slate-950/40">
          {loading ? (
            <div className="flex items-center justify-center h-full text-slate-400 space-x-3">
              <div className="w-5 h-5 border-2 border-purple-500 border-t-transparent rounded-full animate-spin" />
              <span>Analyzing Ukrainian Symbolic Physics...</span>
            </div>
          ) : (
            <>
              {/* TAB 1: Heatmap & Metrics */}
              {activeTab === "heatmap" && epsilonData && (
                <div className="space-y-6">
                  <div className="grid grid-cols-4 gap-4">
                    <div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">
                      <div className="text-xs text-slate-400">Epsilon Climax (ε)</div>
                      <div className="text-2xl font-bold text-amber-400">{epsilonData.epsilon.toFixed(3)}</div>
                      <div className="text-[10px] text-slate-500">Threshold: ≥ 7.50</div>
                    </div>
                    <div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">
                      <div className="text-xs text-slate-400">Frobenius Norm (||A||_F)</div>
                      <div className="text-2xl font-bold text-cyan-400">{epsilonData.frobeniusNorm.toFixed(3)}</div>
                      <div className="text-[10px] text-slate-500">Omega Conf: {epsilonData.omegaConf.toFixed(2)}</div>
                    </div>
                    <div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">
                      <div className="text-xs text-slate-400">Spectral Radius ρ(A)</div>
                      <div className="text-2xl font-bold text-purple-400">{epsilonData.spectralRadius.toFixed(3)}</div>
                      <div className="text-[10px] text-slate-500">Nodes: {epsilonData.nodeCount} | Edges: {epsilonData.edgeCount}</div>
                    </div>
                    <div className="bg-slate-900/80 border border-slate-800 p-4 rounded-lg">
                      <div className="text-xs text-slate-400">Status</div>
                      <div className="mt-1">
                        {epsilonData.isClimax ? (
                          <span className="px-2.5 py-1 bg-red-950 text-red-300 border border-red-800/60 text-xs font-bold rounded">
                            CLIMAX PEAK
                          </span>
                        ) : epsilonData.noiseFiltered ? (
                          <span className="px-2.5 py-1 bg-slate-800 text-slate-400 text-xs rounded">
                            NOISE FILTERED
                          </span>
                        ) : (
                          <span className="px-2.5 py-1 bg-cyan-950 text-cyan-300 border border-cyan-800/60 text-xs rounded">
                            NORMAL TENSION
                          </span>
                        )}
                      </div>
                    </div>
                  </div>

                  {/* SVO Syntax Highlighted Reader */}
                  <div className="bg-slate-900/60 border border-slate-800 p-6 rounded-xl">
                    <h3 className="text-sm font-semibold text-slate-300 mb-4">Chapter SVO Syntax Highlighting</h3>
                    <SvoHighlighter text={chapterText} triplets={triplets} onTripletSelect={setSelectedTriplet} />
                  </div>
                </div>
              )}

              {/* TAB 2: SVO Table Inspector */}
              {activeTab === "svo" && (
                <div className="space-y-4">
                  <div className="overflow-x-auto border border-slate-800 rounded-lg">
                    <table className="w-full text-left text-xs text-slate-300">
                      <thead className="bg-slate-900 text-slate-400 border-b border-slate-800">
                        <tr>
                          <th className="p-3">Actor (Subject)</th>
                          <th className="p-3">Verb (Predicate)</th>
                          <th className="p-3">Target (Object)</th>
                          <th className="p-3">Polarity</th>
                          <th className="p-3">Confidence</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-slate-800/60">
                        {triplets.map((t, idx) => (
                          <tr key={idx} className="hover:bg-slate-800/40 transition-colors">
                            <td className="p-3 font-medium text-purple-300">{t.actor}</td>
                            <td className="p-3 font-semibold text-amber-400">{t.verb}</td>
                            <td className="p-3 text-cyan-300">{t.target || "—"}</td>
                            <td className="p-3">
                              {t.polarity ? (
                                <span className="text-emerald-400">Affirmative</span>
                              ) : (
                                <span className="text-rose-400">Negated</span>
                              )}
                            </td>
                            <td className="p-3 font-mono">{(t.confidence * 100).toFixed(0)}%</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                </div>
              )}

              {/* TAB 3: Paradox Feed */}
              {activeTab === "paradoxes" && paradoxReport && (
                <div className="space-y-4">
                  {paradoxReport.paradoxes.length === 0 ? (
                    <div className="text-center py-12 text-slate-500">
                      No temporal paradoxes detected in manuscript.
                    </div>
                  ) : (
                    paradoxReport.paradoxes.map((pdx, idx) => (
                      <div key={idx} className="bg-rose-950/20 border border-rose-900/40 p-4 rounded-lg flex justify-between items-center">
                        <div>
                          <div className="flex items-center space-x-2">
                            <span className="text-xs font-bold px-2 py-0.5 bg-rose-900/60 text-rose-200 rounded">
                              {pdx.kind === "dead_speaking" ? "Dead-Speaking" : "Spatial Teleportation"}
                            </span>
                            <span className="text-sm font-semibold text-slate-200">{pdx.character}</span>
                          </div>
                          <p className="text-xs text-slate-400 mt-1">{pdx.explanation}</p>
                        </div>
                        <div className="text-xs text-slate-500 text-right">
                          <div>Origin Ch: {pdx.originChapterIdx + 1}</div>
                          <div>Manifest Ch: {pdx.chapterIdx + 1}</div>
                        </div>
                      </div>
                    ))
                  )}
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
};
```

---

## 7. Performance & Virtualization Strategies

1. **Regex-Free Fast Tokenization**: `SvoHighlighter.tsx` utilizes word-boundary substring lookups mapped into HashSets rather than compiling complex regexes over large texts.
2. **Tab Lazy Loading**: POLER IPC queries run once upon modal launch via `Promise.all([cmdComputeEpsilonClimax(...), ...])`, preventing redundant UI renders during tab switching.

---

## 8. Complete Implementation Plan & Verification Steps

| Task ID | Target File | Action | Status |
| :--- | :--- | :--- | :--- |
| **F.2.1** | `src/lib/tauri-commands.ts` | Add POLER DTO interfaces and IPC functions | Ready |
| **F.2.2** | `src/components/litgraph/SvoHighlighter.tsx` | Create Ukrainian syntax highlighter | Ready |
| **F.2.3** | `src/components/litgraph/PolerPanel.tsx` | Create 3-tab visualizer modal dialog | Ready |
| **F.2.4** | `src/components/litgraph/LitApp.tsx` | Wire toolbar button & modal state | Ready |
| **F.2.5** | Verification | Run full build, test on `sfera.md` and `kasiopia.md` | Pending |
