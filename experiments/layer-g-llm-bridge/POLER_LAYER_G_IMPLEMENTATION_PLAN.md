# POLER Layer G — LLM Reasoning Bridge: Implementation Plan

**Document ID:** POLER_LAYER_G_IMPLEMENTATION_PLAN.md
**Source:** Synthesized from `99-DETAILED_WORK_PROMPT.md` Part D
**Status:** Active — implementation in progress
**Last updated:** 2026-08-10

## 1. Overview

Layer G is the LLM Reasoning Bridge — it consumes paradoxes from Layer E's
`ParadoxDetector`, generates 4 canonical resolution hypotheses via an LLM,
and validates the LLM's proposed text against the deterministic WorldState
+ ConstraintEngine.

### Data flow

```
ParadoxDetector (Layer E)
    ↓ Vec<Paradox>
LlmBridge::generate_hypotheses(paradoxes, provider)
    ↓ Vec<Hypothesis>
[User picks a hypothesis in PolerPanel UI]
    ↓ selected Hypothesis
LlmBridge::generate_resolution_text(hypothesis, provider)
    ↓ String (proposed chapter text)
Validator::validate(proposed_text, world_state, constraint_engine)
    ↓ ValidationOutcome (accept | reject | retry)
[If reject → feed back to LLM with feedback_prompt]
```

## 2. Rust: `litgraph-core/src/reasoning/llm_bridge.rs`

