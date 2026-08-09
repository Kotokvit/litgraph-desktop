//! constraints.rs — Constraint, ConstraintEngine, ConstraintViolation.
//!
//! Декларативные инварианты нарратива. Каждый [`Constraint`] говорит:
//! «если у сущности `attribute == value`, то действие `forbids` невозможно».
//! [`ConstraintEngine`] проверяет каждое новое событие против набора
//! ограничений и возвращает [`ConstraintViolation`] для каждого нарушенного
//! инварианта.
//!
//! ## Принципы
//!
//! 1. **State is truth.** Ограничение сверяется с [`WorldState`], а не с
//!    текстом нарратива. Если `WorldState` говорит, что персонаж мёртв
//!    (`alive = false`), то любое `Action::Speak` от него нарушает инвариант.
//! 2. **Declarative.** [`Constraint`] — статическая декларация: «condition →
//!    forbids action». Никакой динамики, никакого вывода.
//! 3. **No LLM.** Ограничения не вызывают LLM — это чисто алгоритмический
//!    слой.
//! 4. **Russian-first UI strings** (в сообщениях/комментариях для
//!    пользователя), английские идентификаторы.
//!
//! ## Семантика matching'а (ВАЖНО)
//!
//! Сравнение `forbids` и `event.action` идёт по дискриминанту варианта для
//! большинства [`Action`], что позволяет одной декларацией запретить «любой
//! `Speak`» или «любой `Move`», независимо от payload (topic, destination,
//! ...). Исключение — `Action::Custom`: он сопоставляется только по
//! полярности (`Positive` / `Negative` / `Neutral`), поскольку `verb_lemma`
//! у разных событий различается, а семантическая окраска обычно достаточна
//! для инвариантов.
//!
//! См. приватную функцию [`action_forbidden`] ниже.
//!
//! ## SPEC deviation (документировано)
//!
//! SPEC §2.9 определяет `Constraint.forbids: Action` (одно действие на
//! ограничение). Для запрета «мёртвый персонаж не может физически действовать»
//! (8 физических `Action`-вариантов: Hit / Kill / Wound / Capture / Imprison /
//! Free / Heal / Touch) мы пишем **8 отдельных `Constraint`** с одинаковым
//! `when` и разными `forbids`, а не расширяем `forbids` до `Vec<Action>`.
//! Это сохраняет совместимость со SPEC и позволяет точечно диагностировать,
//! какое именно физическое действие попытался совершить мёртвый персонаж
//! (имя нарушения включает суффикс действия, например
//! `dead_cannot_act_physically_kill`).

use serde::{Deserialize, Serialize};

use crate::reasoning::facts::{Action, Event, EventId, FactId, FactValue};
use crate::reasoning::state::{Attribute, WorldState};
use crate::reasoning::timeline::TemporalAnchor;

// ============================================================================
// ConstraintCondition
// ============================================================================

/// Условие срабатывания ограничения: «у сущности `attribute == equals`».
///
/// Используется полем [`Constraint::when`]. Семантически — это простая
/// проверка `state.get(entity, &attribute) == Some(&equals)`. Если атрибут
/// отсутствует или имеет другое значение — условие не выполнено (ограничение
/// не применяется, событие разрешено).
#[derive(Debug, Clone)]
pub struct ConstraintCondition {
    /// Имя атрибута («alive», «imprisoned», «captured», ...).
    pub attribute: Attribute,
    /// Ожидаемое значение атрибута.
    pub equals: FactValue,
}

impl ConstraintCondition {
    /// `true`, если у сущности `entity` в `state` атрибут `attribute` имеет
    /// значение `equals`. Если атрибут отсутствует — `false`.
    ///
    /// Сравнение производится через `PartialEq` на `FactValue` (реализован
    /// вручную в `facts.rs`, поддерживает структурное равенство с учётом
    /// тега варианта — `Bool(true) != Int(1)`).
    pub fn is_met_by(&self, state: &WorldState, entity: &str) -> bool {
        match state.get(entity, &self.attribute) {
            Some(v) => v == &self.equals,
            None => false,
        }
    }
}

