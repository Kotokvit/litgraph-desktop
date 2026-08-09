//! rules.rs — Rule, RuleSet, Precondition, RuleEffect, RuleEntity, EventField.
//!
//! Декларативное отображение `Action` → эффекты на `WorldState`. Каждое правило
//! описывает, какое состояние должно измениться, когда произошло некоторое
//! действие. Правила собираются в `RuleSet`, который запрашивается
//! inference-движком (`inference.rs`, Wave 2) при поступлении нового события.
//!
//! ## Принципы
//!
//! 1. **State is truth.** Правило не вычисляет истину — оно описывает, как
//!    изменится состояние при наступлении триггерного `Action`.
//! 2. **Static configuration.** `Rule` — статическая декларация: `name`,
//!    `matches`, `effects`, `preconditions`. Вся динамика (разрешение
//!    `Actor`/`Target` в конкретные `EntityId`, применение эффектов, проверка
//!    preconditions с event-context) живёт в `inference.rs`.
//! 3. **No LLM.** Правила не вызывают LLM — это чисто алгоритмический слой.
//! 4. **Russian-first UI strings** (в сообщениях/комментариях для пользователя),
//!    английские идентификаторы.
//!
//! ## Конвенция о payload-подстановке (ВАЖНО для `inference.rs` Wave 2)
//!
//! Некоторые `Action`-варианты переносят payload, который нельзя статически
//! встроить в `RuleEffect`:
//!   - `Move { destination }`, `Arrive { destination }`, `Leave { source }`
//!   - `Know { fact }`, `Forget { fact }`, `Want { goal }`, `Plan { goal }`
//!   - `FallInLove { partner }`, `Hate { target }`, `Betray { victim }`,
//!     `Marry { partner }`, `Ally { partner }`
//!   - `Speak { topic }`, `Ask { topic }`, `Tell { topic, to }`
//!   - `Discover { fact }`, `Transform { new_form }`
//!
//! В `default_literary()` для таких правил используются **placeholder-значения**:
//!   - `FactValue::Str(String::new())` — для строковых payload (destination,
//!     fact, goal, source, ...)
//!   - `FactValue::EntityRef(String::new())` — для `EntityId`-payload (partner,
//!     target, victim, to, ...)
//!
//! Inference engine (Wave 2, `inference.rs`) обязан детектировать эти
//! placeholder'ы в `RuleEffect::SetAttribute` / `RuleEffect::AppendToList` и
//! подставлять реальный payload из `Action`-варианта триггерного события.
//! Подстановка идёт по таблице (см. `default_literary()` для конкретных правил):
//!
//! | Action variant          | placeholder value           | substituted from    |
//! |-------------------------|-----------------------------|---------------------|
//! | `Move{destination}`     | `Str(String::new())`        | `action.destination`|
//! | `Arrive{destination}`   | `Str(String::new())`        | `action.destination`|
//! | `Leave{source}`         | (InvalidateAttribute, N/A)  | N/A                 |
//! | `Know{fact}`            | `Str(String::new())`        | `action.fact`       |
//! | `Forget{fact}`          | `Str(String::new())`        | `action.fact`       |
//! | `Want{goal}`            | `Str(String::new())`        | `action.goal`       |
//! | `FallInLove{partner}`   | `EntityRef(String::new())`  | `action.partner`    |
//! | `Hate{target}`          | `EntityRef(String::new())`  | `action.target`     |
//! | `Betray{victim}`        | `EntityRef(String::new())`  | `action.victim`     |
//! | `Marry{partner}`        | `EntityRef(String::new())`  | `action.partner`    |
//!
//! Inference engine должен искать `RuleEffect::SetAttribute { value: Str(String::new()), .. }`
//! или `RuleEffect::AppendToList { value: EntityRef(String::new()), .. }` и
//! заменять `String::new()` на соответствующее поле из `Action`.
//!
//! ## Соглашение о matching'е
//!
//! `RuleSet::find_matching(action)` возвращает все правила, чей `matches`-вариант
//! соответствует данному action:
//!   - Для `Action::Custom` сравнивается только `polarity` (verb_lemma — wildcard).
//!   - Для payload-несущих вариантов (`Move`, `Arrive`, `Know`, ...) сравнивается
//!     только дискриминант варианта (payload — wildcard), что позволяет правилу с
//!     placeholder-payload соответствовать любому событию того же варианта.
//!   - Для остальных вариантов — сравнение по дискриминанту (variant-only).
//!
//! См. приватную функцию `action_matches` ниже.

