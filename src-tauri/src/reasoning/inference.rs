//! inference.rs — Forward-chaining inference engine.
//!
//! Применяет правила из [`RuleSet`] к поступающим событиям ([`Event`]) и
//! фиксирует выведенные факты в [`FactLog`], параллельно мутируя
//! [`WorldState`]. Каждый вывод имеет audit trail: какое событие каким
//! правилом породило какой факт — это возвращает [`InferredFact`].
//!
//! # Принцип
//!
//! **State is truth.** Inference engine не «думает» — он детерминированно
//! применяет декларативные правила. Любая «правда» о мире выражается как
//! переход состояния ([`StateTransition`]) и фиксируется как факт ([`Fact`]).
//! LLM сюда не допускается.
//!
//! # Payload substitution (критично)
//!
//! Wave 1 `rules.rs` объявляет правила с placeholder-значениями для тех
//! `Action`-вариантов, чей payload нельзя статически встроить в `RuleEffect`.
//! Inference engine обязан детектировать эти placeholder'ы в
//! `RuleEffect::SetAttribute` / `RuleEffect::AppendToList` и подставлять
//! реальный payload из `Action`-варианта триггерного события.
//!
//! Таблица подстановок:
//!
//! | Action variant          | placeholder pattern                          | substituted from                            |
//! |-------------------------|----------------------------------------------|---------------------------------------------|
//! | `Move{destination}`     | `Str(String::new())` для attribute "location"| `FactValue::Str(action.destination)`       |
//! | `Arrive{destination}`   | `Str(String::new())` для attribute "location"| `FactValue::Str(action.destination)`       |
//! | `Leave{source}`         | n/a (использует `InvalidateAttribute`)        | n/a                                         |
//! | `Know{fact}`            | `Str(String::new())` в `AppendToList` для "knowledge" | `FactValue::Str(action.fact)`        |
//! | `Forget{fact}`          | `Str(String::new())` в `AppendToList` для "knowledge" | special: REMOVE from list (см. ниже) |
//! | `Want{goal}`            | `Str(String::new())` в `AppendToList` для "goals"     | `FactValue::Str(action.goal)`        |
//! | `Plan{goal}`            | `Str(String::new())` в `AppendToList` для "plans"     | `FactValue::Str(action.goal)`        |
//! | `FallInLove{partner}`   | `EntityRef(String::new())` в `AppendToList` для "relationships" | `FactValue::EntityRef(action.partner)` |
//! | `Hate{target}`          | `EntityRef(String::new())` в `AppendToList` для "relationships" | `FactValue::EntityRef(action.target)` |
//! | `Betray{victim}`        | `EntityRef(String::new())` в `AppendToList` для "betrayals"      | `FactValue::EntityRef(action.victim)` |
//! | `Marry{partner}`        | `EntityRef(String::new())` для "spouse" (Actor И Target) | Actor → `EntityRef(action.partner)`; Target → `EntityRef(event.actor)` |
//! | `Tell{topic,to}`        | n/a — обрабатывается через `RecordKnowledge`  | n/a                                         |
//! | `Custom{verb_lemma,..}` | n/a — catch-all правила только с `RecordKnowledge` | n/a                                    |
//!
//! ## Специальная обработка Forget
//!
//! `Forget { fact }` НЕ добавляет факт в список knowledge, а УДАЛЯЕТ
//! совпадающий элемент. Это обрабатывается в [`InferenceEngine::apply_event`]
//! ДО обычной обработки `AppendToList`: после `substitute_payload` (которая
//! для Forget оставляет placeholder `Str(String::new())` без изменений)
//! проверяется сочетание `Action::Forget` + `AppendToList { attribute: "knowledge", value: Str("") }`
//! и вместо добавления вызывается [`apply_forget_removal`].
//!
//! # Связь с другими модулями
//!
//! - [`Event`] / [`Fact`] / [`FactLog`] / [`FactValue`] / [`Action`] — из `facts.rs`.
//! - [`WorldState`] / [`StateTransition`] / [`Attribute`] — из `state.rs`.
//! - [`Rule`] / [`RuleSet`] / [`RuleEffect`] / [`RuleEntity`] / [`Precondition`] — из `rules.rs`.
//! - [`TemporalAnchor`] — из `timeline.rs` (используется для `valid_from` и `at` в переходах).
//!
//! См. `docs/reasoning/SPEC.md` §2.8 (Rule/RuleEffect) для формального контракта.

use serde::{Deserialize, Serialize};

use crate::reasoning::facts::{
    Action, Event, EventId, Fact, FactId, FactLog, FactValue, Provenance,
};
use crate::reasoning::rules::{Precondition, RuleEffect, RuleEntity, RuleSet};
// `Rule` импортируется по требованию SPEC (task brief), но в non-test коде не
// именуется явно (используется через `for rule in matched_rules`). В тестах
// он нужен для конструирования кастомных правил.
#[allow(unused_imports)]
use crate::reasoning::rules::Rule;
use crate::reasoning::state::{Attribute, StateTransition, WorldState};
// `TemporalAnchor` аналогично — в non-test коде используется через
// `event.time` (доступ к полям/методам через `.`), в тестах — для конструирования.
#[allow(unused_imports)]
use crate::reasoning::timeline::TemporalAnchor;

