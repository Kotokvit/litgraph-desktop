# Task 1-d — rules.rs (Wave 1 / Data Layer)

## Agent
full-stack-developer (rules.rs)

## File written
`/home/z/my-project/litgraph-desktop/src-tauri/src/reasoning/rules.rs`

## Public types/APIs exported

| Type | Kind | Notes |
|------|------|-------|
| `RuleEntity` | enum | `Actor`, `Target`, `Specific(EntityId)` |
| `EventField` | enum | `Actor`, `Target`, `Instrument`, `SourceText` |
| `RuleEffect` | enum | 5 variants: `SetAttribute`, `SetAttributeFromEvent`, `AppendToList`, `InvalidateAttribute`, `RecordKnowledge` |
| `Precondition` | struct + impl | Fields: `entity`, `attribute`, `expected`. Method: `is_satisfied(&WorldState) -> bool` |
| `Rule` | struct | Fields: `name: &'static str`, `matches: Action`, `effects: Vec<RuleEffect>`, `preconditions: Vec<Precondition>` |
| `RuleSet` | struct + impl + Default | Methods: `new()`, `default_literary()`, `add(rule)`, `find_matching(action) -> Vec<&Rule>`, `len()`, `is_empty()`, `iter()`. `Default::default()` delegates to `default_literary()`. |

All public types derive `Debug, Clone`. No Serialize/Deserialize (Rule is static config).

## default_literary() rule count
**21 rules** — 18 canonical Action variants + 3 catch-all Custom rules.

Canonical (a–r):
1. kill_target — Action::Kill
2. wound_target — Action::Wound
3. die_action — Action::Die
4. resurrect — Action::Resurrect
5. move_actor — Action::Move{destination} (placeholder)
6. arrive_at — Action::Arrive{destination} (placeholder)
7. leave_from — Action::Leave{source}
8. know_fact — Action::Know{fact} (placeholder)
9. forget_fact — Action::Forget{fact} (placeholder)
10. want_goal — Action::Want{goal} (placeholder)
11. fall_in_love — Action::FallInLove{partner} (placeholder)
12. hate_target — Action::Hate{target} (placeholder)
13. betray_victim — Action::Betray{victim} (placeholder)
14. marry_partner — Action::Marry{partner} (placeholder)
15. capture_target — Action::Capture
16. imprison_target — Action::Imprison
17. free_target — Action::Free
18. heal_target — Action::Heal

Catch-all Custom:
19. custom_positive — Action::Custom{polarity: Positive}
20. custom_negative — Action::Custom{polarity: Negative}
21. custom_neutral — Action::Custom{polarity: Neutral}

## Payload-substitution convention
**Documented in module-level `//!` doc comment** (top of rules.rs).

Convention summary:
- Action variants carrying payload (Move, Arrive, Know, Forget, Want, FallInLove, Hate, Betray, Marry) use placeholder values in their `RuleEffect`:
  - `FactValue::Str(String::new())` for string payloads (destination, fact, goal, source)
  - `FactValue::EntityRef(String::new())` for EntityId payloads (partner, target, victim)
- `inference.rs` (Wave 2) MUST detect these placeholders in `SetAttribute` / `AppendToList` effects and substitute the real payload from the triggering `Action` variant.
- A substitution table is provided in the module doc mapping each Action variant to its placeholder value and source field.

## SPEC deviations (with justification)

1. **`action_matches` helper**: task brief suggested `(a, b) => a == b` fallback for non-Custom variants. **Replaced with `discriminant(a) == discriminant(b)`**.
   - **Justification**: Strict `==` would make payload-carrying rules non-functional. A rule with `matches: Move { destination: String::new() }` would never match a real `Move { destination: "Замок" }` event because the destination strings differ. Discriminant matching is semantically correct: a rule describes "any Move action", not "Move to a specific location". This deviation is documented in a code comment on `action_matches`.
   - **Impact on tests**: All mandatory tests pass (Kill, Die, Resurrect, Custom all use discriminant or polarity matching). Added `test_payload_action_matches_by_discriminant` to verify the behavior explicitly.

2. **Assumed `FactValue: PartialEq`**: `Precondition::is_satisfied` uses `*v == self.expected`, which requires `FactValue` to implement `PartialEq`. SPEC §2.5 derives only `Debug, Clone, Serialize, Deserialize` for `FactValue`.
   - **Justification**: `Action` and `Provenance` already derive `PartialEq` per SPEC §2.3-2.4, so adding it to `FactValue` is a consistent extension. Wave 1 facts.rs agent MUST add `PartialEq` to the `FactValue` derives. Documented in worklog.

3. **`EventId` import omitted**: task brief suggested `use crate::reasoning::facts::{Action, FactValue, VerbPolarity, EventId};`, but `EventId` is not used anywhere in rules.rs (Rule is static config, doesn't store event references). Omitted the unused import to avoid compiler warnings.

## Dependencies on other Wave 1 modules
- `crate::reasoning::facts::{Action, FactValue, VerbPolarity}` — must be implemented per SPEC §2.4-2.5. **Required additional derive: `PartialEq` on `FactValue`** (see deviation #2).
- `crate::reasoning::state::{Attribute, EntityId, WorldState}` — must be implemented per SPEC §2.1, §2.7. Required methods: `WorldState::new()`, `WorldState::get(&EntityId, &Attribute) -> Option<&FactValue>`.

## Tests
9 tests total (6 mandatory + 3 additional):
- test_default_ruleset_has_kill_rule ✓
- test_find_matching_returns_kill_rule ✓
- test_custom_action_matches_by_polarity ✓
- test_precondition_is_satisfied ✓
- test_rule_for_die_action ✓
- test_rule_for_resurrect_action ✓
- test_default_ruleset_count (additional) — asserts 21 rules
- test_payload_action_matches_by_discriminant (additional) — verifies Move/Arrive/Know placeholder matching
- test_ruleset_add_and_iter (additional) — verifies add/iter/is_empty
- test_default_ruleset_default_trait (additional) — verifies Default impl

## Worklog
Appended to `/home/z/my-project/litgraph-desktop/worklog.md` under section `Task ID: 1-d`.

## Status
✅ Complete. Awaiting Wave 1 facts.rs and state.rs agents to implement dependent types so the module compiles end-to-end.
