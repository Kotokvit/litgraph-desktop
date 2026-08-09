//! Высокоуровневый детектор противоречий для Reasoning Engine.
//!
//! Этот модуль — верхний слой рассуждений: он агрегирует три вида
//! противоречий в единый отчёт [`ContradictionReport`]:
//!
//! 1. `ConstraintViolation` — нарушение ограничений (из `constraints.rs`).
//! 2. [`TemporalParadox`] — временной парадокс: персонаж мёртв, но совершает
//!    действие, требующее жизни (говорит, двигается, убивает и т.д.), либо
//!    воскресает, не будучи мёртвым.
//! 3. `CausalLoop` — причинная петля (A → B → C → A), из `causality.rs`.
//!
//! # Алгоритм обнаружения временного парадокса
//!
//! Для каждого события с действием, требующим жизни (всё, кроме `Die`,
//! `Resurrect`, `Custom` с нейтральной полярностью), ищем активный на момент
//! события факт `entity.alive = Bool(false)`. Если такой факт найден —
//! событие невозможно (мёртвые не говорят), эммитим парадокс.
//!
//! Дополнительно: `Action::Resurrect` для актёра, который не был мёртв
//! (`alive = true` или факта `alive` нет), — тоже парадокс «воскрес без
//! смерти».
//!
//! # Временная семантика
//!
//! Факт считается «активным в момент T», если
//! `valid_from <= T` И (`valid_until is None` ИЛИ `T < valid_until`).
//! Это стандартная семантика полуоткрытого интервала `[valid_from, valid_until)`.
//!
//! См. SPEC §2.10 для типа `ContradictionReport` и сопутствующих структур.

use serde::{Deserialize, Serialize};

use crate::reasoning::facts::{
    Action, Event, EventId, FactId, FactLog, FactValue, VerbPolarity,
};
use crate::reasoning::timeline::TemporalAnchor;

// ════════════════════════════════════════════════════════════════════════
//  Sibling-module types (constraints.rs, causality.rs) — real imports
// ════════════════════════════════════════════════════════════════════════
//
// Wave 2 sibling modules have landed. Re-export their types so callers can
// pull ConstraintViolation/CausalLoop from either side (the defining module
// or here, for convenience).

pub use crate::reasoning::constraints::ConstraintViolation;
pub use crate::reasoning::causality::CausalLoop;

// ════════════════════════════════════════════════════════════════════════
//  Собственные типы модуля (SPEC §2.10)
// ════════════════════════════════════════════════════════════════════════

/// Временной парадокс: персонаж мёртв, но совершает действие, требующее
/// жизни (говорит, двигается, убивает и т.д.), либо воскресает, не будучи
/// мёртвым. SPEC §2.10.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalParadox {
    /// Человекочитаемое описание на русском.
    pub description: String,
    /// Факт, утверждающий несовместимое состояние (напр. «alive = false»).
    /// Для парадокса «воскрес без смерти» без явного факта — 0 (sentinel).
    pub earlier_fact: FactId,
    /// Событие, нарушающее факт (напр. `Speak` в более поздней главе).
    pub later_event: EventId,
    /// Время, когда факт начал действовать.
    pub earlier_at: TemporalAnchor,
    /// Время, когда произошло событие-нарушитель.
    pub later_at: TemporalAnchor,
}

/// Отчёт о противоречиях в нарративе. SPEC §2.10.
///
/// Агрегирует три вида противоречий. [`Self::is_empty`] и
/// [`Self::total_count`] дают быстрые сводки; [`Self::summary`] возвращает
/// человекочитаемое описание на русском.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContradictionReport {
    /// Нарушения ограничений (из `ConstraintEngine`).
    pub violations: Vec<ConstraintViolation>,
    /// Временные парадоксы (из [`ContradictionDetector::detect_temporal_paradoxes`]).
    pub temporal_paradoxes: Vec<TemporalParadox>,
    /// Причинные петли (из `causality.rs`).
    pub causal_loops: Vec<CausalLoop>,
}