// ============================================================================
// InferredFact
// ============================================================================

/// Результат применения правила: какой факт был зафиксирован, каким событием
/// он был порождён и через какое правило. Возвращается из
/// [`InferenceEngine::apply_event`] для audit trail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InferredFact {
    /// ID зафиксированного факта в `FactLog`.
    pub fact_id: FactId,
    /// ID события, которое триггерило применение правила.
    pub from_event: EventId,
    /// Имя правила, породившего факт (для диагностики и UI).
    pub rule_name: &'static str,
}

// ============================================================================
// InferenceEngine
// ============================================================================

/// Forward-chaining inference engine: применяет правила из `RuleSet` к
/// поступающим событиям, мутирует `WorldState` и фиксирует выведенные факты
/// в `FactLog`. Полностью синхронный и детерминированный.
pub struct InferenceEngine {
    rule_set: RuleSet,
}

impl InferenceEngine {
    /// Создать engine с заданным набором правил.
    pub fn new(rule_set: RuleSet) -> Self {
        Self { rule_set }
    }

    /// Создать engine с `RuleSet::default_literary()` (21 правило для
    /// литературных текстов).
    pub fn default_literary() -> Self {
        Self::new(RuleSet::default_literary())
    }

    /// Доступ (только для чтения) к набору правил.
    pub fn rule_set(&self) -> &RuleSet {
        &self.rule_set
    }

    /// Главная точка входа: применить одно событие к миру.
    ///
    /// Алгоритм:
    /// 1. Найти все правила, чей `matches` соответствует `event.action`.
    /// 2. Для каждого правила:
    ///    - проверить все `preconditions` (с разрешением Actor/Target через
    ///      контекст события). Если хотя бы одно не выполнено — пропустить
    ///      правило (с логом в `stderr`);
    ///    - для каждого `effect`: подставить payload из события
    ///      ([`substitute_payload`]); если action = `Forget` и эффект =
    ///      `AppendToList` для "knowledge" с placeholder'ом — вызвать
    ///      [`apply_forget_removal`]; иначе — применить эффект через
    ///      [`apply_effect`].
    /// 3. Вернуть список [`InferredFact`] для audit trail.
    ///
    /// **Не записывает само событие в `FactLog`** — это ответственность
    /// вызывающего (или [`Self::apply_events`], который делает это автоматически).
    pub fn apply_event(
        &self,
        event: &Event,
        world: &mut WorldState,
        facts: &mut FactLog,
    ) -> Vec<InferredFact> {
        let mut inferred: Vec<InferredFact> = Vec::new();
        let matched_rules = self.rule_set.find_matching(&event.action);

        for rule in matched_rules {
            // ── Шаг 1: проверка предусловий ──────────────────────────────
            let mut preconditions_ok = true;
            for precondition in &rule.preconditions {
                if !is_precondition_satisfied(precondition, event, world) {
                    eprintln!(
                        "[inference] Правило «{}» пропущено: предусловие не выполнено \
                         (entity={:?}, attribute={}, expected={:?}) для события id={}",
                        rule.name,
                        precondition.entity,
                        precondition.attribute,
                        precondition.expected,
                        event.id
                    );
                    preconditions_ok = false;
                    break;
                }
            }
            if !preconditions_ok {
                continue;
            }

            // ── Шаг 2: применение эффектов ───────────────────────────────
            for effect in &rule.effects {
                let resolved_effect = substitute_payload(effect, event);

                // Специальная обработка Forget: вместо добавления в список
                // knowledge — удаляем совпадающий элемент.
                if let Action::Forget { fact } = &event.action {
                    if let RuleEffect::AppendToList {
                        entity,
                        attribute,
                        value: FactValue::Str(s),
                    } = &resolved_effect
                    {
                        if attribute == "knowledge" && s.is_empty() {
                            if let Some(entity_id) = resolve_entity(entity, event) {
                                apply_forget_removal(
                                    &entity_id,
                                    fact,
                                    event,
                                    world,
                                    facts,
                                    rule.name,
                                    &mut inferred,
                                );
                            } else {
                                eprintln!(
                                    "[inference] Forget: RuleEntity::Target не разрешим \
                                     (event id={} без target) — эффект пропущен",
                                    event.id
                                );
                            }
                            continue;
                        }
                    }
                }

                apply_effect(
                    &resolved_effect,
                    event,
                    world,
                    facts,
                    rule.name,
                    &mut inferred,
                );
            }
        }

        inferred
    }

    /// Пакетная обработка: записывает каждое событие в `FactLog` (что
    /// присваивает канонический `EventId`, если он был 0), затем применяет
    /// его через [`Self::apply_event`]. Удобно для тестов и для цикла
    /// рассуждения (cycle.rs).
    pub fn apply_events<I: IntoIterator<Item = Event>>(
        &self,
        events: I,
        world: &mut WorldState,
        facts: &mut FactLog,
    ) -> Vec<InferredFact> {
        let mut all_inferred: Vec<InferredFact> = Vec::new();
        for mut event in events {
            let event_id = facts.record_event(event.clone());
            event.id = event_id;
            all_inferred.extend(self.apply_event(&event, world, facts));
        }
        all_inferred
    }
}