### 2.1 Types

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum HypothesisKind {
    Flashback,              // спогад про померлого персонажа
    DreamSequence,          // сон або галюцинація
    UnrecordedResurrection, // сюжетне воскресіння (магія/медицина)
    DisguisedIdentity,      // самозванець / подвійник
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hypothesis {
    pub id: String,                    // UUID
    pub paradox_id: String,            // references Paradox.id (B.3 fix)
    pub kind: HypothesisKind,
    pub summary: String,               // 1-2 sentence summary
    pub proposed_text: Option<String>, // full proposed chapter text (None until generated)
    pub confidence: f64,               // LLM self-reported confidence [0,1]
    pub rationale: String,             // why this hypothesis fits
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ValidationOutcome {
    Accept { violations: Vec<String>, paradoxes: Vec<String> },
    Reject { violations: Vec<String>, feedback_prompt: String },
    Retry { reason: String },
}
```

### 2.2 LlmBridge struct

```rust
pub struct LlmBridge {
    provider: AiProvider,
}

impl LlmBridge {
    pub fn new(provider: AiProvider) -> Self { ... }

    /// Generate 4 canonical hypotheses for a single paradox.
    pub async fn generate_hypotheses(&self, paradox: &Paradox) -> Result<Vec<Hypothesis>, String> {
        let prompt = build_paradox_resolution_prompt(paradox);
        let messages = vec![
            ChatMessage { role: "system".into(), content: SYSTEM_PROMPT.into() },
            ChatMessage { role: "user".into(), content: prompt },
        ];
        let response = chat(&self.provider, messages).await.map_err(|e| e.to_string())?;
        parse_hypotheses(&response, &paradox.id)
    }

    /// Generate full resolution text for a chosen hypothesis.
    pub async fn generate_resolution_text(&self, hypothesis: &Hypothesis) -> Result<Hypothesis, String> {
        let prompt = build_resolution_text_prompt(hypothesis);
        let messages = vec![
            ChatMessage { role: "system".into(), content: SYSTEM_PROMPT.into() },
            ChatMessage { role: "user".into(), content: prompt },
        ];
        let response = chat(&self.provider, messages).await.map_err(|e| e.to_string())?;
        Ok(Hypothesis { proposed_text: Some(response), ..hypothesis.clone() })
    }

    /// Validate LLM-proposed text against the deterministic Layer E
    /// ParadoxDetector (no LLM call here — pure symbolic check).
    pub fn validate(&self, proposed_text: &str, original_paradoxes: &[Paradox]) -> ValidationOutcome {
        // Re-run ParadoxDetector on the proposed text. If new paradoxes
        // appear (or original ones persist), reject with feedback.
        // If no new paradoxes and originals are resolved, accept.
    }
}
```

### 2.3 Parser

`parse_hypotheses` extracts 4 `Hypothesis` structs from a raw LLM response.
The LLM is instructed to emit strict JSON of the form:

```json
[
  { "kind": "flashback", "summary": "...", "rationale": "...", "confidence": 0.8 },
  { "kind": "dreamSequence", "summary": "...", "rationale": "...", "confidence": 0.6 },
  { "kind": "unrecordedResurrection", "summary": "...", "rationale": "...", "confidence": 0.5 },
  { "kind": "disguisedIdentity", "summary": "...", "rationale": "...", "confidence": 0.7 }
]
```

The parser:
1. Strips Markdown code fences if present (```json ... ```).
2. Tries `serde_json::from_str`.
3. If parse fails, returns `Err` with a snippet of the LLM response for debugging.
4. Validates that all 4 `HypothesisKind` variants are present (one each).
5. Generates a UUID for each `Hypothesis.id`.

## 3. Rust: `litgraph-core/src/ai/prompts.rs` (extend)

Add two new prompt builders:

### 3.1 `build_paradox_resolution_prompt(paradox: &Paradox) -> String`

Includes:
- Paradox kind, character, chapter indices, explanation
- Verbatim `evidence_text` snippets (added in Phase 0.4 / G.0.4)
- Instruction: "Generate exactly 4 hypotheses — one of each kind:
  Flashback, DreamSequence, UnrecordedResurrection, DisguisedIdentity"
- Output format spec (strict JSON array)

### 3.2 `build_resolution_text_prompt(hypothesis: &Hypothesis) -> String`

Includes:
- Hypothesis kind + summary + rationale
- Original paradox context (passed via the Hypothesis.paradox_id reference)
- Instruction: "Write a 500-1500 word chapter section that resolves this paradox"
- Constraint: "Must be consistent with the WorldState (introduce no new paradoxes)"

## 4. Rust: `src-tauri/src/commands/llm_bridge.rs` (new)

Three Tauri commands, following the `poler.rs` template:

```rust
#[tauri::command]
pub async fn cmd_generate_llm_hypotheses(
    paradox: ParadoxDto,
    provider: AiProvider,
) -> Result<Vec<HypothesisDto>, String> { ... }

#[tauri::command]
pub async fn cmd_generate_resolution_text(
    hypothesis: HypothesisDto,
    provider: AiProvider,
) -> Result<HypothesisDto, String> { ... }

#[tauri::command]
pub async fn cmd_validate_llm_response(
    proposed_text: String,
    original_paradoxes: Vec<ParadoxDto>,
) -> Result<ValidationOutcomeDto, String> { ... }
```

Register in `src-tauri/src/commands/mod.rs` and `src-tauri/src/lib.rs::generate_handler!`.

## 5. TS DTOs + wrappers (`src/lib/tauri-commands.ts`, extend)

```typescript
export type HypothesisKind = "flashback" | "dreamSequence" | "unrecordedResurrection" | "disguisedIdentity";

export interface HypothesisDto {
  id: string;
  paradoxId: string;
  kind: HypothesisKind;
  summary: string;
  proposedText: string | null;
  confidence: number;
  rationale: string;
}

export type ValidationOutcomeDto =
  | { kind: "accept"; violations: string[]; paradoxes: string[] }
  | { kind: "reject"; violations: string[]; feedbackPrompt: string }
  | { kind: "retry"; reason: string };

export async function cmdGenerateLlmHypotheses(paradox: ParadoxDto, provider: AiProviderConfig): Promise<HypothesisDto[]>;
export async function cmdGenerateResolutionText(hypothesis: HypothesisDto, provider: AiProviderConfig): Promise<HypothesisDto>;
export async function cmdValidateLlmResponse(proposedText: string, originalParadoxes: ParadoxDto[]): Promise<ValidationOutcomeDto>;
```

## 6. High-level API (`src/lib/llm-bridge/api.ts`, new)

```typescript
export async function generateHypothesesForParadox(paradox: ParadoxDto): Promise<HypothesisDto[]>;
export async function generateResolution(hypothesis: HypothesisDto): Promise<HypothesisDto>;
export async function validateResolution(text: string, originalParadoxes: ParadoxDto[]): Promise<ValidationOutcomeDto>;
```

These read `aiProviderConfig` from the Zustand store and forward to the
typed Tauri wrappers.

## 7. React UI: 4th tab in PolerPanel

Per subagent 10's recommendation (PolerPanel is the Layer F visualizer,
Layer G extends F naturally):

Add a 4th tab "🧪 LLM Hypotheses" to PolerPanel.tsx:

- Shows list of paradoxes (reuse from Paradox Feed tab)
- Each paradox has a "🧪 Hypothesize" button
- Clicking generates 4 hypotheses via `cmdGenerateLlmHypotheses`
- Each hypothesis card shows: kind, summary, rationale, confidence
- "Generate Full Text" button → calls `cmdGenerateResolutionText`
- "Validate" button → calls `cmdValidateLlmResponse`, shows accept/reject/retry badge
- If reject, shows feedbackPrompt and offers "Regenerate with feedback" button

## 8. Test plan

### 8.1 Rust unit tests (`litgraph-core/src/reasoning/llm_bridge.rs`)

- `test_parse_hypotheses_returns_4_kinds` — mock chat() to return canned JSON, verify 4 hypotheses with distinct kinds.
- `test_parse_hypotheses_handles_malformed_json` — verify graceful error.
- `test_parse_hypotheses_strips_markdown_fences` — input wrapped in ```json ... ```, verify still parses.
- `test_validate_accepts_consistent_text` — proposed text with no paradoxes → Accept.
- `test_validate_rejects_dead_speaking` — proposed text where dead character speaks → Reject with feedback.
- `test_validate_rejects_spatial_teleportation` — proposed text with teleportation → Reject.

### 8.2 src-tauri integration tests (`src-tauri/src/commands/llm_bridge.rs`)

- `test_cmd_generate_llm_hypotheses_smoke` — smoke test with mock provider
- `test_cmd_validate_llm_response_smoke` — smoke test

### 8.3 TS tests (`src/components/litgraph/PolerPanel.test.tsx`)

- Tab switching test
- Hypothesis card rendering test
- Validation badge rendering test (mock cmdGenerateLlmHypotheses etc.)

## 9. Risk mitigations (from Part H)

- **H.2 (LLM non-determinism):** All Layer G tests use a `MockProvider` — never call real LLMs in CI.
- **H.3 (prompt injection):** CSP fix (Phase 0.3 / G.0.3) ships before Layer G. LLM output rendered via React's default JSX escaping (never `dangerouslySetInnerHTML`).
- **H.6 (PolerPanel complexity):** Tab content extracted into `LlmHypothesesTab.tsx` (separate component) to keep PolerPanel.tsx under 800 LOC.

## 10. Source of truth

When this spec disagrees with code, **code is the source of truth**.
The Rust DTOs in `litgraph-core/src/reasoning/llm_bridge.rs` and
`src-tauri/src/commands/llm_bridge.rs` define the wire format. The TS
DTOs in `tauri-commands.ts` mirror them byte-for-byte (camelCase via
`#[serde(rename_all = "camelCase")]`).
