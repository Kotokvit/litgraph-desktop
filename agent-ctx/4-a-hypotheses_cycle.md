# Task 4-a: hypotheses.rs + cycle.rs

**Agent**: full-stack-developer
**Task**: Build hypotheses.rs and cycle.rs — orchestration layer (Wave 4)
**Status**: ✅ Complete — all 10 unit tests pass, 0 clippy warnings in my files.

## Files Written

1. `/home/z/my-project/litgraph-desktop/src-tauri/src/reasoning/hypotheses.rs` (~910 LOC)
2. `/home/z/my-project/litgraph-desktop/src-tauri/src/reasoning/cycle.rs` (~770 LOC)

## Verification

Standalone cargo project at `/tmp/check_cycle/`:
- `cargo check --lib`: clean (0 warnings in my files)
- `cargo clippy --lib --tests`: 0 warnings, 0 errors in hypotheses.rs and cycle.rs
  (pre-existing errors in facts.rs tests: approx_constant — sibling module)
- `cargo test --lib`: **98/98 passing** (5 hypotheses + 5 cycle + 88 sibling)

### Test Results (my files)

```
running 5 tests for reasoning::hypotheses::tests::
test test_generate_for_violation_proposes_flashback_and_dream ... ok
test test_generate_for_paradox_proposes_resurrect ... ok
test test_hypothesis_log_assigns_sequential_ids ... ok
test test_verifier_accepts_flashback_resolution ... ok
test test_verifier_rejects_resurrect_without_event ... ok

running 5 tests for reasoning::cycle::tests::
test test_from_project_initializes_character_alive_facts ... ok
test test_run_cycle_accepts_flashback_hypothesis ... ok
test test_run_cycle_detects_dead_speaking_paradox ... ok
test test_run_cycle_generates_hypothesis_for_paradox ... ok
test test_run_cycle_with_kill_event_marks_target_dead ... ok
```

## Public API Summary

### hypotheses.rs exports

| Type | Derives | Notes |
|------|---------|-------|
| `HypothesisId` | (type alias u64) | |
| `EventKind` | Debug/Clone/Ser/De/PartialEq | Canonical/Flashback/Dream/Vision/StoryWithinStory |
| `Resolution` | Debug/Clone/Ser/De | MarkEventAs { event_id, kind } |
| `HypothesisSource` | Debug/Clone/Ser/De/PartialEq | Algorithm/Llm/User |
| `HypothesisStatus` | Debug/Clone/Ser/De/PartialEq | Pending/Accepted/Rejected(String) |
| `Hypothesis` | Debug/Clone/Ser/De | id/statement/proposed_resolution/evidence_for/evidence_against/status/source |
| `HypothesisGenerator` | Debug/Clone/Default | new, generate_for_violation, generate_for_paradox |
| `HypothesisVerifier` | Debug/Clone/Default | new, verify |
| `HypothesisLog` | Debug/Clone/Default | new, add, get, get_mut, pending, accepted, rejected, all |

### cycle.rs exports

| Type | Derives | Notes |
|------|---------|-------|
| `CycleReport` | Debug/Clone/Ser/De | 7 fields per SPEC §2.12 |
| `ReasoningCycle` | (manual Default) | 11 public + 2 private fields; 12 methods |

## SPEC Deviations (documented)

1. **classifications map**: SPEC §2.12 suggests updating `Event.provenance`
   for flashback/dream markers, but `Provenance` enum has no such variant.
   Solution: `HashMap<EventId, EventKind>` on `ReasoningCycle` + getters
   `event_classification()` and `classifications()`.

2. **Separate FactLog for memory**: `KnowledgeBase::from_project` takes
   ownership of `FactLog`. Can't share with `cycle.facts` without modifying
   sibling module. Solution: `cycle.memory` gets its own empty `FactLog`;
   not auto-synced with `cycle.facts`.

3. **update_state first-write-wins**: Multiple Accepted hypotheses may target
   the same event with different kinds (Flashback vs Dream). Using
   `entry().or_insert()` ensures the first classification wins, preventing
   Dream from overwriting Flashback.

## Fixes Applied During Verification

1. `test_hypothesis_log_assigns_sequential_ids`: replaced `..h1.clone()` struct
   update (h1 moved) with a `make_hyp` closure.
2. `test_run_cycle_accepts_flashback_hypothesis`: replaced `.iter().find()` on
   temporary Vec with `.into_iter().find().cloned()`.
3. `update_state`: changed `insert` to `entry().or_insert()` (first-write-wins).
4. Moved `TemporalAnchor` import to test module (was unused in non-test code).
5. Reflowed `CycleReport` doc comment (doc_lazy_continuation clippy warning).
6. Simplified `reason()` return (let_and_return clippy warning).

## Cross-Module Dependencies

hypotheses.rs imports:
- `crate::reasoning::constraints::ConstraintViolation`
- `crate::reasoning::contradictions::TemporalParadox`
- `crate::reasoning::facts::{Action, EventId, FactId, FactLog, FactValue}`
- `crate::reasoning::state::WorldState`

cycle.rs imports:
- `crate::models::Project`
- `crate::reasoning::causality::CausalityEngine`
- `crate::reasoning::constraints::{ConstraintEngine, ConstraintViolation}`
- `crate::reasoning::contradictions::{ContradictionDetector, ContradictionReport, TemporalParadox}`
- `crate::reasoning::facts::{Event, EventId, FactLog, FactValue}`
- `crate::reasoning::hypotheses::{EventKind, HypothesisGenerator, HypothesisId, HypothesisLog, HypothesisStatus, HypothesisVerifier, Resolution}`
- `crate::reasoning::inference::{InferenceEngine, InferredFact}`
- `crate::reasoning::memory::KnowledgeBase`
- `crate::reasoning::rules::RuleSet`
- `crate::reasoning::state::{StateTransition, WorldSnapshot, WorldState}`
- `std::collections::{HashMap, HashSet}`

## Ready for Wave 5 Integration

Coordinator should in `mod.rs`:
1. Uncomment `pub mod hypotheses;` and `pub mod cycle;`
2. Add to re-exports:
   ```rust
   pub use hypotheses::{EventKind, Hypothesis, HypothesisGenerator, HypothesisId,
     HypothesisLog, HypothesisSource, HypothesisStatus, HypothesisVerifier, Resolution};
   pub use cycle::{CycleReport, ReasoningCycle};
   ```