impl Default for InferenceEngine {
    /// По умолчанию — литературный набор правил.
    fn default() -> Self {
        Self::default_literary()
    }
}

// ============================================================================
// Private helpers
// ============================================================================

/// Разрешить [`RuleEntity`] в конкретный `EntityId` на основе события.
///
/// - `Actor` → `event.actor.clone()`
/// - `Target` → `event.target.clone()` (или `None`, если у события нет target)
/// - `Specific(id)` → `id.clone()`
fn resolve_entity(rule_entity: &RuleEntity, event: &Event) -> Option<String> {
    match rule_entity {
        RuleEntity::Actor => Some(event.actor.clone()),
        RuleEntity::Target => event.target.clone(),
        RuleEntity::Specific(id) => Some(id.clone()),
    }
}

/// Проверка предусловия с учётом контекста события.
///
/// В отличие от `Precondition::is_satisfied(&WorldState)` (которая возвращает
/// `false` для `Actor`/`Target`, т.к. не может их разрешить без event-context),
/// эта функция разрешает `Actor`/`Target` через событие и затем сравнивает
/// значение в `world` с ожидаемым через `PartialEq` на `FactValue`.
fn is_precondition_satisfied(
    precondition: &Precondition,
    event: &Event,
    world: &WorldState,
) -> bool {
    let entity_id = match &precondition.entity {
        RuleEntity::Specific(id) => id.clone(),
        RuleEntity::Actor => event.actor.clone(),
        RuleEntity::Target => match &event.target {
            Some(t) => t.clone(),
            None => {
                eprintln!(
                    "[inference] Предусловие на Target не разрешимо: у события id={} \
                     отсутствует target",
                    event.id
                );
                return false;
            }
        },
    };
    match world.get(&entity_id, &precondition.attribute) {
        Some(v) => *v == precondition.expected,
        None => false,
    }
}

/// Подставить реальный payload из события в эффект.
///
/// Возвращает НОВЫЙ [`RuleEffect`] с подставленными значениями. Если подстановка
/// не применима (нет placeholder'а или action variant не входит в таблицу
/// подстановок) — возвращает клон исходного эффекта без изменений.
///
/// Специальный случай: для `Action::Forget` placeholder НЕ подставляется
/// (остаётся `Str(String::new())`), чтобы `apply_event` мог обнаружить его и
/// выполнить удаление из списка вместо добавления.
fn substitute_payload(effect: &RuleEffect, event: &Event) -> RuleEffect {
    match effect {
        RuleEffect::SetAttribute {
            entity,
            attribute,
            value,
        } => {
            let new_value = substitute_value(value, entity, attribute, event);
            RuleEffect::SetAttribute {
                entity: entity.clone(),
                attribute: attribute.clone(),
                value: new_value,
            }
        }
        RuleEffect::AppendToList {
            entity,
            attribute,
            value,
        } => {
            let new_value = substitute_value(value, entity, attribute, event);
            RuleEffect::AppendToList {
                entity: entity.clone(),
                attribute: attribute.clone(),
                value: new_value,
            }
        }
        // Остальные варианты не содержат placeholder'ов — возвращаем клон как есть.
        _ => effect.clone(),
    }
}

/// Подстановка значения для `SetAttribute` / `AppendToList`. Делегирует в
/// [`substitute_str_value`] для строковых placeholder'ов и в
/// [`substitute_entity_ref_value`] для `EntityRef` placeholder'ов.
fn substitute_value(
    value: &FactValue,
    entity: &RuleEntity,
    attribute: &str,
    event: &Event,
) -> FactValue {
    match value {
        FactValue::Str(s) if s.is_empty() => {
            substitute_str_value(attribute, event, value)
        }
        FactValue::EntityRef(s) if s.is_empty() => {
            substitute_entity_ref_value(entity, attribute, event, value)
        }
        _ => value.clone(),
    }
}

/// Подстановка для строкового placeholder (`Str(String::new())`).
///
/// Матчит по `(action variant, attribute)` и возвращает подставленное значение.
/// Если комбинация не входит в таблицу подстановок — возвращает исходный
/// placeholder без изменений (это позволяет `Forget` остаться `Str("")` и
/// попасть в специальную обработку в `apply_event`).
fn substitute_str_value(attribute: &str, event: &Event, original: &FactValue) -> FactValue {
    match (&event.action, attribute) {
        (Action::Move { destination }, "location") => FactValue::Str(destination.clone()),
        (Action::Arrive { destination }, "location") => FactValue::Str(destination.clone()),
        (Action::Know { fact }, "knowledge") => FactValue::Str(fact.clone()),
        (Action::Want { goal }, "goals") => FactValue::Str(goal.clone()),
        (Action::Plan { goal }, "plans") => FactValue::Str(goal.clone()),
        // Forget намеренно НЕ подставляется здесь — он обрабатывается отдельно.
        _ => original.clone(),
    }
}