// ============================================================================
// Constraint
// ============================================================================

/// Декларативный инвариант: «если у сущности условие `when` выполнено, то
/// действие `forbids` невозможно».
///
/// Каждый `Constraint` уникален по `name` — это идентификатор, попадающий в
/// [`ConstraintViolation::constraint_name`] для диагностики.
#[derive(Debug, Clone)]
pub struct Constraint {
    /// Идентификатор ограничения (например, `"dead_cannot_speak"`).
    pub name: &'static str,
    /// Условие срабатывания: «когда `entity.attribute == equals`».
    pub when: ConstraintCondition,
    /// Запрещённое действие. Payload-агностично: сравнение по дискриминанту
    /// (кроме `Custom` — там по полярности). См. [`action_forbidden`].
    pub forbids: Action,
    /// Человекочитаемое объяснение на русском (для UI и диагностики).
    pub reason: String,
}

// ============================================================================
// ConstraintViolation
// ============================================================================

/// Фиксация нарушения ограничения конкретным событием. Возвращается
/// [`ConstraintEngine::check`] / [`ConstraintEngine::check_all`].
///
/// Поле `conflicting_fact` пока всегда `None` — Wave 2 `contradictions.rs`
/// обогатит его реальной ссылкой на `FactId`, породивший конфликт.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintViolation {
    /// Имя нарушенного ограничения (`Constraint::name`).
    pub constraint_name: String,
    /// ID события, попытавшегося нарушить инвариант.
    pub event_id: EventId,
    /// Кто совершил запрещённое действие (`Event::actor`).
    pub actor: String,
    /// Какое действие было попытано (`Event::action`).
    pub attempted_action: Action,
    /// Человекочитаемое объяснение нарушения (скопировано из
    /// `Constraint::reason`).
    pub reason: String,
    /// Опциональная ссылка на конфликтующий факт (заполняется Wave 2
    /// `contradictions.rs`). На текущей волне — `None`.
    pub conflicting_fact: Option<FactId>,
    /// Момент времени, в которое произошло нарушение.
    pub at: TemporalAnchor,
}

// ============================================================================
// ConstraintEngine
// ============================================================================

/// Движок проверки ограничений. Хранит набор [`Constraint`] и для каждого
/// нового [`Event`] возвращает `Vec<ConstraintViolation>` — список всех
/// нарушенных инвариантов.
///
/// Движок иммутабелен относительно проверок: `check` / `check_all` не
/// мутируют ни `self`, ни `state`. Добавление новых ограничений — через
/// [`ConstraintEngine::add`].
pub struct ConstraintEngine {
    constraints: Vec<Constraint>,
}

impl ConstraintEngine {
    /// Пустой движок без ограничений. Используйте
    /// [`ConstraintEngine::default_literary`] для предзаполненного набора
    /// или [`ConstraintEngine::add`] для добавления своих.
    pub fn new() -> Self {
        Self {
            constraints: Vec::new(),
        }
    }