use crate::reasoning::facts::{Action, FactValue, VerbPolarity};
use crate::reasoning::state::{Attribute, EntityId, WorldState};

// NOTE: `EventId` из facts.rs здесь не используется напрямую (Rule — статическая
// конфигурация, не хранит event-id'ы). Если в будущем потребуется ссылка на
// event в правиле — добавить `use crate::reasoning::facts::EventId;`.

// ============================================================================
// RuleEntity
// ============================================================================

/// На кого направлено правило: на actor'а события, на target'а события или на
/// конкретную сущность по ID. `Actor` и `Target` разрешаются inference-движком
/// в момент применения правила на основе триггерного `Event`.
#[derive(Debug, Clone)]
pub enum RuleEntity {
    /// Тот, кто совершил действие (event.actor)
    Actor,
    /// Тот, над кем совершено действие (event.target, если есть)
    Target,
    /// Конкретная сущность по ID (например, "Пётр")
    Specific(EntityId),
}

// ============================================================================
// EventField
// ============================================================================

/// Поле события, из которого можно извлечь значение для `SetAttributeFromEvent`.
#[derive(Debug, Clone)]
pub enum EventField {
    /// event.actor (EntityId)
    Actor,
    /// event.target (EntityId)
    Target,
    /// event.instrument (String)
    Instrument,
    /// event.source_text (String)
    SourceText,
}

// ============================================================================
// RuleEffect
// ============================================================================

/// Эффект, применяемый правилом к `WorldState`. Inference-движок интерпретирует
/// каждый вариант:
///
/// - `SetAttribute` — установить значение атрибута (перезаписать).
/// - `SetAttributeFromEvent` — установить атрибут из поля события
///   (например, location = event.instrument для Move... но на практике Move
///   использует placeholder + подстановку, см. конвенцию выше).
/// - `AppendToList` — добавить значение в список-атрибут (например, knowledge).
/// - `InvalidateAttribute` — пометить атрибут как `FactValue::Unknown`
///   (например, location после смерти).
/// - `RecordKnowledge` — записать в knowledge-base, что `knower` узнал о
///   событии (или о факте, если `about_event = false`).
#[derive(Debug, Clone)]
pub enum RuleEffect {
    /// Установить атрибут сущности в конкретное значение.
    SetAttribute {
        entity: RuleEntity,
        attribute: Attribute,
        value: FactValue,
    },
    /// Установить атрибут сущности, взяв значение из поля события.
    SetAttributeFromEvent {
        entity: RuleEntity,
        attribute: Attribute,
        source: EventField,
    },
    /// Добавить значение в список-атрибут (FactValue::List) сущности.
    AppendToList {
        entity: RuleEntity,
        attribute: Attribute,
        value: FactValue,
    },
    /// Пометить атрибут как Unknown (например, location после смерти).
    InvalidateAttribute {
        entity: RuleEntity,
        attribute: Attribute,
    },
    /// Записать в knowledge-base, что `knower` узнал о событии.
    /// Если `about_event = true` — knower узнал о триггерном событии.
    /// Если `about_event = false` — knower узнал о факте (заглушка для будущих
    /// расширений; в default_literary всегда `true`).
    RecordKnowledge {
        knower: RuleEntity,
        about_event: bool,
    },
}

// ============================================================================
// Precondition
// ============================================================================

/// Предусловие правила: у сущности `entity` атрибут `attribute` должен быть
/// равен `expected`. Если предусловие не выполнено — правило не применяется.
///
/// **Важно:** `is_satisfied` без event-context может проверить только
/// `RuleEntity::Specific(id)`. Для `Actor`/`Target` возвращается `false`
/// (inference-движок Wave 2 разрешит их и перепроверит).
#[derive(Debug, Clone)]
pub struct Precondition {
    pub entity: RuleEntity,
    pub attribute: Attribute,
    pub expected: FactValue,
}