/// Подстановка для `EntityRef` placeholder (`EntityRef(String::new())`).
///
/// Матчит по `(action variant, entity, attribute)`. Особый случай — `Marry`:
/// для `Actor` подставляется `action.partner`, для `Target` — `event.actor`
/// (симметрия брака: если Анна выходит замуж за Бориса, то Борис — супруг Анны,
/// а Анна — супруга Бориса).
fn substitute_entity_ref_value(
    entity: &RuleEntity,
    attribute: &str,
    event: &Event,
    original: &FactValue,
) -> FactValue {
    match (&event.action, entity, attribute) {
        (Action::FallInLove { partner }, _, "relationships") => {
            FactValue::EntityRef(partner.clone())
        }
        (Action::Hate { target }, _, "relationships") => FactValue::EntityRef(target.clone()),
        (Action::Betray { victim }, _, "betrayals") => FactValue::EntityRef(victim.clone()),
        (Action::Marry { partner }, RuleEntity::Actor, "spouse") => {
            FactValue::EntityRef(partner.clone())
        }
        (Action::Marry { .. }, RuleEntity::Target, "spouse") => {
            // Симметрия: супруг Target'а — это actor события.
            FactValue::EntityRef(event.actor.clone())
        }
        _ => original.clone(),
    }
}

// ============================================================================
// Effect application
// ============================================================================

/// Применить эффект к `world` и `facts`. Записывает [`InferredFact`] в
/// `inferred` для audit trail.
#[allow(clippy::too_many_arguments)]
fn apply_effect(
    effect: &RuleEffect,
    event: &Event,
    world: &mut WorldState,
    facts: &mut FactLog,
    rule_name: &'static str,
    inferred: &mut Vec<InferredFact>,
) {
    match effect {
        RuleEffect::SetAttribute {
            entity,
            attribute,
            value,
        } => {
            if let Some(entity_id) = resolve_entity(entity, event) {
                apply_set_attribute(
                    &entity_id,
                    attribute,
                    value.clone(),
                    event,
                    world,
                    facts,
                    rule_name,
                    inferred,
                );
            } else {
                eprintln!(
                    "[inference] SetAttribute: RuleEntity::Target не разрешим \
                     (event id={} без target) — эффект пропущен (rule={})",
                    event.id, rule_name
                );
            }
        }
        RuleEffect::InvalidateAttribute { entity, attribute } => {
            if let Some(entity_id) = resolve_entity(entity, event) {
                apply_invalidate_attribute(
                    &entity_id,
                    attribute,
                    event,
                    world,
                    facts,
                    rule_name,
                    inferred,
                );
            } else {
                eprintln!(
                    "[inference] InvalidateAttribute: RuleEntity::Target не разрешим \
                     (event id={} без target) — эффект пропущен (rule={})",
                    event.id, rule_name
                );
            }
        }
        RuleEffect::AppendToList {
            entity,
            attribute,
            value,
        } => {
            if let Some(entity_id) = resolve_entity(entity, event) {
                apply_append_to_list(
                    &entity_id,
                    attribute,
                    value.clone(),
                    event,
                    world,
                    facts,
                    rule_name,
                    inferred,
                );
            } else {
                eprintln!(
                    "[inference] AppendToList: RuleEntity::Target не разрешим \
                     (event id={} без target) — эффект пропущен (rule={})",
                    event.id, rule_name
                );
            }
        }
        RuleEffect::RecordKnowledge {
            knower,
            about_event: true,
        } => {
            if let Some(knower_id) = resolve_entity(knower, event) {
                // Формат знания: «actor did action to target at chapter».
                let knowledge_string = format!(
                    "{} did {:?} to {:?} at {}",
                    event.actor,
                    event.action,
                    event.target,
                    event.time.display_chapter()
                );
                apply_append_to_list(
                    &knower_id,
                    "knowledge",
                    FactValue::Str(knowledge_string),
                    event,
                    world,
                    facts,
                    rule_name,
                    inferred,
                );
            } else {
                eprintln!(
                    "[inference] RecordKnowledge: knower (Target) не разрешим \
                     (event id={} без target) — эффект пропущен (rule={})",
                    event.id, rule_name
                );
            }
        }
        RuleEffect::RecordKnowledge {
            about_event: false, ..
        } => {
            eprintln!(
                "[inference] RecordKnowledge с about_event=false не поддерживается \
                 (rule={}, event id={}) — эффект пропущен",
                rule_name, event.id
            );
        }
        RuleEffect::SetAttributeFromEvent { .. } => {
            // Не используется в default_literary. Реализация требует импорта
            // EventField, который вне скоупа текущей задачи. Логируем и пропускаем.
            eprintln!(
                "[inference] SetAttributeFromEvent не реализован (rule={}, event id={}) \
                 — эффект пропущен",
                rule_name, event.id
            );
        }
    }
}

/// Применить `SetAttribute`: записать переход + факт + обновить `world`.
#[allow(clippy::too_many_arguments)]
fn apply_set_attribute(
    entity_id: &str,
    attribute: &str,
    value: FactValue,
    event: &Event,
    world: &mut WorldState,
    facts: &mut FactLog,
    rule_name: &'static str,
    inferred: &mut Vec<InferredFact>,
) {
    let old_value = world.get(entity_id, attribute).cloned();
    let attr_string: Attribute = attribute.to_string();
    let transition = StateTransition {
        entity: entity_id.to_string(),
        attribute: attr_string.clone(),
        old_value,
        new_value: value.clone(),
        caused_by_event: Some(event.id),
        at: event.time.clone(),
    };
    world.set(entity_id, attr_string.clone(), value.clone(), transition);

    let fact = Fact {
        id: 0,
        entity: entity_id.to_string(),
        attribute: attr_string,
        value,
        derived_from: vec![event.id],
        valid_from: event.time.clone(),
        valid_until: None,
        provenance: Provenance::Verified,
    };
    let fact_id = facts.assert_fact(fact);
    inferred.push(InferredFact {
        fact_id,
        from_event: event.id,
        rule_name,
    });
}

