//! # World State — текущее состояние нарратива
//!
//! `WorldState` — единственный источник истины в reasoning engine.
//! Хранит текущие значения атрибутов каждой сущности (персонажа, локации,
//! артефакта) и историю всех переходов для отката и audit trail.
//!
//! ## Принцип
//! Каждый раз, когда факт становится истинным в нарративе (Пётр умер, Анна
//! переехала в Москву, Иван узнал тайну), это отражается в `WorldState` через
//! [`WorldState::set`] или [`WorldState::invalidate`]. История переходов
//! сохраняется и может быть использована для:
//! - отката к предыдущему снимку ([`WorldState::snapshot`] / [`WorldState::restore`]);
//! - диагностики противоречий (что было до изменения?);
//! - генерации объяснений для пользователя.
//!
//! ## Соглашения об атрибутах (документированы, не enforced)
//! - `alive: Bool` — `true` пока персонаж жив
//! - `location: Str` — текущая локация (`Unknown` после смерти)
//! - `knowledge: List<Str>` — факты, известные персонажу
//! - `goals: List<Str>` — активные цели
//! - `relationships: List<EntityRef>` — известные отношения
//! - `emotional_state: Str` — текущая эмоция
//! - `physical_state: Str` — `"healthy"`, `"wounded"`, `"dead"`, ...
//!
//! См. `docs/reasoning/SPEC.md` §2.7 для формального контракта.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::reasoning::facts::{EventId, FactValue};
use crate::reasoning::timeline::TemporalAnchor;

/// Идентификатор сущности. Совпадает с `LitNode.id` (см. SPEC §2.1).
/// Тот же тип, что и в `facts.rs` — обе стороны объявляют `pub type EntityId = String`.
pub type EntityId = String;

/// Имя атрибута (например, `"alive"`, `"location"`, `"knowledge"`).
pub type Attribute = String;

/// Запись об одном переходе состояния: сущность, атрибут, старое и новое
/// значения, событие-причина и момент времени. Добавляется в `history`
/// при каждом `set()`/`invalidate()`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateTransition {
    pub entity: EntityId,
    pub attribute: Attribute,
    pub old_value: Option<FactValue>,
    pub new_value: FactValue,
    pub caused_by_event: Option<EventId>,
    pub at: TemporalAnchor,
}

/// Сериализуемый снимок `current` и `now` для сохранения/восстановления.
///
/// Получается через [`WorldState::snapshot`] и применяется через
/// [`WorldState::restore`]. Может сериализоваться в JSON вместе с проектом.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSnapshot {
    /// Полная карта entity → attribute → value.
    pub current: HashMap<EntityId, HashMap<Attribute, FactValue>>,
    /// Момент времени, к которому относится снимок.
    pub now: TemporalAnchor,
}

/// World State — текущие значения атрибутов всех сущностей плюс история
/// переходов и текущий момент времени в нарративе.
///
/// Это **единственный источник истины**: ни LLM, ни текст нарратива не
/// определяют истину напрямую — только переходы, применённые через `set` /
/// `invalidate` после проверки ограничений.
pub struct WorldState {
    /// Текущее состояние каждой сущности: entity_id → attribute → value.
    current: HashMap<EntityId, HashMap<Attribute, FactValue>>,
    /// История изменений (для отката и audit trail).
    history: Vec<StateTransition>,
    /// Текущий момент времени в нарративе.
    now: TemporalAnchor,
}

impl WorldState {
    /// Создаёт пустое состояние с `now = chapter 1 / offset 0`.
    pub fn new() -> Self {
        Self {
            current: HashMap::new(),
            history: Vec::new(),
            now: TemporalAnchor {
                chapter_num: 1,
                chapter_suffix: None,
                scene_index: None,
                char_offset: 0,
            },
        }
    }

    /// Возвращает текущее значение атрибута `attr` сущности `entity`,
    /// или `None`, если атрибут ещё не установлен.
    pub fn get(&self, entity: &str, attr: &str) -> Option<&FactValue> {
        self.current
            .get(entity)
            .and_then(|attrs| attrs.get(attr))
    }