impl Precondition {
    /// Проверить, выполнено ли предусловие в данном `WorldState`.
    ///
    /// - Для `RuleEntity::Specific(id)` — реальная проверка через
    ///   `state.get(id, attribute)`.
    /// - Для `RuleEntity::Actor` / `RuleEntity::Target` — возвращает `false`,
    ///   т.к. без event-context невозможно разрешить, кто есть actor/target.
    ///   Inference-движок (Wave 2) разрешит сущности и перепроверит.
    pub fn is_satisfied(&self, state: &WorldState) -> bool {
        match &self.entity {
            RuleEntity::Specific(id) => match state.get(id, &self.attribute) {
                Some(v) => *v == self.expected,
                None => false,
            },
            // Без event-context не можем разрешить Actor/Target → считаем
            // неудовлетворённым. Inference.rs (Wave 2) перепроверит после
            // разрешения сущностей.
            RuleEntity::Actor | RuleEntity::Target => false,
        }
    }
}

// ============================================================================
// Rule
// ============================================================================

/// Правило: «если произошло событие с action → применить effects к WorldState,
/// при условии что все preconditions выполнены».
///
/// `matches` — Action-вариант, который триггерит правило. Для `Action::Custom`
/// matching идёт по polarity (verb_lemma — wildcard). Для payload-несущих
/// вариантов matching идёт по дискриминанту (payload — wildcard).
#[derive(Debug, Clone)]
pub struct Rule {
    /// Человекочитаемое имя правила (для логов и отладки).
    pub name: &'static str,
    /// Какой Action-вариант триггерит правило.
    pub matches: Action,
    /// Эффекты, применяемые к WorldState.
    pub effects: Vec<RuleEffect>,
    /// Предусловия (все должны быть выполнены).
    pub preconditions: Vec<Precondition>,
}

// ============================================================================
// RuleSet
// ============================================================================