    /// Стандартный набор ограничений для литературного нарратива.
    ///
    /// Включает 16 инвариантов:
    ///   - `dead_cannot_speak` — мёртвый не может говорить
    ///   - `dead_cannot_move` — мёртвый не может перемещаться
    ///   - 8 × `dead_cannot_act_physically_*` — мёртвый не может физически
    ///     действовать (`Hit` / `Kill` / `Wound` / `Capture` / `Imprison` /
    ///     `Free` / `Heal` / `Touch`)
    ///   - `imprisoned_cannot_move` — заключённый не может перемещаться
    ///   - `imprisoned_cannot_speak_freely` — заключённый не может свободно
    ///     рассказывать
    ///   - `captured_cannot_betray` — пленённый не может предавать
    ///   - `dead_cannot_die_again` — мёртвый не может умереть снова
    ///   - `dead_cannot_marry` — мёртвый не может вступить в брак
    ///   - `dead_cannot_know_new_facts` — мёртвый не может узнавать новые
    ///     факты
    pub fn default_literary() -> Self {
        let mut engine = Self::new();

        // ── a) dead_cannot_speak ──────────────────────────────────────────
        engine.add(Constraint {
            name: "dead_cannot_speak",
            when: ConstraintCondition {
                attribute: "alive".to_string(),
                equals: FactValue::Bool(false),
            },
            forbids: Action::Speak { topic: None },
            reason: "Невозможно: персонаж мёртв, но пытается говорить".to_string(),
        });

        // ── b) dead_cannot_move ───────────────────────────────────────────
        engine.add(Constraint {
            name: "dead_cannot_move",
            when: ConstraintCondition {
                attribute: "alive".to_string(),
                equals: FactValue::Bool(false),
            },
            forbids: Action::Move {
                destination: String::new(),
            },
            reason: "Невозможно: персонаж мёртв, но перемещается".to_string(),
        });

        // ── c) dead_cannot_act_physically (8 инвариантов, по одному на Action)
        // SPEC deviation: вместо `forbids: Vec<Action>` (которое нарушило бы
        // SPEC §2.9) пишем 8 отдельных `Constraint` с одинаковым `when`,
        // разными `forbids`. Каждое имеет уникальное имя с суффиксом
        // действия, что позволяет точечно диагностировать нарушение.
        for (name, forbids) in [
            ("dead_cannot_act_physically_hit", Action::Hit),
            ("dead_cannot_act_physically_kill", Action::Kill),
            ("dead_cannot_act_physically_wound", Action::Wound),
            ("dead_cannot_act_physically_capture", Action::Capture),
            ("dead_cannot_act_physically_imprison", Action::Imprison),
            ("dead_cannot_act_physically_free", Action::Free),
            ("dead_cannot_act_physically_heal", Action::Heal),
            ("dead_cannot_act_physically_touch", Action::Touch),
        ] {
            engine.add(Constraint {
                name,
                when: ConstraintCondition {
                    attribute: "alive".to_string(),
                    equals: FactValue::Bool(false),
                },
                forbids,
                reason: "Невозможно: мёртвый персонаж не может физически действовать"
                    .to_string(),
            });
        }

        // ── d) imprisoned_cannot_move ────────────────────────────────────
        engine.add(Constraint {
            name: "imprisoned_cannot_move",
            when: ConstraintCondition {
                attribute: "imprisoned".to_string(),
                equals: FactValue::Bool(true),
            },
            forbids: Action::Move {
                destination: String::new(),
            },
            reason: "Невозможно: персонаж в заточении, не может переместиться".to_string(),
        });

        // ── e) imprisoned_cannot_speak_freely ────────────────────────────
        engine.add(Constraint {
            name: "imprisoned_cannot_speak_freely",
            when: ConstraintCondition {
                attribute: "imprisoned".to_string(),
                equals: FactValue::Bool(true),
            },
            forbids: Action::Tell {
                topic: String::new(),
                to: String::new(),
            },
            reason: "Невозможно: заключённый не может свободно рассказывать".to_string(),
        });

        // ── f) captured_cannot_betray ────────────────────────────────────
        engine.add(Constraint {
            name: "captured_cannot_betray",
            when: ConstraintCondition {
                attribute: "captured".to_string(),
                equals: FactValue::Bool(true),
            },
            forbids: Action::Betray {
                victim: String::new(),
            },
            reason: "Пленённый персонаж не может совершать предательства".to_string(),
        });

        // ── g) dead_cannot_die_again ─────────────────────────────────────
        engine.add(Constraint {
            name: "dead_cannot_die_again",
            when: ConstraintCondition {
                attribute: "alive".to_string(),
                equals: FactValue::Bool(false),
            },
            forbids: Action::Die,
            reason: "Персонаж уже мёртв".to_string(),
        });

        // ── h) dead_cannot_marry ─────────────────────────────────────────
        engine.add(Constraint {
            name: "dead_cannot_marry",
            when: ConstraintCondition {
                attribute: "alive".to_string(),
                equals: FactValue::Bool(false),
            },
            forbids: Action::Marry {
                partner: String::new(),
            },
            reason: "Мёртвый персонаж не может вступить в брак".to_string(),
        });

        // ── i) dead_cannot_know_new_facts ────────────────────────────────
        engine.add(Constraint {
            name: "dead_cannot_know_new_facts",
            when: ConstraintCondition {
                attribute: "alive".to_string(),
                equals: FactValue::Bool(false),
            },
            forbids: Action::Know {
                fact: String::new(),
            },
            reason: "Мёртвый персонаж не может узнавать новые факты".to_string(),
        });

        engine
    }