impl ContradictionReport {
    /// Пустой отчёт.
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` если ни одного противоречия не найдено.
    pub fn is_empty(&self) -> bool {
        self.violations.is_empty()
            && self.temporal_paradoxes.is_empty()
            && self.causal_loops.is_empty()
    }

    /// Суммарное количество противоречий всех трёх видов.
    pub fn total_count(&self) -> usize {
        self.violations.len() + self.temporal_paradoxes.len() + self.causal_loops.len()
    }

    /// Человекочитаемая сводка на русском.
    ///
    /// Примеры:
    /// - `"Противоречий не найдено"` (пусто)
    /// - `"Найдено 3 противоречия: 2 нарушения ограничений, 1 временной парадокс"`
    /// - `"Найдено 5 противоречий: 3 нарушения ограничений, 1 временной парадокс, 1 причинная петля"`
    pub fn summary(&self) -> String {
        if self.is_empty() {
            return "Противоречий не найдено".to_string();
        }
        let total = self.total_count();
        let v = self.violations.len();
        let p = self.temporal_paradoxes.len();
        let c = self.causal_loops.len();

        let contradictions_word = pluralize_ru(total, "противоречие", "противоречия", "противоречий");

        let mut parts: Vec<String> = Vec::new();
        if v > 0 {
            let w = pluralize_ru(
                v,
                "нарушение ограничений",
                "нарушения ограничений",
                "нарушений ограничений",
            );
            parts.push(format!("{} {}", v, w));
        }
        if p > 0 {
            let w = pluralize_ru(
                p,
                "временной парадокс",
                "временных парадокса",
                "временных парадоксов",
            );
            parts.push(format!("{} {}", p, w));
        }
        if c > 0 {
            let w = pluralize_ru(
                c,
                "причинная петля",
                "причинные петли",
                "причинных петель",
            );
            parts.push(format!("{} {}", c, w));
        }

        format!(
            "Найдено {} {}: {}",
            total,
            contradictions_word,
            parts.join(", ")
        )
    }
}

// ════════════════════════════════════════════════════════════════════════
//  ContradictionDetector
// ════════════════════════════════════════════════════════════════════════

/// Детектор противоречий: ищет временные парадоксы и агрегирует все виды.
///
/// Stateless — все методы принимают ссылки на данные. Можно переиспользовать
/// между вызовами (поэтому реализует `Clone`/`Default`).
#[derive(Debug, Clone, Default)]
pub struct ContradictionDetector;

impl ContradictionDetector {
    /// Создать новый детектор.
    pub fn new() -> Self {
        Self
    }

    /// Поиск временных парадоксов: мёртвый персонаж действует ИЛИ
    /// живой персонаж воскресает.
    ///
    /// Алгоритм:
    /// 1. Для каждого события с действием, требующим жизни, ищем активный
    ///    на момент события факт `entity.alive = Bool(false)`. Если найден —
    ///    парадокс.
    /// 2. Для `Action::Resurrect`: проверяем, что актёр действительно был
    ///    мёртв до события. Если `alive = true` или факта нет — парадокс
    ///    «воскрес без смерти».
    pub fn detect_temporal_paradoxes(
        &self,
        facts: &FactLog,
        events: &[Event],
    ) -> Vec<TemporalParadox> {
        let mut paradoxes = Vec::new();

        for event in events {
            // Resurrect — особый случай (обрабатывается отдельно).
            if matches!(event.action, Action::Resurrect) {
                if let Some(paradox) = Self::check_resurrect_without_dying(facts, event) {
                    paradoxes.push(paradox);
                }
                // Resurrect сам по себе не «действие, требующее жизни» —
                // переходим к следующему событию.
                continue;
            }

            if !action_requires_life(&event.action) {
                continue;
            }

            // Ищем активный на момент event.time факт «alive = false».
            let death_fact = facts.all_facts().iter().find(|f| {
                f.entity == event.actor
                    && f.attribute == "alive"
                    && f.valid_from <= event.time
                    && match &f.valid_until {
                        None => true,
                        Some(until) => &event.time < until,
                    }
                    && matches!(f.value, FactValue::Bool(false))
            });

            if let Some(fact) = death_fact {
                let description = format!(
                    "{} мёртв с {}, но совершает действие {:?} в {}",
                    fact.entity,
                    fact.valid_from.display_chapter(),
                    event.action,
                    event.time.display_chapter()
                );
                paradoxes.push(TemporalParadox {
                    description,
                    earlier_fact: fact.id,
                    later_event: event.id,
                    earlier_at: fact.valid_from.clone(),
                    later_at: event.time.clone(),
                });
            }
        }

        paradoxes
    }

    /// Проверка парадокса «воскрес без смерти».
    ///
    /// Возвращает `Some(paradox)` если актёр не был мёртв до `Resurrect`
    /// (т.е. `alive = true` ИЛИ факта `alive` вообще нет на момент события).
    /// Если последний активный факт `alive = Bool(false)` — парадокса нет
    /// (воскрешение было оправдано).
    fn check_resurrect_without_dying(
        facts: &FactLog,
        event: &Event,
    ) -> Option<TemporalParadox> {
        // Последний активный на момент event.time факт `alive` для актёра.
        let alive_fact = facts
            .all_facts()
            .iter()
            .filter(|f| {
                f.entity == event.actor
                    && f.attribute == "alive"
                    && f.valid_from <= event.time
                    && match &f.valid_until {
                        None => true,
                        Some(until) => &event.time < until,
                    }
            })
            .max_by(|a, b| a.valid_from.cmp(&b.valid_from));

        let was_dead = alive_fact
            .map(|f| matches!(f.value, FactValue::Bool(false)))
            .unwrap_or(false);

        if was_dead {
            return None;
        }

        // Не был мёртв — парадокс.
        let description = format!(
            "{} воскресает в {}, но не был мёртв до этого",
            event.actor,
            event.time.display_chapter()
        );

        // Если есть факт `alive = true`, указываем на него (это факт, с
        // которым конфликтует воскресение). Иначе — sentinel (FactId=0,
        // earlier_at=event.time) — символизирует «явного факта не было, но
        // и смерти тоже не было».
        let (earlier_fact, earlier_at) = match alive_fact {
            Some(f) => (f.id, f.valid_from.clone()),
            None => (0, event.time.clone()),
        };

        Some(TemporalParadox {
            description,
            earlier_fact,
            later_event: event.id,
            earlier_at,
            later_at: event.time.clone(),
        })
    }

    /// Собрать полный отчёт из всех источников противоречий.
    ///
    /// `constraint_violations` и `causal_loops` передаются готовыми
    /// (их производят sibling-модули `constraints.rs` и `causality.rs`).
    /// Временные парадоксы детектор ищет сам по `facts` и `events`.
    pub fn detect_all(
        &self,
        constraint_violations: Vec<ConstraintViolation>,
        facts: &FactLog,
        events: &[Event],
        causal_loops: Vec<CausalLoop>,
    ) -> ContradictionReport {
        let temporal_paradoxes = self.detect_temporal_paradoxes(facts, events);
        ContradictionReport {
            violations: constraint_violations,
            temporal_paradoxes,
            causal_loops,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Вспомогательные функции (private)
// ════════════════════════════════════════════════════════════════════════

/// `true` если действие требует живого актёра.
///
/// НЕ требуют жизни: `Die`, `Resurrect`, `Custom` с нейтральной полярностью.
/// Все остальные действия — требуют (по task brief 2-d §5: «any action that
/// requires being alive — i.e. NOT `Die`, `Resurrect`, `Custom` with neutral
/// polarity»).
fn action_requires_life(action: &Action) -> bool {
    match action {
        Action::Die => false,
        Action::Resurrect => false,
        Action::Custom { polarity, .. } => !matches!(polarity, VerbPolarity::Neutral),
        // Все остальные варианты Action требуют живого актёра.
        _ => true,
    }
}

/// Русское склонение существительного по числу.
///
/// Возвращает одну из трёх форм в зависимости от `n`:
/// - `one`  — для 1, 21, 31, ... (но не 11, 111, ...)
/// - `few`  — для 2–4, 22–24, ... (но не 12–14)
/// - `many` — для 0, 5–20, 25–30, ...
fn pluralize_ru(n: usize, one: &'static str, few: &'static str, many: &'static str) -> &'static str {
    let mod10 = n % 10;
    let mod100 = n % 100;
    if mod10 == 1 && mod100 != 11 {
        one
    } else if (2..=4).contains(&mod10) && !(12..=14).contains(&mod100) {
        few
    } else {
        many
    }
}

// ════════════════════════════════════════════════════════════════════════
//  Юнит-тесты
// ════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::facts::{Action, Event, Fact, FactLog, FactValue, Provenance};
    use crate::reasoning::timeline::TemporalAnchor;

    /// Хелпер: `TemporalAnchor` для главы (без суффикса/сцены/offset).
    fn anchor(chapter: u32) -> TemporalAnchor {
        TemporalAnchor::new(chapter)
    }

    /// Хелпер: минимальное событие для тестов.
    fn dummy_event(actor: &str, action: Action, time: TemporalAnchor) -> Event {
        Event {
            id: 0,
            actor: actor.to_string(),
            action,
            target: None,
            instrument: None,
            time,
            source_text: String::new(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        }
    }

    /// Хелпер: минимальный факт для тестов.
    fn dummy_fact(entity: &str, attr: &str, value: FactValue, time: TemporalAnchor) -> Fact {
        Fact {
            id: 0,
            entity: entity.to_string(),
            attribute: attr.to_string(),
            value,
            derived_from: Vec::new(),
            valid_from: time,
            valid_until: None,
            provenance: Provenance::SvoParser,
        }
    }

    /// Хелпер: минимальное `ConstraintViolation` для тестов.
    fn dummy_violation(event_id: EventId, actor: &str, at: TemporalAnchor) -> ConstraintViolation {
        ConstraintViolation {
            constraint_name: "no_action_when_dead".to_string(),
            event_id,
            actor: actor.to_string(),
            attempted_action: Action::Speak { topic: None },
            reason: "Персонаж мёртв".to_string(),
            conflicting_fact: None,
            at,
        }
    }

    /// Хелпер: минимальный `CausalLoop` для тестов.
    fn dummy_causal_loop(chain: Vec<EventId>) -> CausalLoop {
        CausalLoop {
            description: "Причинная петля".to_string(),
            chain,
        }
    }

    #[test]
    fn test_detect_peter_dead_in_ch12_speaks_in_ch15() {
        let mut log = FactLog::new();
        // Пётр мёртв с главы 12.
        log.assert_fact(dummy_fact(
            "peter",
            "alive",
            FactValue::Bool(false),
            anchor(12),
        ));
        // Пётр говорит в главе 15.
        log.record_event(dummy_event(
            "peter",
            Action::Speak { topic: None },
            anchor(15),
        ));

        let detector = ContradictionDetector::new();
        let paradoxes = detector.detect_temporal_paradoxes(&log, log.all_events());
        assert_eq!(paradoxes.len(), 1, "мёртвый говорящий — парадокс");

        let p = &paradoxes[0];
        assert!(
            p.description.contains("peter"),
            "описание должно упоминать актёра: {}",
            p.description
        );
        assert!(
            p.description.contains("мёртв"),
            "описание должно говорить о смерти: {}",
            p.description
        );
        assert!(
            p.description.contains("Глава 12"),
            "описание должно упоминать главу смерти: {}",
            p.description
        );
        assert!(
            p.description.contains("Speak"),
            "описание должно упоминать действие: {}",
            p.description
        );
        assert!(
            p.description.contains("Глава 15"),
            "описание должно упоминать главу события: {}",
            p.description
        );

        // Структурированные поля.
        assert_eq!(p.earlier_fact, 1, "earlier_fact должен указывать на факт о смерти");
        assert_eq!(p.later_event, 1, "later_event должен указывать на событие Speak");
        assert_eq!(p.earlier_at, anchor(12));
        assert_eq!(p.later_at, anchor(15));
    }

    #[test]
    fn test_no_paradox_for_alive_character_speaking() {
        let mut log = FactLog::new();
        // Пётр жив с главы 1.
        log.assert_fact(dummy_fact(
            "peter",
            "alive",
            FactValue::Bool(true),
            anchor(1),
        ));
        // Пётр говорит в главе 5.
        log.record_event(dummy_event(
            "peter",
            Action::Speak { topic: None },
            anchor(5),
        ));

        let detector = ContradictionDetector::new();
        let paradoxes = detector.detect_temporal_paradoxes(&log, log.all_events());
        assert_eq!(paradoxes.len(), 0, "живой говорящий — НЕ парадокс");
    }

    #[test]
    fn test_detect_resurrect_without_dying() {
        let detector = ContradictionDetector::new();

        // Случай 1: Пётр жив, но воскресает — парадокс.
        let mut log = FactLog::new();
        log.assert_fact(dummy_fact(
            "peter",
            "alive",
            FactValue::Bool(true),
            anchor(1),
        ));
        log.record_event(dummy_event("peter", Action::Resurrect, anchor(5)));

        let paradoxes = detector.detect_temporal_paradoxes(&log, log.all_events());
        assert_eq!(paradoxes.len(), 1, "воскресший без смерти — парадокс");

        let p = &paradoxes[0];
        assert!(p.description.contains("воскресает"), "описание: «воскресает»: {}", p.description);
        assert!(p.description.contains("Глава 5"), "описание: глава 5: {}", p.description);
        assert!(
            p.description.contains("не был мёртв"),
            "описание: «не был мёртв»: {}",
            p.description
        );
        // earlier_fact указывает на факт «alive = true» (тот, с которым
        // конфликтует воскресение).
        assert_eq!(p.earlier_fact, 1);
        assert_eq!(p.earlier_at, anchor(1));
        assert_eq!(p.later_at, anchor(5));

        // Случай 2: Пётр действительно мёртв, воскресает — НЕ парадокс.
        let mut log2 = FactLog::new();
        log2.assert_fact(dummy_fact(
            "peter",
            "alive",
            FactValue::Bool(false),
            anchor(1),
        ));
        log2.record_event(dummy_event("peter", Action::Resurrect, anchor(5)));

        let paradoxes2 = detector.detect_temporal_paradoxes(&log2, log2.all_events());
        assert_eq!(paradoxes2.len(), 0, "мёртвый, воскресающий — НЕ парадокс");

        // Случай 3: Нет факта alive вообще, но актёр воскресает — парадокс
        // (не было смерти, значит и воскресать не из чего).
        let mut log3 = FactLog::new();
        log3.record_event(dummy_event("peter", Action::Resurrect, anchor(5)));

        let paradoxes3 = detector.detect_temporal_paradoxes(&log3, log3.all_events());
        assert_eq!(
            paradoxes3.len(),
            1,
            "воскресший без явного факта alive — парадокс"
        );
        // earlier_fact = 0 (sentinel для «нет факта»).
        assert_eq!(paradoxes3[0].earlier_fact, 0);
    }

    #[test]
    fn test_contradiction_report_summary() {
        let mut report = ContradictionReport::new();
        assert!(report.is_empty());
        assert_eq!(report.total_count(), 0);
        assert_eq!(report.summary(), "Противоречий не найдено");

        report.violations.push(dummy_violation(1, "peter", anchor(5)));
        report.violations.push(dummy_violation(2, "peter", anchor(7)));
        report.temporal_paradoxes.push(TemporalParadox {
            description: "peter мёртв с Глава 5, но говорит в Глава 7".to_string(),
            earlier_fact: 1,
            later_event: 2,
            earlier_at: anchor(5),
            later_at: anchor(7),
        });

        assert!(!report.is_empty());
        assert_eq!(report.total_count(), 3);

        let s = report.summary();
        assert!(
            s.contains("3 противоречия"),
            "summary должен содержать «3 противоречия»: {}",
            s
        );
        assert!(
            s.contains("2 нарушения ограничений"),
            "summary должен содержать «2 нарушения ограничений»: {}",
            s
        );
        assert!(
            s.contains("1 временной парадокс"),
            "summary должен содержать «1 временной парадокс»: {}",
            s
        );
    }

    #[test]
    fn test_contradiction_report_is_empty() {
        // Пустой отчёт.
        let empty = ContradictionReport::new();
        assert!(empty.is_empty());
        assert_eq!(empty.total_count(), 0);

        // Default-трейт = пустой отчёт.
        let default = ContradictionReport::default();
        assert!(default.is_empty());

        // Только causal_loops — не пустой.
        let mut with_loop = ContradictionReport::new();
        with_loop.causal_loops.push(dummy_causal_loop(vec![1, 2, 3, 1]));
        assert!(!with_loop.is_empty());
        assert_eq!(with_loop.total_count(), 1);
    }

    #[test]
    fn test_detect_all_combines_violations_and_paradoxes() {
        let mut log = FactLog::new();
        // Пётр мёртв с главы 12.
        log.assert_fact(dummy_fact(
            "peter",
            "alive",
            FactValue::Bool(false),
            anchor(12),
        ));
        // Пётр говорит в главе 15.
        log.record_event(dummy_event(
            "peter",
            Action::Speak { topic: None },
            anchor(15),
        ));

        let violations = vec![dummy_violation(1, "peter", anchor(15))];
        let causal_loops = vec![dummy_causal_loop(vec![1, 2, 3, 1])];

        let detector = ContradictionDetector::new();
        let report = detector.detect_all(violations, &log, log.all_events(), causal_loops);

        // Все три категории заполнены.
        assert_eq!(report.violations.len(), 1, "одна violation");
        assert_eq!(report.temporal_paradoxes.len(), 1, "один temporal_paradox");
        assert_eq!(report.causal_loops.len(), 1, "один causal_loop");
        assert_eq!(report.total_count(), 3);
        assert!(!report.is_empty());

        // Сводка упоминает все три категории.
        let s = report.summary();
        assert!(s.contains("3 противоречия"), "summary: {}", s);
        assert!(s.contains("1 нарушение ограничений"), "summary: {}", s);
        assert!(s.contains("1 временной парадокс"), "summary: {}", s);
        assert!(s.contains("1 причинная петля"), "summary: {}", s);
    }
}