/// Набор правил. Запрашивается inference-движком через `find_matching(action)`,
/// который возвращает все правила, чей `matches` соответствует данному action.
pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    /// Пустой набор правил.
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Базовый набор правил для литературных текстов.
    ///
    /// Включает 21 правило:
    ///   - 18 правил для канонических Action-вариантов (Kill, Wound, Die, ...)
    ///   - 3 catch-all правила для `Action::Custom` (positive/negative/neutral)
    ///
    /// См. конвенцию о payload-подстановке в модуле doc выше.
    pub fn default_literary() -> Self {
        let mut rs = Self::new();

        // === a) kill_target — Action::Kill ===
        rs.add(Rule {
            name: "kill_target",
            matches: Action::Kill,
            effects: vec![
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Target,
                    attribute: "alive".to_string(),
                    value: FactValue::Bool(false),
                },
                RuleEffect::InvalidateAttribute {
                    entity: RuleEntity::Target,
                    attribute: "location".to_string(),
                },
                RuleEffect::RecordKnowledge {
                    knower: RuleEntity::Actor,
                    about_event: true,
                },
            ],
            preconditions: vec![],
        });

        // === b) wound_target — Action::Wound ===
        rs.add(Rule {
            name: "wound_target",
            matches: Action::Wound,
            effects: vec![
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Target,
                    attribute: "physical_state".to_string(),
                    value: FactValue::Str("wounded".to_string()),
                },
                RuleEffect::RecordKnowledge {
                    knower: RuleEntity::Actor,
                    about_event: true,
                },
            ],
            preconditions: vec![],
        });

        // === c) die_action — Action::Die ===
        rs.add(Rule {
            name: "die_action",
            matches: Action::Die,
            effects: vec![
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Actor,
                    attribute: "alive".to_string(),
                    value: FactValue::Bool(false),
                },
                RuleEffect::InvalidateAttribute {
                    entity: RuleEntity::Actor,
                    attribute: "location".to_string(),
                },
            ],
            preconditions: vec![],
        });

        // === d) resurrect — Action::Resurrect ===
        rs.add(Rule {
            name: "resurrect",
            matches: Action::Resurrect,
            effects: vec![RuleEffect::SetAttribute {
                entity: RuleEntity::Actor,
                attribute: "alive".to_string(),
                value: FactValue::Bool(true),
            }],
            preconditions: vec![],
        });

        // === e) move_actor — Action::Move { destination } ===
        // placeholder: inference.rs (Wave 2) substitutes destination from Action::Move
        rs.add(Rule {
            name: "move_actor",
            matches: Action::Move {
                destination: String::new(),
            },
            effects: vec![RuleEffect::SetAttribute {
                entity: RuleEntity::Actor,
                attribute: "location".to_string(),
                value: FactValue::Str(String::new()),
            }],
            preconditions: vec![],
        });

        // === f) arrive_at — Action::Arrive { destination } ===
        // placeholder: inference.rs substitutes destination from Action::Arrive
        rs.add(Rule {
            name: "arrive_at",
            matches: Action::Arrive {
                destination: String::new(),
            },
            effects: vec![RuleEffect::SetAttribute {
                entity: RuleEntity::Actor,
                attribute: "location".to_string(),
                value: FactValue::Str(String::new()),
            }],
            preconditions: vec![],
        });

        // === g) leave_from — Action::Leave { source } ===
        rs.add(Rule {
            name: "leave_from",
            matches: Action::Leave {
                source: String::new(),
            },
            effects: vec![RuleEffect::InvalidateAttribute {
                entity: RuleEntity::Actor,
                attribute: "location".to_string(),
            }],
            preconditions: vec![],
        });

        // === h) know_fact — Action::Know { fact } ===
        // placeholder: inference.rs substitutes fact from Action::Know
        rs.add(Rule {
            name: "know_fact",
            matches: Action::Know {
                fact: String::new(),
            },
            effects: vec![RuleEffect::AppendToList {
                entity: RuleEntity::Actor,
                attribute: "knowledge".to_string(),
                value: FactValue::Str(String::new()),
            }],
            preconditions: vec![],
        });

        // === i) forget_fact — Action::Forget { fact } ===
        // placeholder: inference.rs substitutes fact from Action::Forget
        // NOTE: семантически forget должен удалять из списка, но AppendToList —
        // единственный list-вариант в RuleEffect. Inference.rs Wave 2 должен
        // интерпретировать это как "remove from list" для forget_fact.
        rs.add(Rule {
            name: "forget_fact",
            matches: Action::Forget {
                fact: String::new(),
            },
            effects: vec![RuleEffect::AppendToList {
                entity: RuleEntity::Actor,
                attribute: "knowledge".to_string(),
                value: FactValue::Str(String::new()),
            }],
            preconditions: vec![],
        });

        // === j) want_goal — Action::Want { goal } ===
        // placeholder: inference.rs substitutes goal from Action::Want
        rs.add(Rule {
            name: "want_goal",
            matches: Action::Want {
                goal: String::new(),
            },
            effects: vec![RuleEffect::AppendToList {
                entity: RuleEntity::Actor,
                attribute: "goals".to_string(),
                value: FactValue::Str(String::new()),
            }],
            preconditions: vec![],
        });

        // === k) fall_in_love — Action::FallInLove { partner } ===
        // placeholder: inference.rs substitutes partner from Action::FallInLove
        rs.add(Rule {
            name: "fall_in_love",
            matches: Action::FallInLove {
                partner: String::new(),
            },
            effects: vec![RuleEffect::AppendToList {
                entity: RuleEntity::Actor,
                attribute: "relationships".to_string(),
                value: FactValue::EntityRef(String::new()),
            }],
            preconditions: vec![],
        });

        // === l) hate_target — Action::Hate { target } ===
        // placeholder: inference.rs substitutes target from Action::Hate
        rs.add(Rule {
            name: "hate_target",
            matches: Action::Hate {
                target: String::new(),
            },
            effects: vec![RuleEffect::AppendToList {
                entity: RuleEntity::Actor,
                attribute: "relationships".to_string(),
                value: FactValue::EntityRef(String::new()),
            }],
            preconditions: vec![],
        });

        // === m) betray_victim — Action::Betray { victim } ===
        // placeholder: inference.rs substitutes victim from Action::Betray
        rs.add(Rule {
            name: "betray_victim",
            matches: Action::Betray {
                victim: String::new(),
            },
            effects: vec![
                RuleEffect::AppendToList {
                    entity: RuleEntity::Actor,
                    attribute: "betrayals".to_string(),
                    value: FactValue::EntityRef(String::new()),
                },
                RuleEffect::RecordKnowledge {
                    knower: RuleEntity::Target,
                    about_event: true,
                },
            ],
            preconditions: vec![],
        });

        // === n) marry_partner — Action::Marry { partner } ===
        // placeholder: inference.rs substitutes partner from Action::Marry
        rs.add(Rule {
            name: "marry_partner",
            matches: Action::Marry {
                partner: String::new(),
            },
            effects: vec![
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Actor,
                    attribute: "spouse".to_string(),
                    value: FactValue::EntityRef(String::new()),
                },
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Target,
                    attribute: "spouse".to_string(),
                    value: FactValue::EntityRef(String::new()),
                },
            ],
            preconditions: vec![],
        });

        // === o) capture_target — Action::Capture ===
        rs.add(Rule {
            name: "capture_target",
            matches: Action::Capture,
            effects: vec![RuleEffect::SetAttribute {
                entity: RuleEntity::Target,
                attribute: "captured".to_string(),
                value: FactValue::Bool(true),
            }],
            preconditions: vec![],
        });

        // === p) imprison_target — Action::Imprison ===
        rs.add(Rule {
            name: "imprison_target",
            matches: Action::Imprison,
            effects: vec![
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Target,
                    attribute: "imprisoned".to_string(),
                    value: FactValue::Bool(true),
                },
                RuleEffect::InvalidateAttribute {
                    entity: RuleEntity::Target,
                    attribute: "location".to_string(),
                },
            ],
            preconditions: vec![],
        });

        // === q) free_target — Action::Free ===
        rs.add(Rule {
            name: "free_target",
            matches: Action::Free,
            effects: vec![
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Target,
                    attribute: "captured".to_string(),
                    value: FactValue::Bool(false),
                },
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Target,
                    attribute: "imprisoned".to_string(),
                    value: FactValue::Bool(false),
                },
            ],
            preconditions: vec![],
        });

        // === r) heal_target — Action::Heal ===
        rs.add(Rule {
            name: "heal_target",
            matches: Action::Heal,
            effects: vec![RuleEffect::SetAttribute {
                entity: RuleEntity::Target,
                attribute: "physical_state".to_string(),
                value: FactValue::Str("healthy".to_string()),
            }],
            preconditions: vec![],
        });

        // === Catch-all правила для Action::Custom ===
        // verb_lemma: String::new() — wildcard, matches any Custom with same polarity.
        // Target узнаёт о действии, совершённом над ним (независимо от полярности).

        // custom_positive — Action::Custom { polarity: Positive }
        rs.add(Rule {
            name: "custom_positive",
            matches: Action::Custom {
                verb_lemma: String::new(),
                polarity: VerbPolarity::Positive,
            },
            effects: vec![RuleEffect::RecordKnowledge {
                knower: RuleEntity::Target,
                about_event: true,
            }],
            preconditions: vec![],
        });

        // custom_negative — Action::Custom { polarity: Negative }
        rs.add(Rule {
            name: "custom_negative",
            matches: Action::Custom {
                verb_lemma: String::new(),
                polarity: VerbPolarity::Negative,
            },
            effects: vec![RuleEffect::RecordKnowledge {
                knower: RuleEntity::Target,
                about_event: true,
            }],
            preconditions: vec![],
        });

        // custom_neutral — Action::Custom { polarity: Neutral }
        rs.add(Rule {
            name: "custom_neutral",
            matches: Action::Custom {
                verb_lemma: String::new(),
                polarity: VerbPolarity::Neutral,
            },
            effects: vec![RuleEffect::RecordKnowledge {
                knower: RuleEntity::Target,
                about_event: true,
            }],
            preconditions: vec![],
        });

        rs
    }

    /// Добавить правило в набор.
    pub fn add(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Найти все правила, чей `matches` соответствует данному `action`.
    ///
    /// Возвращает `Vec<&Rule>` (ссылки на правила в `self`). Inference-движок
    /// применяет все найденные правила (если их preconditions выполнены).
    pub fn find_matching(&self, action: &Action) -> Vec<&Rule> {
        self.rules
            .iter()
            .filter(|r| action_matches(&r.matches, action))
            .collect()
    }

    /// Количество правил в наборе.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// `true`, если набор пуст.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Итератор по правилам.
    pub fn iter(&self) -> impl Iterator<Item = &Rule> {
        self.rules.iter()
    }
}