    /// Добавляет ограничение в движок. Порядок добавления сохраняется при
    /// итерации в [`ConstraintEngine::check`] (первый добавленный — первый
    /// проверяемый; порядок violation в результате соответствует порядку
    /// ограничений).
    pub fn add(&mut self, c: Constraint) {
        self.constraints.push(c);
    }

    /// Проверяет событие `event` против всех ограничений. Возвращает
    /// `Vec<ConstraintViolation>` — один элемент на каждое нарушенное
    /// ограничение (возможно несколько violation для одного события, если
    /// оно нарушает сразу несколько инвариантов).
    ///
    /// Алгоритм:
    /// 1. Для каждого ограничения проверяем, попадает ли `event.action` под
    ///    запрет `constraint.forbids` (см. [`action_forbidden`]).
    /// 2. Если да — проверяем условие `constraint.when.is_met_by(state,
    ///    &event.actor)`.
    /// 3. Если оба условия выполнены — emit violation с populated полями.
    ///
    /// `conflicting_fact` всегда `None` (заполняется в `contradictions.rs`,
    /// Wave 2).
    pub fn check(&self, state: &WorldState, event: &Event) -> Vec<ConstraintViolation> {
        let mut violations = Vec::new();
        for c in &self.constraints {
            // Шаг 1: действие события попадает под запрет ограничения?
            if !action_forbidden(&c.forbids, &event.action) {
                continue;
            }
            // Шаг 2: условие срабатывания выполнено для этого actor'а?
            if !c.when.is_met_by(state, &event.actor) {
                continue;
            }
            // Шаг 3: emit violation.
            violations.push(ConstraintViolation {
                constraint_name: c.name.to_string(),
                event_id: event.id,
                actor: event.actor.clone(),
                attempted_action: event.action.clone(),
                reason: c.reason.clone(),
                conflicting_fact: None,
                at: event.time.clone(),
            });
        }
        violations
    }

    /// Пакетная проверка: применяет [`ConstraintEngine::check`] к каждому
    /// событию в порядке следования и собирает все нарушения в один вектор.
    ///
    /// NB: состояние `state` НЕ мутируется между событиями — это статический
    /// снэпшот. Если вам нужна проверка «по ходу построения состояния»,
    /// вызывайте `check()` для каждого события в цикле, обновляя `state`
    /// между итерациями. Это ответственность `cycle.rs` (Wave 4).
    pub fn check_all(
        &self,
        state: &WorldState,
        events: &[Event],
    ) -> Vec<ConstraintViolation> {
        let mut all = Vec::new();
        for ev in events {
            let mut v = self.check(state, ev);
            all.append(&mut v);
        }
        all
    }

    /// Количество зарегистрированных ограничений.
    pub fn len(&self) -> usize {
        self.constraints.len()
    }

    /// `true`, если ограничений нет.
    pub fn is_empty(&self) -> bool {
        self.constraints.is_empty()
    }
}

impl Default for ConstraintEngine {
    /// Делегирует в [`ConstraintEngine::default_literary`] — стандартный
    /// набор литературных ограничений. Это соответствует конвенции проекта
    /// (см. `rules.rs::RuleSet::default()`, который также возвращает
    /// `default_literary()`).
    ///
    /// Для пустого движка используйте [`ConstraintEngine::new`].
    fn default() -> Self {
        Self::default_literary()
    }
}

// ============================================================================
// action_forbidden — семантика matching'а
// ============================================================================