/// Применить `InvalidateAttribute`: установить `FactValue::Unknown`,
/// записать переход + факт.
#[allow(clippy::too_many_arguments)]
fn apply_invalidate_attribute(
    entity_id: &str,
    attribute: &str,
    event: &Event,
    world: &mut WorldState,
    facts: &mut FactLog,
    rule_name: &'static str,
    inferred: &mut Vec<InferredFact>,
) {
    let old_value = world.get(entity_id, attribute).cloned();
    let attr_string: Attribute = attribute.to_string();
    let transition = StateTransition {
        entity: entity_id.to_string(),
        attribute: attr_string.clone(),
        old_value,
        new_value: FactValue::Unknown,
        caused_by_event: Some(event.id),
        at: event.time.clone(),
    };
    world.set(entity_id, attr_string.clone(), FactValue::Unknown, transition);

    let fact = Fact {
        id: 0,
        entity: entity_id.to_string(),
        attribute: attr_string,
        value: FactValue::Unknown,
        derived_from: vec![event.id],
        valid_from: event.time.clone(),
        valid_until: None,
        provenance: Provenance::Verified,
    };
    let fact_id = facts.assert_fact(fact);
    inferred.push(InferredFact {
        fact_id,
        from_event: event.id,
        rule_name,
    });
}

/// Применить `AppendToList`: добавить значение в список-атрибут.
///
/// Семантика:
/// - Если текущее значение — `FactValue::List(v)`, push'им в него.
/// - Если `FactValue::Unknown` или отсутствует — создаём новый список `[value]`.
/// - Если другой тип — логируем ошибку и пропускаем эффект.
#[allow(clippy::too_many_arguments)]
fn apply_append_to_list(
    entity_id: &str,
    attribute: &str,
    value: FactValue,
    event: &Event,
    world: &mut WorldState,
    facts: &mut FactLog,
    rule_name: &'static str,
    inferred: &mut Vec<InferredFact>,
) {
    let new_value = match world.get(entity_id, attribute) {
        Some(FactValue::List(v)) => {
            let mut v = v.clone();
            v.push(value);
            FactValue::List(v)
        }
        Some(FactValue::Unknown) | None => FactValue::List(vec![value]),
        Some(other) => {
            eprintln!(
                "[inference] AppendToList: невозможно добавить в не-list атрибут «{}» \
                 на сущности «{}» (текущее значение: {:?}) — эффект пропущен (rule={})",
                attribute, entity_id, other, rule_name
            );
            return;
        }
    };

    let old_value = world.get(entity_id, attribute).cloned();
    let attr_string: Attribute = attribute.to_string();
    let transition = StateTransition {
        entity: entity_id.to_string(),
        attribute: attr_string.clone(),
        old_value,
        new_value: new_value.clone(),
        caused_by_event: Some(event.id),
        at: event.time.clone(),
    };
    world.set(entity_id, attr_string.clone(), new_value.clone(), transition);

    let fact = Fact {
        id: 0,
        entity: entity_id.to_string(),
        attribute: attr_string,
        value: new_value,
        derived_from: vec![event.id],
        valid_from: event.time.clone(),
        valid_until: None,
        provenance: Provenance::Verified,
    };
    let fact_id = facts.assert_fact(fact);
    inferred.push(InferredFact {
        fact_id,
        from_event: event.id,
        rule_name,
    });
}