impl Default for RuleSet {
    /// По умолчанию — литературный набор правил.
    fn default() -> Self {
        Self::default_literary()
    }
}

// ============================================================================
// action_matches (private helper)
// ============================================================================

/// Сравнивает `Action` из правила с `Action` из события.
///
/// Правила matching'а:
///   - `Action::Custom { polarity, .. }` ↔ `Action::Custom { polarity, .. }`:
///     сравнивается только `polarity` (verb_lemma — wildcard).
///   - Для всех остальных вариантов — сравнение по дискриминанту
///     (variant-only, payload — wildcard). Это позволяет правилу с
///     placeholder-payload (например, `Move { destination: String::new() }`)
///     соответствовать любому событию того же варианта
///     (например, `Move { destination: "Замок" }`).
///
/// **Отступление от буквальной спецификации:** SPEC §2.8 / task brief
/// предлагают fallback `(a, b) => a == b` (точное сравнение через PartialEq).
/// Но это сделало бы правила для payload-вариантов (Move, Arrive, Know, ...)
/// неработоспособными — правило с `Move { destination: String::new() }`
/// никогда бы не совпало с реальным `Move { destination: "Замок" }`. Поэтому
/// используется сравнение по дискриминанту, что semantically корректно:
/// правило описывает «любое действие Move», а не «Move в конкретную точку».
fn action_matches(rule_action: &Action, given: &Action) -> bool {
    use std::mem::discriminant;
    match (rule_action, given) {
        // Custom: match by polarity only (verb_lemma is wildcard)
        (
            Action::Custom {
                polarity: rp,
                verb_lemma: _,
            },
            Action::Custom {
                polarity: gp,
                verb_lemma: _,
            },
        ) => rp == gp,
        // Все остальные: match by discriminant (variant-only, payload is wildcard)
        (a, b) => discriminant(a) == discriminant(b),
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::state::WorldState;

    #[test]
    fn test_default_ruleset_has_kill_rule() {
        let rs = RuleSet::default_literary();
        // В дефолтном наборе должно быть правило kill_target, матчащее Action::Kill.
        let matched = rs.find_matching(&Action::Kill);
        assert!(
            matched.iter().any(|r| r.name == "kill_target"),
            "kill_target rule must be present and match Action::Kill"
        );
    }

    #[test]
    fn test_find_matching_returns_kill_rule() {
        let rs = RuleSet::default_literary();
        let matched = rs.find_matching(&Action::Kill);
        assert!(!matched.is_empty(), "at least one rule must match Action::Kill");

        let kill_rule = matched
            .iter()
            .find(|r| r.name == "kill_target")
            .expect("kill_target rule must exist");

        // Должно быть 3 эффекта: SetAttribute alive=false, InvalidateAttribute location, RecordKnowledge Actor
        assert_eq!(kill_rule.effects.len(), 3);

        // SetAttribute { Target, "alive", Bool(false) }
        assert!(
            kill_rule.effects.iter().any(|e| matches!(
                e,
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Target,
                    attribute,
                    value: FactValue::Bool(false),
                } if attribute == "alive"
            )),
            "kill_target must set Target.alive = false"
        );

        // InvalidateAttribute { Target, "location" }
        assert!(
            kill_rule.effects.iter().any(|e| matches!(
                e,
                RuleEffect::InvalidateAttribute {
                    entity: RuleEntity::Target,
                    attribute,
                } if attribute == "location"
            )),
            "kill_target must invalidate Target.location"
        );

        // RecordKnowledge { Actor, about_event: true }
        assert!(
            kill_rule.effects.iter().any(|e| matches!(
                e,
                RuleEffect::RecordKnowledge {
                    knower: RuleEntity::Actor,
                    about_event: true,
                }
            )),
            "kill_target must record knowledge for Actor about the event"
        );

        // preconditions пустые
        assert!(kill_rule.preconditions.is_empty());
    }

    #[test]
    fn test_custom_action_matches_by_polarity() {
        let rs = RuleSet::default_literary();

        // Positive custom action — должно матчить только custom_positive
        let pos_matched = rs.find_matching(&Action::Custom {
            verb_lemma: "спасти".to_string(),
            polarity: VerbPolarity::Positive,
        });
        assert!(
            pos_matched.iter().any(|r| r.name == "custom_positive"),
            "Positive Custom must match custom_positive rule"
        );
        assert!(
            !pos_matched.iter().any(|r| r.name == "custom_negative"),
            "Positive Custom must NOT match custom_negative rule"
        );
        assert!(
            !pos_matched.iter().any(|r| r.name == "custom_neutral"),
            "Positive Custom must NOT match custom_neutral rule"
        );

        // Negative custom action — должно матчить только custom_negative
        let neg_matched = rs.find_matching(&Action::Custom {
            verb_lemma: "оскорбить".to_string(),
            polarity: VerbPolarity::Negative,
        });
        assert!(
            neg_matched.iter().any(|r| r.name == "custom_negative"),
            "Negative Custom must match custom_negative rule"
        );
        assert!(
            !neg_matched.iter().any(|r| r.name == "custom_positive"),
            "Negative Custom must NOT match custom_positive rule"
        );

        // Neutral custom action — должно матчить только custom_neutral
        let neu_matched = rs.find_matching(&Action::Custom {
            verb_lemma: "пойти".to_string(),
            polarity: VerbPolarity::Neutral,
        });
        assert!(
            neu_matched.iter().any(|r| r.name == "custom_neutral"),
            "Neutral Custom must match custom_neutral rule"
        );
        assert!(
            !neu_matched.iter().any(|r| r.name == "custom_positive"),
            "Neutral Custom must NOT match custom_positive rule"
        );

        // Custom action не должен матчить канонические правила (Kill, Wound, ...)
        assert!(
            !pos_matched.iter().any(|r| r.name == "kill_target"),
            "Custom action must not match kill_target"
        );
    }

    #[test]
    fn test_precondition_is_satisfied() {
        // Precondition с Specific entity можно проверить против WorldState.
        let precondition = Precondition {
            entity: RuleEntity::Specific("Пётр".to_string()),
            attribute: "alive".to_string(),
            expected: FactValue::Bool(true),
        };

        // Пустое состояние — значение отсутствует, предусловие НЕ выполнено.
        let state = WorldState::new();
        assert!(
            !precondition.is_satisfied(&state),
            "Precondition on empty state must be unsatisfied (no value)"
        );

        // Precondition с Actor/Target не может быть разрешён без event-context.
        // is_satisfied возвращает false.
        let unresolved_actor = Precondition {
            entity: RuleEntity::Actor,
            attribute: "alive".to_string(),
            expected: FactValue::Bool(true),
        };
        assert!(
            !unresolved_actor.is_satisfied(&state),
            "Precondition with Actor entity must be false without event context"
        );

        let unresolved_target = Precondition {
            entity: RuleEntity::Target,
            attribute: "alive".to_string(),
            expected: FactValue::Bool(true),
        };
        assert!(
            !unresolved_target.is_satisfied(&state),
            "Precondition with Target entity must be false without event context"
        );
    }

    #[test]
    fn test_rule_for_die_action() {
        let rs = RuleSet::default_literary();
        let matched = rs.find_matching(&Action::Die);
        assert!(!matched.is_empty(), "at least one rule must match Action::Die");

        let die_rule = matched
            .iter()
            .find(|r| r.name == "die_action")
            .expect("die_action rule must exist");

        // SetAttribute { Actor, "alive", Bool(false) }
        assert!(
            die_rule.effects.iter().any(|e| matches!(
                e,
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Actor,
                    attribute,
                    value: FactValue::Bool(false),
                } if attribute == "alive"
            )),
            "die_action must set Actor.alive = false"
        );

        // InvalidateAttribute { Actor, "location" }
        assert!(
            die_rule.effects.iter().any(|e| matches!(
                e,
                RuleEffect::InvalidateAttribute {
                    entity: RuleEntity::Actor,
                    attribute,
                } if attribute == "location"
            )),
            "die_action must invalidate Actor.location"
        );
    }

    #[test]
    fn test_rule_for_resurrect_action() {
        let rs = RuleSet::default_literary();
        let matched = rs.find_matching(&Action::Resurrect);
        assert!(
            !matched.is_empty(),
            "at least one rule must match Action::Resurrect"
        );

        let resurrect_rule = matched
            .iter()
            .find(|r| r.name == "resurrect")
            .expect("resurrect rule must exist");

        // SetAttribute { Actor, "alive", Bool(true) }
        assert!(
            resurrect_rule.effects.iter().any(|e| matches!(
                e,
                RuleEffect::SetAttribute {
                    entity: RuleEntity::Actor,
                    attribute,
                    value: FactValue::Bool(true),
                } if attribute == "alive"
            )),
            "resurrect must set Actor.alive = true"
        );

        // Должен быть ровно 1 эффект
        assert_eq!(
            resurrect_rule.effects.len(),
            1,
            "resurrect must have exactly 1 effect"
        );
    }

    // === Дополнительные smoke-тесты ===

    #[test]
    fn test_default_ruleset_count() {
        let rs = RuleSet::default_literary();
        // 18 канонических правил (a-r) + 3 catch-all Custom = 21 правило.
        assert_eq!(rs.len(), 21, "default_literary must contain 21 rules");
        assert!(!rs.is_empty());
    }

    #[test]
    fn test_payload_action_matches_by_discriminant() {
        let rs = RuleSet::default_literary();

        // Move с конкретным destination должен матчить move_actor (placeholder).
        let matched = rs.find_matching(&Action::Move {
            destination: "Замок".to_string(),
        });
        assert!(
            matched.iter().any(|r| r.name == "move_actor"),
            "Move with any destination must match move_actor rule (discriminant matching)"
        );

        // Arrive с конкретным destination должен матчить arrive_at.
        let matched = rs.find_matching(&Action::Arrive {
            destination: "Лес".to_string(),
        });
        assert!(
            matched.iter().any(|r| r.name == "arrive_at"),
            "Arrive with any destination must match arrive_at rule"
        );

        // Know с конкретным fact должен матчить know_fact.
        let matched = rs.find_matching(&Action::Know {
            fact: "Пётр предал".to_string(),
        });
        assert!(
            matched.iter().any(|r| r.name == "know_fact"),
            "Know with any fact must match know_fact rule"
        );
    }

    #[test]
    fn test_ruleset_add_and_iter() {
        let mut rs = RuleSet::new();
        assert!(rs.is_empty());
        assert_eq!(rs.len(), 0);

        rs.add(Rule {
            name: "test_rule",
            matches: Action::Touch,
            effects: vec![],
            preconditions: vec![],
        });
        assert_eq!(rs.len(), 1);
        assert!(!rs.is_empty());

        let names: Vec<&str> = rs.iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["test_rule"]);
    }

    #[test]
    fn test_default_ruleset_default_trait() {
        // Default trait делегирует в default_literary.
        let rs = RuleSet::default();
        assert_eq!(rs.len(), 21);
        assert!(rs.find_matching(&Action::Kill).iter().any(|r| r.name == "kill_target"));
    }
}