/// Проверяет, попадает ли `attempted` действие под запрет `forbids`.
///
/// Правила сопоставления:
/// - `Action::Custom { polarity: p1, .. }` vs `Action::Custom { polarity: p2,
///   .. }`: совпадение, если `p1 == p2` (полярность — единственная
///   семантически значимая часть для инвариантов; `verb_lemma` — wildcard).
/// - Все остальные варианты: совпадение по `std::mem::discriminant`, что
///   позволяет запретить «любой `Move`», «любой `Speak`» и т.п. одной
///   декларацией, независимо от payload (destination, topic, ...).
/// - Разные варианты: всегда не совпадает (`Hit` не запрещает `Kill`).
fn action_forbidden(forbids: &Action, attempted: &Action) -> bool {
    use std::mem::discriminant;
    match (forbids, attempted) {
        (
            Action::Custom {
                polarity: p1, ..
            },
            Action::Custom {
                polarity: p2, ..
            },
        ) => p1 == p2,
        (a, b) => discriminant(a) == discriminant(b),
    }
}

// ============================================================================
// Юнит-тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::facts::{Provenance, VerbPolarity};
    use crate::reasoning::state::StateTransition;

    /// Хелпер: построить `TemporalAnchor` для тестов.
    fn anchor(chapter: u32) -> TemporalAnchor {
        TemporalAnchor {
            chapter_num: chapter,
            chapter_suffix: None,
            scene_index: None,
            char_offset: 0,
        }
    }

    /// Хелпер: построить `Event` с минимально достаточными полями.
    fn make_event(id: u64, actor: &str, action: Action, chapter: u32) -> Event {
        Event {
            id,
            actor: actor.to_string(),
            action,
            target: None,
            instrument: None,
            time: anchor(chapter),
            source_text: String::new(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        }
    }

    /// Хелпер: установить у сущности булев атрибут в `WorldState`.
    fn set_bool(state: &mut WorldState, entity: &str, attr: &str, value: bool) {
        let at = state.now().clone();
        state.set(
            entity,
            attr.to_string(),
            FactValue::Bool(value),
            StateTransition {
                entity: entity.to_string(),
                attribute: attr.to_string(),
                old_value: None,
                new_value: FactValue::Bool(value),
                caused_by_event: None,
                at,
            },
        );
    }

    #[test]
    fn test_dead_character_cannot_speak() {
        let mut state = WorldState::new();
        // Пётр мёртв.
        set_bool(&mut state, "Petr", "alive", false);

        let engine = ConstraintEngine::default_literary();
        let event = make_event(1, "Petr", Action::Speak { topic: None }, 12);
        let violations = engine.check(&state, &event);

        assert_eq!(violations.len(), 1, "должно быть ровно одно нарушение");
        let v = &violations[0];
        assert_eq!(v.constraint_name, "dead_cannot_speak");
        assert_eq!(v.event_id, 1);
        assert_eq!(v.actor, "Petr");
        assert!(v.reason.contains("мёртв"), "reason должен содержать 'мёртв'");
        assert!(
            v.conflicting_fact.is_none(),
            "conflicting_fact должен быть None на текущей волне"
        );
        assert_eq!(v.at.chapter_num, 12);
    }

    #[test]
    fn test_alive_character_can_speak() {
        let mut state = WorldState::new();
        // Иван жив.
        set_bool(&mut state, "Ivan", "alive", true);

        let engine = ConstraintEngine::default_literary();
        let event = make_event(1, "Ivan", Action::Speak { topic: None }, 5);
        let violations = engine.check(&state, &event);

        assert!(
            violations.is_empty(),
            "живой персонаж может говорить — нарушений быть не должно, получено: {:?}",
            violations
        );
    }

    #[test]
    fn test_dead_character_cannot_move() {
        let mut state = WorldState::new();
        set_bool(&mut state, "Petr", "alive", false);

        let engine = ConstraintEngine::default_literary();
        let event = make_event(
            7,
            "Petr",
            Action::Move {
                destination: "Замок".to_string(),
            },
            13,
        );
        let violations = engine.check(&state, &event);

        assert_eq!(violations.len(), 1, "должно быть одно нарушение");
        assert_eq!(violations[0].constraint_name, "dead_cannot_move");
        // Payload должен быть сохранён в attempted_action.
        match &violations[0].attempted_action {
            Action::Move { destination } => {
                assert_eq!(destination, "Замок", "destination должен сохраниться");
            }
            other => panic!("ожидался Action::Move, получено {:?}", other),
        }
    }

    #[test]
    fn test_imprisoned_character_cannot_move() {
        let mut state = WorldState::new();
        // Анна жива, но заключена.
        set_bool(&mut state, "Anna", "alive", true);
        set_bool(&mut state, "Anna", "imprisoned", true);

        let engine = ConstraintEngine::default_literary();
        let event = make_event(
            42,
            "Anna",
            Action::Move {
                destination: "Лес".to_string(),
            },
            7,
        );
        let violations = engine.check(&state, &event);

        // Должно сработать именно imprisoned_cannot_move (Anna жива, поэтому
        // dead_cannot_move не применяется).
        let names: Vec<&str> =
            violations.iter().map(|v| v.constraint_name.as_str()).collect();
        assert!(
            names.contains(&"imprisoned_cannot_move"),
            "ожидается нарушение imprisoned_cannot_move, получены: {:?}",
            names
        );
        assert!(
            !names.contains(&"dead_cannot_move"),
            "Anna жива — dead_cannot_move не должен сработать"
        );
        assert_eq!(violations.len(), 1, "ожидалось ровно одно нарушение");
    }

    #[test]
    fn test_action_forbidden_uses_discriminant() {
        // Speak с topic=None (как в dead_cannot_speak) должен совпадать с
        // любым Speak, независимо от topic.
        let forbids = Action::Speak { topic: None };
        let attempted_with_topic =
            Action::Speak { topic: Some("война".to_string()) };
        let attempted_no_topic = Action::Speak { topic: None };

        assert!(action_forbidden(&forbids, &attempted_with_topic));
        assert!(action_forbidden(&forbids, &attempted_no_topic));

        // Move с пустой destination должен совпадать с любым Move.
        let forbids_move = Action::Move {
            destination: String::new(),
        };
        let attempted_move = Action::Move {
            destination: "Москва".to_string(),
        };
        assert!(action_forbidden(&forbids_move, &attempted_move));

        // Разные варианты не совпадают.
        assert!(!action_forbidden(&forbids_move, &Action::Die));
        assert!(!action_forbidden(&Action::Hit, &Action::Kill));
        assert!(!action_forbidden(&Action::Move { destination: String::new() }, &Action::Arrive { destination: String::new() }));
    }

    #[test]
    fn test_action_forbidden_custom_matches_by_polarity() {
        use VerbPolarity::*;
        // Positive forbids vs Positive attempted → совпадение (разные verb_lemma).
        assert!(action_forbidden(
            &Action::Custom {
                verb_lemma: String::new(),
                polarity: Positive,
            },
            &Action::Custom {
                verb_lemma: "спасти".to_string(),
                polarity: Positive,
            },
        ));
        // Negative forbids vs Negative attempted → совпадение.
        assert!(action_forbidden(
            &Action::Custom {
                verb_lemma: String::new(),
                polarity: Negative,
            },
            &Action::Custom {
                verb_lemma: "предать".to_string(),
                polarity: Negative,
            },
        ));
        // Разные полярности → не совпадает, даже если verb_lemma одинаковый.
        assert!(!action_forbidden(
            &Action::Custom {
                verb_lemma: "идти".to_string(),
                polarity: Positive,
            },
            &Action::Custom {
                verb_lemma: "идти".to_string(),
                polarity: Negative,
            },
        ));
        // Neutral vs Neutral → совпадение.
        assert!(action_forbidden(
            &Action::Custom {
                verb_lemma: String::new(),
                polarity: Neutral,
            },
            &Action::Custom {
                verb_lemma: "сказать".to_string(),
                polarity: Neutral,
            },
        ));
        // Custom vs не-Custom → никогда не совпадает (по discriminant).
        assert!(!action_forbidden(
            &Action::Custom {
                verb_lemma: String::new(),
                polarity: Positive,
            },
            &Action::Hit,
        ));
        assert!(!action_forbidden(
            &Action::Kill,
            &Action::Custom {
                verb_lemma: String::new(),
                polarity: Negative,
            },
        ));
    }

    #[test]
    fn test_check_all_returns_violations_for_multiple_events() {
        let mut state = WorldState::new();
        // Пётр мёртв, Анна заключена (но жива), Иван жив и свободен.
        set_bool(&mut state, "Petr", "alive", false);
        set_bool(&mut state, "Anna", "alive", true);
        set_bool(&mut state, "Anna", "imprisoned", true);
        set_bool(&mut state, "Ivan", "alive", true);

        let engine = ConstraintEngine::default_literary();
        let events = vec![
            // event 1: Пётр (мёртв) пытается говорить → нарушение.
            make_event(1, "Petr", Action::Speak { topic: None }, 5),
            // event 2: Анна (заключена) пытается переместиться → нарушение.
            make_event(
                2,
                "Anna",
                Action::Move {
                    destination: "Лес".to_string(),
                },
                6,
            ),
            // event 3: Иван (жив, свободен) говорит → без нарушений.
            make_event(3, "Ivan", Action::Speak { topic: None }, 7),
            // event 4: Пётр (мёртв) снова умирает → нарушение dead_cannot_die_again.
            make_event(4, "Petr", Action::Die, 8),
        ];

        let violations = engine.check_all(&state, &events);

        let ids: Vec<u64> = violations.iter().map(|v| v.event_id).collect();
        assert!(ids.contains(&1), "event 1 (Petr speaks) должен быть нарушен");
        assert!(ids.contains(&2), "event 2 (Anna moves) должен быть нарушен");
        assert!(ids.contains(&4), "event 4 (Petr dies again) должен быть нарушен");
        assert!(
            !ids.contains(&3),
            "event 3 (Ivan speaks) не должен быть нарушен"
        );
        // Минимум 3 нарушения (по одному на проблемное событие).
        assert!(
            violations.len() >= 3,
            "ожидается >= 3 нарушений, получено {}: {:?}",
            violations.len(),
            violations
                .iter()
                .map(|v| (v.event_id, v.constraint_name.as_str()))
                .collect::<Vec<_>>()
        );

        // Конкретно: event 4 (Petr умирает) нарушает dead_cannot_die_again.
        let event4_violations: Vec<&ConstraintViolation> = violations
            .iter()
            .filter(|v| v.event_id == 4)
            .collect();
        assert!(
            event4_violations
                .iter()
                .any(|v| v.constraint_name == "dead_cannot_die_again"),
            "event 4 должен нарушить dead_cannot_die_again"
        );
    }

    #[test]
    fn test_default_literary_has_expected_constraints() {
        let engine = ConstraintEngine::default_literary();
        assert!(
            engine.len() >= 9,
            "ожидается >= 9 ограничений в default_literary, получено {}",
            engine.len()
        );
        assert!(!engine.is_empty());

        // Проверим, что ключевые ограничения присутствуют по имени.
        let names: Vec<&str> = engine.constraints.iter().map(|c| c.name).collect();
        for required in [
            "dead_cannot_speak",
            "dead_cannot_move",
            "dead_cannot_act_physically_hit",
            "dead_cannot_act_physically_kill",
            "dead_cannot_act_physically_wound",
            "dead_cannot_act_physically_capture",
            "dead_cannot_act_physically_imprison",
            "dead_cannot_act_physically_free",
            "dead_cannot_act_physically_heal",
            "dead_cannot_act_physically_touch",
            "imprisoned_cannot_move",
            "imprisoned_cannot_speak_freely",
            "captured_cannot_betray",
            "dead_cannot_die_again",
            "dead_cannot_marry",
            "dead_cannot_know_new_facts",
        ] {
            assert!(
                names.contains(&required),
                "ограничение '{}' отсутствует в default_literary: {:?}",
                required,
                names
            );
        }

        // Пустой движок — отдельно.
        let empty = ConstraintEngine::new();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }
}