    /// Записывает новое значение атрибута.
    ///
    /// Старое значение (извлечённое из `transition.old_value`) попадает в
    /// `history` как audit trail, а текущее состояние обновляется на `value`.
    ///
    /// Вызывающий обязан гарантировать консистентность `transition`:
    /// - `transition.entity` соответствует `entity`;
    /// - `transition.attribute` соответствует `attr`;
    /// - `transition.old_value` отражает действительное предыдущее значение
    ///   (или `None`, если атрибута не было);
    /// - `transition.new_value` совпадает с `value`.
    ///
    /// Эти инварианты не проверяются здесь намеренно: фабрика переходов
    /// (rules/inference) отвечает за корректность, а `set` остаётся
    /// дешёвой операцией записи.
    pub fn set(
        &mut self,
        entity: &str,
        attr: Attribute,
        value: FactValue,
        transition: StateTransition,
    ) {
        // Записываем переход в историю (audit trail).
        self.history.push(transition);
        // Обновляем текущее состояние новым значением.
        self.current
            .entry(entity.to_string())
            .or_default()
            .insert(attr, value);
    }

    /// Возвращает `true`, если у сущности `entity` установлен атрибут `attr`
    /// (включая случай, когда значение — `FactValue::Unknown`).
    pub fn has_attribute(&self, entity: &str, attr: &str) -> bool {
        self.current
            .get(entity)
            .map(|attrs| attrs.contains_key(attr))
            .unwrap_or(false)
    }

    /// Находит все сущности, у которых атрибут `attr` равен `value`.
    ///
    /// Сравнение выполняется по структурному равенству (см. приватную
    /// `fact_value_eq`), так как `FactValue` намеренно не выводит `PartialEq`.
    /// Порядок возвращаемых идентификаторов не гарантирован (HashMap).
    pub fn entities_with(&self, attr: &str, value: &FactValue) -> Vec<EntityId> {
        let mut result = Vec::new();
        for (entity_id, attrs) in &self.current {
            if let Some(v) = attrs.get(attr) {
                if fact_value_eq(v, value) {
                    result.push(entity_id.clone());
                }
            }
        }
        result
    }

    /// Сдвигает текущий момент времени на `anchor`.
    ///
    /// **Паникует**, если `anchor` раньше текущего `now`: это нарушило бы
    /// причинно-следственную связь (нельзя применять события прошлого к
    /// уже построенному настоящему). Для отката используйте `restore`.
    pub fn advance_to(&mut self, anchor: &TemporalAnchor) {
        if anchor.before(&self.now) {
            panic!(
                "advance_to: попытка вернуться в прошлое \
                 (now = {:?}, запрошенный anchor = {:?}) — используйте restore() для отката",
                self.now, anchor
            );
        }
        self.now = anchor.clone();
    }

    /// Текущий момент времени в нарративе.
    pub fn now(&self) -> &TemporalAnchor {
        &self.now
    }

    /// История переходов состояния (audit trail). Только для чтения.
    pub fn history(&self) -> &[StateTransition] {
        &self.history
    }

    /// Создаёт сериализуемый снимок текущего состояния и момента времени.
    pub fn snapshot(&self) -> WorldSnapshot {
        WorldSnapshot {
            current: self.current.clone(),
            now: self.now.clone(),
        }
    }

    /// Восстанавливает состояние из снимка.
    ///
    /// `current` и `now` заменяются значениями из снимка. В `history`
    /// добавляется синтетическая запись `__restore__`, чтобы audit trail
    /// отражал факт отката (но не терял предыдущие переходы).
    pub fn restore(&mut self, snap: WorldSnapshot) {
        let at = snap.now.clone();
        self.current = snap.current;
        self.now = at.clone();
        self.history.push(StateTransition {
            entity: String::new(),
            attribute: "__restore__".to_string(),
            old_value: None,
            new_value: FactValue::Str("Состояние восстановлено из снимка".to_string()),
            caused_by_event: None,
            at,
        });
    }