/// Специальная обработка Forget: удалить `FactValue::Str(fact)` из списка
/// knowledge (если он там есть).
///
/// Если список не существует / не список / не содержит элемента — никакие
/// переходы и факты не записываются (no-op).
#[allow(clippy::too_many_arguments)]
fn apply_forget_removal(
    entity_id: &str,
    fact: &str,
    event: &Event,
    world: &mut WorldState,
    facts: &mut FactLog,
    rule_name: &'static str,
    inferred: &mut Vec<InferredFact>,
) {
    let target_value = FactValue::Str(fact.to_string());
    let new_value = match world.get(entity_id, "knowledge") {
        Some(FactValue::List(v)) => {
            let filtered: Vec<FactValue> =
                v.iter().filter(|x| **x != target_value).cloned().collect();
            if filtered.len() == v.len() {
                eprintln!(
                    "[inference] Forget: факт «{}» не найден в knowledge списка «{}» \
                     — изменений нет",
                    fact, entity_id
                );
                return;
            }
            FactValue::List(filtered)
        }
        Some(other) => {
            eprintln!(
                "[inference] Forget: knowledge на «{}» не список ({:?}) — пропуск",
                entity_id, other
            );
            return;
        }
        None => {
            eprintln!(
                "[inference] Forget: knowledge на «{}» отсутствует — нечего удалять",
                entity_id
            );
            return;
        }
    };

    let old_value = world.get(entity_id, "knowledge").cloned();
    let transition = StateTransition {
        entity: entity_id.to_string(),
        attribute: "knowledge".to_string(),
        old_value,
        new_value: new_value.clone(),
        caused_by_event: Some(event.id),
        at: event.time.clone(),
    };
    world.set(
        entity_id,
        "knowledge".to_string(),
        new_value.clone(),
        transition,
    );

    let fact_record = Fact {
        id: 0,
        entity: entity_id.to_string(),
        attribute: "knowledge".to_string(),
        value: new_value,
        derived_from: vec![event.id],
        valid_from: event.time.clone(),
        valid_until: None,
        provenance: Provenance::Verified,
    };
    let fact_id = facts.assert_fact(fact_record);
    inferred.push(InferredFact {
        fact_id,
        from_event: event.id,
        rule_name,
    });
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::facts::Provenance;
    use crate::reasoning::rules::{Rule, RuleSet};
    use crate::reasoning::state::WorldState;
    use crate::reasoning::timeline::TemporalAnchor;

    /// Хелпер: `TemporalAnchor` для главы (без суффикса/сцены/offset).
    fn anchor(chapter: u32) -> TemporalAnchor {
        TemporalAnchor {
            chapter_num: chapter,
            chapter_suffix: None,
            scene_index: None,
            char_offset: 0,
        }
    }

    /// Хелпер: построить `Event` для тестов.
    fn make_event(actor: &str, action: Action, target: Option<&str>, time: TemporalAnchor) -> Event {
        Event {
            id: 0,
            actor: actor.to_string(),
            action,
            target: target.map(|s| s.to_string()),
            instrument: None,
            time,
            source_text: String::new(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        }
    }

    /// Хелпер: структурное сравнение `Option<&FactValue>` с ожидаемым значением.
    fn assert_fact_eq(actual: Option<&FactValue>, expected: &FactValue) {
        match actual {
            Some(v) => assert!(
                *v == *expected,
                "ожидалось {:?}, получено {:?}",
                expected,
                v
            ),
            None => panic!("ожидалось {:?}, получено None", expected),
        }
    }

    // ──────────────────────────────────────────────────────────────────
    // Обязательные тесты (8 штук)
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn test_kill_action_marks_target_dead() {
        let engine = InferenceEngine::default_literary();
        let mut world = WorldState::new();
        let mut facts = FactLog::new();

        let event = make_event(
            "Ivan",
            Action::Kill,
            Some("Petr"),
            anchor(2),
        );
        let inferred = engine.apply_event(&event, &mut world, &mut facts);

        // kill_target имеет 3 эффекта: SetAttribute alive=false, InvalidateAttribute location,
        // RecordKnowledge Actor → 3 InferredFact.
        assert_eq!(
            inferred.len(),
            3,
            "kill_target должен породить 3 факта (alive, location, knowledge)"
        );
        // Все 3 должны ссылаться на то же событие и то же правило.
        assert!(inferred.iter().all(|f| f.from_event == event.id));
        assert!(inferred.iter().all(|f| f.rule_name == "kill_target"));

        // Target помечен мёртвым.
        assert_fact_eq(world.get("Petr", "alive"), &FactValue::Bool(false));

        // Location у мертвеца — Unknown.
        assert_fact_eq(world.get("Petr", "location"), &FactValue::Unknown);

        // Ivan (actor) получил knowledge о событии (см. test_record_knowledge_creates_audit_trail).
        assert!(world.get("Ivan", "knowledge").is_some());

        // FactLog содержит 3 факта с provenance = Verified.
        let all_facts = facts.all_facts();
        assert_eq!(all_facts.len(), 3);
        assert!(all_facts.iter().all(|f| f.provenance == Provenance::Verified));
    }

    #[test]
    fn test_die_action_marks_actor_dead() {
        let engine = InferenceEngine::default_literary();
        let mut world = WorldState::new();
        let mut facts = FactLog::new();

        let event = make_event("Petr", Action::Die, None, anchor(5));
        let inferred = engine.apply_event(&event, &mut world, &mut facts);

        // die_action: SetAttribute alive=false, InvalidateAttribute location → 2 InferredFact.
        assert_eq!(inferred.len(), 2);
        assert!(inferred.iter().all(|f| f.rule_name == "die_action"));

        // Actor помечен мёртвым.
        assert_fact_eq(world.get("Petr", "alive"), &FactValue::Bool(false));

        // Location у мертвеца — Unknown.
        assert_fact_eq(world.get("Petr", "location"), &FactValue::Unknown);
    }

    #[test]
    fn test_move_action_updates_location() {
        let engine = InferenceEngine::default_literary();
        let mut world = WorldState::new();
        let mut facts = FactLog::new();

        let event = make_event(
            "Ivan",
            Action::Move { destination: "Москва".to_string() },
            None,
            anchor(3),
        );
        let inferred = engine.apply_event(&event, &mut world, &mut facts);

        // move_actor: SetAttribute location = destination (подставлено из action) → 1 InferredFact.
        assert_eq!(inferred.len(), 1);
        assert_eq!(inferred[0].rule_name, "move_actor");

        // Location обновлён на «Москва» (payload substitution сработала).
        assert_fact_eq(
            world.get("Ivan", "location"),
            &FactValue::Str("Москва".to_string()),
        );
    }

    #[test]
    fn test_know_action_appends_to_knowledge_list() {
        let engine = InferenceEngine::default_literary();
        let mut world = WorldState::new();
        let mut facts = FactLog::new();

        let event = make_event(
            "Ivan",
            Action::Know { fact: "Пётр предал".to_string() },
            None,
            anchor(1),
        );
        let inferred = engine.apply_event(&event, &mut world, &mut facts);

        // know_fact: AppendToList knowledge = fact (подставлено из action) → 1 InferredFact.
        assert_eq!(inferred.len(), 1);
        assert_eq!(inferred[0].rule_name, "know_fact");

        // Knowledge — список с одним элементом.
        let knowledge = world.get("Ivan", "knowledge");
        assert!(knowledge.is_some(), "Ivan должен иметь список knowledge");
        match knowledge.unwrap() {
            FactValue::List(v) => {
                assert_eq!(v.len(), 1, "должен быть 1 элемент в knowledge");
                match &v[0] {
                    FactValue::Str(s) => assert_eq!(s, "Пётр предал"),
                    other => panic!("ожидалась FactValue::Str, получено {:?}", other),
                }
            }
            other => panic!("ожидалась FactValue::List, получено {:?}", other),
        }

        // Повторное Know добавляет ещё один элемент (не перезаписывает).
        let event2 = make_event(
            "Ivan",
            Action::Know { fact: "Анна уехала".to_string() },
            None,
            anchor(2),
        );
        engine.apply_event(&event2, &mut world, &mut facts);

        match world.get("Ivan", "knowledge").unwrap() {
            FactValue::List(v) => assert_eq!(v.len(), 2, "должно быть 2 элемента после второго Know"),
            other => panic!("ожидалась FactValue::List, получено {:?}", other),
        }
    }

    #[test]
    fn test_forget_action_removes_from_knowledge_list() {
        let engine = InferenceEngine::default_literary();
        let mut world = WorldState::new();
        let mut facts = FactLog::new();

        // Pre-setup: Ivan знает два факта.
        let initial_knowledge = FactValue::List(vec![
            FactValue::Str("secret1".to_string()),
            FactValue::Str("secret2".to_string()),
        ]);
        world.set(
            "Ivan",
            "knowledge".to_string(),
            initial_knowledge.clone(),
            StateTransition {
                entity: "Ivan".to_string(),
                attribute: "knowledge".to_string(),
                old_value: None,
                new_value: initial_knowledge,
                caused_by_event: None,
                at: anchor(1),
            },
        );

        let event = make_event(
            "Ivan",
            Action::Forget { fact: "secret1".to_string() },
            None,
            anchor(2),
        );
        let inferred = engine.apply_event(&event, &mut world, &mut facts);

        // Forget удаляет один элемент → 1 InferredFact (с переходом).
        assert_eq!(inferred.len(), 1);
        assert_eq!(inferred[0].rule_name, "forget_fact");

        // Knowledge теперь содержит только «secret2».
        match world.get("Ivan", "knowledge").unwrap() {
            FactValue::List(v) => {
                assert_eq!(v.len(), 1, "должен остаться 1 элемент");
                match &v[0] {
                    FactValue::Str(s) => assert_eq!(s, "secret2"),
                    other => panic!("ожидалась FactValue::Str, получено {:?}", other),
                }
            }
            other => panic!("ожидалась FactValue::List, получено {:?}", other),
        }

        // Forget несуществующего факта — не порождает новый InferredFact.
        let event2 = make_event(
            "Ivan",
            Action::Forget { fact: "nonexistent".to_string() },
            None,
            anchor(3),
        );
        let inferred2 = engine.apply_event(&event2, &mut world, &mut facts);
        assert!(
            inferred2.is_empty(),
            "Forget несуществующего факта не должен породить InferredFact"
        );

        // Knowledge остался неизменным.
        match world.get("Ivan", "knowledge").unwrap() {
            FactValue::List(v) => assert_eq!(v.len(), 1),
            other => panic!("ожидалась FactValue::List, получено {:?}", other),
        }
    }

    #[test]
    fn test_precondition_blocks_rule() {
        // Кастомный RuleSet с одним правилом: «Kill применим только если
        // Actor.alive == true». Actor мёртв → правило должно быть пропущено.
        let mut rs = RuleSet::new();
        rs.add(Rule {
            name: "kill_only_if_actor_alive",
            matches: Action::Kill,
            effects: vec![RuleEffect::SetAttribute {
                entity: RuleEntity::Target,
                attribute: "alive".to_string(),
                value: FactValue::Bool(false),
            }],
            preconditions: vec![Precondition {
                entity: RuleEntity::Actor,
                attribute: "alive".to_string(),
                expected: FactValue::Bool(true),
            }],
        });
        let engine = InferenceEngine::new(rs);

        let mut world = WorldState::new();
        let mut facts = FactLog::new();

        // Setup: Ivan мёртв.
        world.set(
            "Ivan",
            "alive".to_string(),
            FactValue::Bool(false),
            StateTransition {
                entity: "Ivan".to_string(),
                attribute: "alive".to_string(),
                old_value: None,
                new_value: FactValue::Bool(false),
                caused_by_event: None,
                at: anchor(1),
            },
        );

        let event = make_event("Ivan", Action::Kill, Some("Petr"), anchor(2));
        let inferred = engine.apply_event(&event, &mut world, &mut facts);

        // Предусловие не выполнено → правило пропущено → 0 InferredFact.
        assert!(
            inferred.is_empty(),
            "Правило должно быть пропущено из-за невыполненного предусловия"
        );
        // Target.alive НЕ установлен.
        assert!(
            world.get("Petr", "alive").is_none(),
            "Target не должен быть помечен мёртвым (правило пропущено)"
        );

        // Контрольный тест: когда Actor жив — правило срабатывает.
        world.set(
            "Ivan",
            "alive".to_string(),
            FactValue::Bool(true),
            StateTransition {
                entity: "Ivan".to_string(),
                attribute: "alive".to_string(),
                old_value: Some(FactValue::Bool(false)),
                new_value: FactValue::Bool(true),
                caused_by_event: None,
                at: anchor(1),
            },
        );
        let event2 = make_event("Ivan", Action::Kill, Some("Petr"), anchor(3));
        let inferred2 = engine.apply_event(&event2, &mut world, &mut facts);
        assert_eq!(
            inferred2.len(),
            1,
            "Правило должно сработать, когда Actor жив"
        );
        assert_fact_eq(world.get("Petr", "alive"), &FactValue::Bool(false));
    }

    #[test]
    fn test_record_knowledge_creates_audit_trail() {
        let engine = InferenceEngine::default_literary();
        let mut world = WorldState::new();
        let mut facts = FactLog::new();

        let event = make_event("Ivan", Action::Kill, Some("Petr"), anchor(2));
        let inferred = engine.apply_event(&event, &mut world, &mut facts);

        // kill_target: 3 эффекта, последний — RecordKnowledge для Actor.
        assert_eq!(inferred.len(), 3);
        // Последний InferredFact — это knowledge-факт на Actor.
        let knowledge_fact_id = inferred[2].fact_id;
        let knowledge_fact = facts
            .all_facts()
            .iter()
            .find(|f| f.id == knowledge_fact_id)
            .expect("knowledge fact should be in FactLog");
        assert_eq!(knowledge_fact.entity, "Ivan");
        assert_eq!(knowledge_fact.attribute, "knowledge");
        assert_eq!(knowledge_fact.provenance, Provenance::Verified);
        assert_eq!(knowledge_fact.derived_from, vec![event.id]);

        // Проверяем формат knowledge-строки.
        match &knowledge_fact.value {
            FactValue::List(v) => {
                assert_eq!(v.len(), 1);
                match &v[0] {
                    FactValue::Str(s) => {
                        // Формат: «{actor} did {action:?} to {target:?} at {chapter}»
                        assert!(s.contains("Ivan"), "строка должна содержать actor: {}", s);
                        assert!(s.contains("Kill"), "строка должна содержать action: {}", s);
                        assert!(
                            s.contains("Petr"),
                            "строка должна содержать target: {}",
                            s
                        );
                        assert!(
                            s.contains("Глава 2"),
                            "строка должна содержать главу: {}",
                            s
                        );
                    }
                    other => panic!("ожидалась FactValue::Str в списке, получено {:?}", other),
                }
            }
            other => panic!("ожидалась FactValue::List, получено {:?}", other),
        }

        // Audit trail в WorldState: один из переходов — это Ivan.knowledge.
        let history = world.history();
        let knowledge_transition = history
            .iter()
            .find(|t| t.entity == "Ivan" && t.attribute == "knowledge")
            .expect("должен быть переход Ivan.knowledge в history");
        assert_eq!(knowledge_transition.caused_by_event, Some(event.id));
        assert!(knowledge_transition.old_value.is_none());
    }

    #[test]
    fn test_marry_action_sets_spouse_both_ways() {
        let engine = InferenceEngine::default_literary();
        let mut world = WorldState::new();
        let mut facts = FactLog::new();

        let event = make_event(
            "Anna",
            Action::Marry { partner: "Bob".to_string() },
            Some("Bob"),
            anchor(1),
        );
        let inferred = engine.apply_event(&event, &mut world, &mut facts);

        // marry_partner: 2 эффекта (SetAttribute spouse для Actor и для Target) → 2 InferredFact.
        assert_eq!(inferred.len(), 2);
        assert!(inferred.iter().all(|f| f.rule_name == "marry_partner"));

        // Anna.spouse = Bob (подстановка partner для Actor).
        assert_fact_eq(
            world.get("Anna", "spouse"),
            &FactValue::EntityRef("Bob".to_string()),
        );

        // Bob.spouse = Anna (подстановка event.actor для Target — симметрия брака).
        assert_fact_eq(
            world.get("Bob", "spouse"),
            &FactValue::EntityRef("Anna".to_string()),
        );

        // Audit: 2 факта в FactLog, оба Verified.
        let spouse_facts: Vec<&Fact> = facts
            .all_facts()
            .iter()
            .filter(|f| f.attribute == "spouse")
            .collect();
        assert_eq!(spouse_facts.len(), 2);
        assert!(spouse_facts.iter().all(|f| f.provenance == Provenance::Verified));
    }
}