    /// Обнуляет атрибут: устанавливает его в [`FactValue::Unknown`] и
    /// записывает переход в историю.
    ///
    /// Типичный сценарий: персонаж умирает → его `location` становится
    /// неизвестной (тело может быть где угодно, narратив об этом не говорит).
    pub fn invalidate(
        &mut self,
        entity: &str,
        attr: &str,
        caused_by_event: Option<EventId>,
    ) {
        let old_value = self
            .current
            .get(entity)
            .and_then(|attrs| attrs.get(attr))
            .cloned();
        let transition = StateTransition {
            entity: entity.to_string(),
            attribute: attr.to_string(),
            old_value,
            new_value: FactValue::Unknown,
            caused_by_event,
            at: self.now.clone(),
        };
        self.current
            .entry(entity.to_string())
            .or_default()
            .insert(attr.to_string(), FactValue::Unknown);
        self.history.push(transition);
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}

/// Структурное сравнение [`FactValue`] без требования `PartialEq` на самом типе
/// (SPEC §2.5 не выводит `PartialEq` для `FactValue`).
///
/// - `Float` сравнивается через `==` (NaN != NaN — стандартное поведение).
/// - `List` сравнивается поэлементно (включая длину).
/// - Разные варианты всегда не равны (`Bool(true) != Str("true")`).
fn fact_value_eq(a: &FactValue, b: &FactValue) -> bool {
    match (a, b) {
        (FactValue::Bool(x), FactValue::Bool(y)) => x == y,
        (FactValue::Str(x), FactValue::Str(y)) => x == y,
        (FactValue::Int(x), FactValue::Int(y)) => x == y,
        (FactValue::Float(x), FactValue::Float(y)) => x == y,
        (FactValue::EntityRef(x), FactValue::EntityRef(y)) => x == y,
        (FactValue::List(x), FactValue::List(y)) => {
            x.len() == y.len()
                && x.iter().zip(y.iter()).all(|(xi, yi)| fact_value_eq(xi, yi))
        }
        (FactValue::Unknown, FactValue::Unknown) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Хелпер для построения `TemporalAnchor` в тестах.
    fn anchor(chapter: u32, offset: usize) -> TemporalAnchor {
        TemporalAnchor {
            chapter_num: chapter,
            chapter_suffix: None,
            scene_index: None,
            char_offset: offset,
        }
    }

    /// Хелпер: проверить, что `actual` равен `expected` (без `PartialEq` на FactValue).
    fn assert_fact_eq(actual: Option<&FactValue>, expected: &FactValue) {
        match actual {
            Some(v) => assert!(
                fact_value_eq(v, expected),
                "ожидалось {:?}, получено {:?}",
                expected,
                v
            ),
            None => panic!("ожидалось {:?}, получено None", expected),
        }
    }

    #[test]
    fn test_set_and_get_attribute() {
        let mut state = WorldState::new();
        let at = state.now().clone();

        // Устанавливаем атрибут "alive" = true для Петра.
        state.set(
            "Petr",
            "alive".to_string(),
            FactValue::Bool(true),
            StateTransition {
                entity: "Petr".to_string(),
                attribute: "alive".to_string(),
                old_value: None,
                new_value: FactValue::Bool(true),
                caused_by_event: Some(1),
                at,
            },
        );

        assert_fact_eq(state.get("Petr", "alive"), &FactValue::Bool(true));
        assert!(state.has_attribute("Petr", "alive"));
        assert!(!state.has_attribute("Petr", "location"));
        assert!(!state.has_attribute("Ivan", "alive"));
        assert!(state.get("Ivan", "alive").is_none());
    }

    #[test]
    fn test_history_records_transitions() {
        let mut state = WorldState::new();
        let at = state.now().clone();

        // Первый переход: Пётр приезжает в Москву.
        state.set(
            "Petr",
            "location".to_string(),
            FactValue::Str("Москва".to_string()),
            StateTransition {
                entity: "Petr".to_string(),
                attribute: "location".to_string(),
                old_value: None,
                new_value: FactValue::Str("Москва".to_string()),
                caused_by_event: Some(10),
                at: at.clone(),
            },
        );

        // Второй переход: Пётр переезжает в Петербург.
        state.set(
            "Petr",
            "location".to_string(),
            FactValue::Str("Петербург".to_string()),
            StateTransition {
                entity: "Petr".to_string(),
                attribute: "location".to_string(),
                old_value: Some(FactValue::Str("Москва".to_string())),
                new_value: FactValue::Str("Петербург".to_string()),
                caused_by_event: Some(11),
                at: at.clone(),
            },
        );

        let history = state.history();
        assert_eq!(history.len(), 2, "ожидается 2 перехода в истории");

        // Первый переход: старого значения не было.
        assert!(history[0].old_value.is_none());
        assert_fact_eq(Some(&history[0].new_value), &FactValue::Str("Москва".to_string()));
        assert_eq!(history[0].caused_by_event, Some(10));

        // Второй переход: старое значение = Москва.
        assert_fact_eq(
            history[1].old_value.as_ref(),
            &FactValue::Str("Москва".to_string()),
        );
        assert_fact_eq(Some(&history[1].new_value), &FactValue::Str("Петербург".to_string()));

        // Текущее состояние — Петербург.
        assert_fact_eq(
            state.get("Petr", "location"),
            &FactValue::Str("Петербург".to_string()),
        );
    }

    #[test]
    fn test_invalidate_sets_unknown() {
        let mut state = WorldState::new();
        let at = state.now().clone();

        // Пётр живёт в Москве.
        state.set(
            "Petr",
            "location".to_string(),
            FactValue::Str("Москва".to_string()),
            StateTransition {
                entity: "Petr".to_string(),
                attribute: "location".to_string(),
                old_value: None,
                new_value: FactValue::Str("Москва".to_string()),
                caused_by_event: Some(1),
                at,
            },
        );
        assert_fact_eq(
            state.get("Petr", "location"),
            &FactValue::Str("Москва".to_string()),
        );

        // Пётр умирает → location становится Unknown.
        state.invalidate("Petr", "location", Some(99));

        assert_fact_eq(state.get("Petr", "location"), &FactValue::Unknown);
        assert!(
            state.has_attribute("Petr", "location"),
            "после invalidate атрибут всё ещё считается установленным (со значением Unknown)"
        );

        // История содержит переход с new_value = Unknown.
        let history = state.history();
        let last = history.last().expect("история не должна быть пустой");
        assert_fact_eq(Some(&last.new_value), &FactValue::Unknown);
        assert_fact_eq(
            last.old_value.as_ref(),
            &FactValue::Str("Москва".to_string()),
        );
        assert_eq!(last.caused_by_event, Some(99));
        assert_eq!(last.entity, "Petr");
        assert_eq!(last.attribute, "location");
    }

    #[test]
    fn test_snapshot_and_restore() {
        let mut state = WorldState::new();
        let at = state.now().clone();

        // Состояние A: Пётр в Москве.
        state.set(
            "Petr",
            "location".to_string(),
            FactValue::Str("Москва".to_string()),
            StateTransition {
                entity: "Petr".to_string(),
                attribute: "location".to_string(),
                old_value: None,
                new_value: FactValue::Str("Москва".to_string()),
                caused_by_event: Some(1),
                at: at.clone(),
            },
        );

        // Делаем снимок состояния A.
        let snap = state.snapshot();
        assert_eq!(snap.now.chapter_num, 1);
        assert!(snap.current.contains_key("Petr"));
        assert_fact_eq(
            snap.current
                .get("Petr")
                .and_then(|a| a.get("location")),
            &FactValue::Str("Москва".to_string()),
        );

        // Состояние B: Пётр переехал в Петербург.
        state.set(
            "Petr",
            "location".to_string(),
            FactValue::Str("Петербург".to_string()),
            StateTransition {
                entity: "Petr".to_string(),
                attribute: "location".to_string(),
                old_value: Some(FactValue::Str("Москва".to_string())),
                new_value: FactValue::Str("Петербург".to_string()),
                caused_by_event: Some(2),
                at: at.clone(),
            },
        );
        assert_fact_eq(
            state.get("Petr", "location"),
            &FactValue::Str("Петербург".to_string()),
        );

        // Восстанавливаемся в снимок A.
        state.restore(snap);

        assert_fact_eq(
            state.get("Petr", "location"),
            &FactValue::Str("Москва".to_string()),
        );
        assert_eq!(state.now().chapter_num, 1);

        // История: set Москва, set Петербург, synthetic __restore__.
        let history = state.history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[2].attribute, "__restore__");
        assert!(history[2].entity.is_empty());
        assert!(history[2].caused_by_event.is_none());
        match &history[2].new_value {
            FactValue::Str(msg) => {
                assert!(
                    msg.contains("восстановлено"),
                    "synthetic note должна упоминать восстановление, получено: {}",
                    msg
                );
            }
            other => panic!("ожидалась FactValue::Str для synthetic note, получено {:?}", other),
        }
    }

    #[test]
    fn test_advance_to_updates_now() {
        let mut state = WorldState::new();
        // Начальное now = chapter 1 / offset 0.
        assert_eq!(state.now().chapter_num, 1);
        assert_eq!(state.now().char_offset, 0);

        // Переход к главе 2.
        state.advance_to(&anchor(2, 0));
        assert_eq!(state.now().chapter_num, 2);

        // Переход вперёд в той же главе (больший offset).
        state.advance_to(&anchor(2, 500));
        assert_eq!(state.now().char_offset, 500);

        // Равный anchor (idempotent) — не паникует.
        state.advance_to(&anchor(2, 500));
        assert_eq!(state.now().char_offset, 500);

        // Переход к главе 5 со сценой.
        state.advance_to(&anchor(5, 100));
        assert_eq!(state.now().chapter_num, 5);
        assert_eq!(state.now().scene_index, None);
    }

    #[test]
    fn test_entities_with_finds_matching() {
        let mut state = WorldState::new();
        let at = state.now().clone();

        // Три персонажа в разных локациях.
        let setup: &[(&str, &str)] = &[
            ("Petr", "Замок"),
            ("Ivan", "Замок"),
            ("Anna", "Лес"),
        ];
        for (entity, location) in setup {
            state.set(
                *entity,
                "location".to_string(),
                FactValue::Str(location.to_string()),
                StateTransition {
                    entity: entity.to_string(),
                    attribute: "location".to_string(),
                    old_value: None,
                    new_value: FactValue::Str(location.to_string()),
                    caused_by_event: None,
                    at: at.clone(),
                },
            );
        }

        let in_castle = state.entities_with("location", &FactValue::Str("Замок".to_string()));
        assert_eq!(in_castle.len(), 2, "в Замке должно быть 2 персонажа");
        assert!(in_castle.contains(&"Petr".to_string()));
        assert!(in_castle.contains(&"Ivan".to_string()));

        let in_forest = state.entities_with("location", &FactValue::Str("Лес".to_string()));
        assert_eq!(in_forest.len(), 1);
        assert!(in_forest.contains(&"Anna".to_string()));

        // Неизвестное значение → пустой результат.
        let nowhere = state.entities_with("location", &FactValue::Str("Нигде".to_string()));
        assert!(nowhere.is_empty());

        // Unknown-атрибут не должен совпадать со строкой.
        state.invalidate("Petr", "location", None);
        let still_in_castle =
            state.entities_with("location", &FactValue::Str("Замок".to_string()));
        assert_eq!(still_in_castle.len(), 1, "после invalidate Пётр больше не в Замке");
        assert!(still_in_castle.contains(&"Ivan".to_string()));

        // Unknown-атрибут должен находиться через FactValue::Unknown.
        let unknown_loc = state.entities_with("location", &FactValue::Unknown);
        assert_eq!(unknown_loc.len(), 1);
        assert!(unknown_loc.contains(&"Petr".to_string()));
    }
}
