//! # Hypotheses — генерация и верификация гипотез
//!
//! Когда reasoning engine находит противоречие (constraint violation или
//! временной парадокс), у него есть три стратегии разрешения:
//!
//! 1. **Алгоритмическая реструктуризация нарратива.** Объявить событие
//!    воспоминанием/сном/видением — тогда оно не противоречит WorldState,
//!    потому что происходит «в голове» персонажа, а не в реальности.
//! 2. **Поиск пропущенного события.** Если персонаж действует после своей
//!    смерти — возможно, между смертью и этим действием было воскрешение,
//!    которое парсер не заметил.
//! 3. **Передача пользователю.** Если ни алгоритм, ни LLM не могут
//!    разрешить противоречие — это ошибка в тексте, и человек должен
//!    решить: переписать, удалить или принять как художественный приём.
//!
//! Этот модуль реализует **чисто алгоритмическую** генерацию гипотез (LLM
//! подключается позже через `llm_bridge.rs`) и их верификацию против
//! текущего `WorldState` и `FactLog`.
//!
//! ## Принципы
//!
//! - **Determinism first.** Генератор детерминированно предлагает 3 гипотезы
//!   на каждое противоречие (flashback / dream / text-error для violations;
//!   resurrect / flashback / dream для paradoxes). Никакого LLM.
//! - **State is truth.** Верификатор сверяет гипотезу с `WorldState` и
//!   `FactLog`, а не с текстом нарратива.
//! - **Russian-first UI strings** — формулировки гипотез на русском;
//!   идентификаторы и имена типов — английские.
//!
//! ## Связь с SPEC
//!
//! См. `docs/reasoning/SPEC.md` §2.11 для формального контракта типов
//! `Hypothesis`, `Resolution`, `EventKind`, `HypothesisSource`,
//! `HypothesisStatus`.

use serde::{Deserialize, Serialize};

use crate::reasoning::constraints::ConstraintViolation;
use crate::reasoning::contradictions::TemporalParadox;
use crate::reasoning::facts::{Action, EventId, FactId, FactLog, FactValue};
use crate::reasoning::state::WorldState;

/// Монотонно возрастающий идентификатор гипотезы внутри `HypothesisLog`.
pub type HypothesisId = u64;

/// Классификация события в нарративе. SPEC §2.11.
///
/// Каноническое событие — реально происходящее в мире истории. Остальные
/// варианты — нарративные приёмы, выводящие событие за рамки «объективной
/// реальности» нарратива: воспоминание, сон, видение, рассказ в рассказе.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EventKind {
    /// Реальное событие в нарративе (по умолчанию).
    Canonical,
    /// Воспоминание персонажа о прошлом событии.
    Flashback,
    /// Сон / галлюцинация.
    Dream,
    /// Видение (пророческое, мистическое).
    Vision,
    /// Рассказ в рассказе (вложенная история).
    StoryWithinStory,
}

/// Предлагаемое разрешение противоречия. SPEC §2.11.
///
/// На текущей волне реализован единственный вариант: пометить событие
/// как `EventKind` (flashback/dream/vision/...). Это выводит его за
/// рамки конфликтов с `WorldState`, т.к. нарративные приёмы не требуют
/// физической консистентности.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Resolution {
    /// Пометить событие `event_id` как `kind`.
    MarkEventAs {
        /// ID события для переклассификации.
        event_id: EventId,
        /// Новый класс события.
        kind: EventKind,
    },
}

/// Происхождение гипотезы. SPEC §2.11.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HypothesisSource {
    /// Сгенерирована алгоритмом (этот модуль).
    Algorithm,
    /// Предложена LLM (через `llm_bridge.rs`).
    Llm,
    /// Введена пользователем вручную.
    User,
}

/// Текущий статус верификации гипотезы. SPEC §2.11.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum HypothesisStatus {
    /// Гипотеза ещё не проверена (или требует решения пользователя).
    Pending,
    /// Гипотеза проверена и принята — её Resolution можно применить.
    Accepted,
    /// Гипотеза отвергнута с указанием причины.
    Rejected(String),
}

/// Гипотеза о разрешении противоречия. SPEC §2.11.
///
/// Каждая гипотеза — это формулировка («Пётр каким-то образом выжил после
/// Г12») + опциональное предлагаемое разрешение + списки фактов за и против.
/// Верификатор решает, принять гипотезу или отвергнуть, на основе
/// `WorldState` и `FactLog`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    /// ID, назначаемый `HypothesisLog::add`. Если 0 на входе — авто-присвоение.
    pub id: HypothesisId,
    /// Человекочитаемая формулировка на русском.
    pub statement: String,
    /// Опциональное машинное разрешение (если None — требует человеческого
    /// решения или нового события в нарративе).
    pub proposed_resolution: Option<Resolution>,
    /// ID фактов, поддерживающих гипотезу (например, факт смерти для
    /// гипотезы «воскрес»).
    pub evidence_for: Vec<FactId>,
    /// ID фактов, противоречащих гипотезе (например, факт `alive=true`
    /// для гипотезы «персонаж мёртв»).
    pub evidence_against: Vec<FactId>,
    /// Текущий статус верификации.
    pub status: HypothesisStatus,
    /// Откуда пришла гипотеза.
    pub source: HypothesisSource,
}

// ============================================================================
// HypothesisGenerator
// ============================================================================

/// Генератор гипотез. Stateless — все методы принимают внешние данные.
/// Для каждого противоречия детерминированно предлагает 3 гипотезы.
#[derive(Debug, Clone, Default)]
pub struct HypothesisGenerator;

impl HypothesisGenerator {
    /// Создать новый генератор.
    pub fn new() -> Self {
        Self
    }

    /// Сгенерировать гипотезы для нарушения ограничения.
    ///
    /// Типичный сценарий: мёртвый персонаж пытается говорить (`dead_cannot_speak`).
    /// Алгоритм предлагает 3 гипотезы:
    ///
    /// 1. **Flashback** — событие является воспоминанием. Resolution:
    ///    `MarkEventAs { event_id, Flashback }`.
    /// 2. **Dream** — событие является сном. Resolution:
    ///    `MarkEventAs { event_id, Dream }`.
    /// 3. **Text error** — ошибка в тексте, событие нужно удалить или
    ///    переписать. Без Resolution (требует решения пользователя).
    ///
    /// `evidence_for` заполняется ID факта `alive=false` для актёра
    /// нарушения (если такой факт есть в `FactLog`).
    pub fn generate_for_violation(
        &self,
        violation: &ConstraintViolation,
        facts: &FactLog,
    ) -> Vec<Hypothesis> {
        // Ищем факт смерти для актёра (alive=false) — это «доказательство»
        // того, что нарушение не выдумка, а реальный конфликт состояний.
        let death_fact_id = facts
            .all_facts()
            .iter()
            .find(|f| {
                f.entity == violation.actor
                    && f.attribute == "alive"
                    && matches!(f.value, FactValue::Bool(false))
            })
            .map(|f| f.id);

        let evidence_for: Vec<FactId> = death_fact_id.into_iter().collect();
        let event_id = violation.event_id;

        vec![
            // H1: Flashback.
            Hypothesis {
                id: 0,
                statement: format!(
                    "Событие {} является воспоминанием/flashback'ом",
                    event_id
                ),
                proposed_resolution: Some(Resolution::MarkEventAs {
                    event_id,
                    kind: EventKind::Flashback,
                }),
                evidence_for: evidence_for.clone(),
                evidence_against: Vec::new(),
                status: HypothesisStatus::Pending,
                source: HypothesisSource::Algorithm,
            },
            // H2: Dream.
            Hypothesis {
                id: 0,
                statement: format!("Событие {} является сном", event_id),
                proposed_resolution: Some(Resolution::MarkEventAs {
                    event_id,
                    kind: EventKind::Dream,
                }),
                evidence_for: evidence_for.clone(),
                evidence_against: Vec::new(),
                status: HypothesisStatus::Pending,
                source: HypothesisSource::Algorithm,
            },
            // H3: Text error (без Resolution — требует решения пользователя).
            Hypothesis {
                id: 0,
                statement: format!(
                    "Ошибка в тексте — событие {} нужно удалить или переписать",
                    event_id
                ),
                proposed_resolution: None,
                evidence_for: evidence_for.clone(),
                evidence_against: Vec::new(),
                status: HypothesisStatus::Pending,
                source: HypothesisSource::Algorithm,
            },
        ]
    }

    /// Сгенерировать гипотезы для временного парадокса.
    ///
    /// Типичный сценарий: Пётр умер в главе 12, но действует в главе 15.
    /// Алгоритм предлагает 3 гипотезы:
    ///
    /// 1. **Resurrect** — персонаж воскрес между смертью и более поздним
    ///    событием. Без Resolution (требует нового события `Resurrect` в
    ///    нарративе).
    /// 2. **Flashback** — более позднее событие является воспоминанием.
    ///    Resolution: `MarkEventAs { later_event, Flashback }`.
    /// 3. **Dream** — более позднее событие является сном. Resolution:
    ///    `MarkEventAs { later_event, Dream }`.
    ///
    /// `evidence_for` заполняется ID факта смерти (`paradox.earlier_fact`),
    /// `evidence_against` — ID фактов `alive=true` для той же сущности
    /// (если такие есть в `FactLog`).
    pub fn generate_for_paradox(
        &self,
        paradox: &TemporalParadox,
        facts: &FactLog,
    ) -> Vec<Hypothesis> {
        // Ищем факт смерти по ID, чтобы получить имя сущности.
        let death_fact = facts.all_facts().iter().find(|f| f.id == paradox.earlier_fact);
        let entity = death_fact
            .map(|f| f.entity.clone())
            .unwrap_or_else(|| "Персонаж".to_string());

        let death_chapter = paradox.earlier_at.display_chapter();
        let later_chapter = paradox.later_at.display_chapter();
        let later_event_id = paradox.later_event;

        // evidence_for: факт смерти (если ID не sentinel 0).
        let evidence_for: Vec<FactId> = if paradox.earlier_fact != 0 {
            vec![paradox.earlier_fact]
        } else {
            Vec::new()
        };

        // evidence_against: любые факты alive=true для той же сущности
        // (они противоречат гипотезе «персонаж мёртв / воскрес»).
        let evidence_against: Vec<FactId> = facts
            .all_facts()
            .iter()
            .filter(|f| {
                f.entity == entity
                    && f.attribute == "alive"
                    && matches!(f.value, FactValue::Bool(true))
            })
            .map(|f| f.id)
            .collect();

        vec![
            // H1: Resurrect (без Resolution — требует нового события).
            Hypothesis {
                id: 0,
                statement: format!(
                    "Персонаж {} воскрес между {} и {}",
                    entity, death_chapter, later_chapter
                ),
                proposed_resolution: None,
                evidence_for: evidence_for.clone(),
                evidence_against: evidence_against.clone(),
                status: HypothesisStatus::Pending,
                source: HypothesisSource::Algorithm,
            },
            // H2: Flashback для более позднего события.
            Hypothesis {
                id: 0,
                statement: format!("Событие в {} — flashback", later_chapter),
                proposed_resolution: Some(Resolution::MarkEventAs {
                    event_id: later_event_id,
                    kind: EventKind::Flashback,
                }),
                evidence_for: evidence_for.clone(),
                evidence_against: evidence_against.clone(),
                status: HypothesisStatus::Pending,
                source: HypothesisSource::Algorithm,
            },
            // H3: Dream для более позднего события.
            Hypothesis {
                id: 0,
                statement: format!("Событие в {} — сон", later_chapter),
                proposed_resolution: Some(Resolution::MarkEventAs {
                    event_id: later_event_id,
                    kind: EventKind::Dream,
                }),
                evidence_for: evidence_for.clone(),
                evidence_against: evidence_against.clone(),
                status: HypothesisStatus::Pending,
                source: HypothesisSource::Algorithm,
            },
        ]
    }
}

// ============================================================================
// HypothesisVerifier
// ============================================================================

/// Верификатор гипотез. Stateless — сверяет каждую гипотезу с текущим
/// `WorldState` и `FactLog`, возвращая новый `HypothesisStatus`.
///
/// ## Правила верификации
///
/// - `Resolution::MarkEventAs` с `Flashback` / `Dream` / `Vision` /
///   `StoryWithinStory` → **Accepted**. Нарративные приёмы не конфликтуют
///   с `WorldState` — они выводят событие за рамки объективной реальности.
/// - `Resolution::MarkEventAs` с `Canonical` → **Rejected**. Канонический
///   статус подтверждает, что событие реально, и не разрешает противоречие.
/// - Гипотеза «воскрес» (без Resolution, statement содержит «воскрес») →
///   проверяем наличие события `Action::Resurrect` для этой сущности после
///   момента смерти. Если есть → **Accepted**; если нет → **Rejected**.
/// - Гипотеза «ошибка в тексте» (без Resolution, statement содержит
///   «Ошибка в тексте») → **Pending** (требует решения пользователя).
#[derive(Debug, Clone, Default)]
pub struct HypothesisVerifier;

impl HypothesisVerifier {
    /// Создать новый верификатор.
    pub fn new() -> Self {
        Self
    }

    /// Проверить гипотезу против текущего мира.
    pub fn verify(
        &self,
        hyp: &Hypothesis,
        world: &WorldState,
        facts: &FactLog,
    ) -> HypothesisStatus {
        match &hyp.proposed_resolution {
            Some(Resolution::MarkEventAs { event_id: _, kind }) => match kind {
                EventKind::Flashback
                | EventKind::Dream
                | EventKind::Vision
                | EventKind::StoryWithinStory => HypothesisStatus::Accepted,
                EventKind::Canonical => HypothesisStatus::Rejected(
                    "Канонический статус не разрешает противоречие — событие остаётся реальным"
                        .to_string(),
                ),
            },
            None => {
                // Различаем «воскрес» и «ошибку в тексте» по формулировке.
                if hyp.statement.contains("воскрес") {
                    self.verify_resurrect(hyp, world, facts)
                } else {
                    // «Ошибка в тексте» или иная гипотеза без Resolution —
                    // оставляем Pending для решения пользователя.
                    HypothesisStatus::Pending
                }
            }
        }
    }

    /// Проверка гипотезы «персонаж воскрес»: ищем событие `Resurrect` для
    /// сущности после момента смерти.
    ///
    /// Алгоритм:
    /// 1. Из `evidence_for` находим факт `alive=false` — это факт смерти.
    /// 2. Сверяем с `WorldState`: текущее значение `alive` должно быть
    ///    `Bool(false)` (если нет — воскрешение не требуется).
    /// 3. Ищем в `FactLog` событие `Action::Resurrect` для этой сущности
    ///    с `time > death.valid_from`. Если есть → **Accepted**.
    /// 4. Иначе → **Rejected** с причиной «Нет события воскрешения».
    fn verify_resurrect(
        &self,
        hyp: &Hypothesis,
        world: &WorldState,
        facts: &FactLog,
    ) -> HypothesisStatus {
        // Шаг 1: находим факт смерти в evidence_for.
        let death_fact = hyp
            .evidence_for
            .iter()
            .filter_map(|fid| facts.all_facts().iter().find(|f| f.id == *fid))
            .find(|f| f.attribute == "alive" && matches!(f.value, FactValue::Bool(false)));

        let Some(death) = death_fact else {
            return HypothesisStatus::Rejected(
                "Нет факта смерти в evidence_for — невозможно проверить воскрешение".to_string(),
            );
        };

        // Шаг 2: sanity-check с WorldState. Если персонаж сейчас не мёртв,
        // воскрешение не требуется (и гипотеза бессмысленна).
        if let Some(v) = world.get(&death.entity, "alive") {
            if !matches!(v, FactValue::Bool(false)) {
                return HypothesisStatus::Rejected(
                    "Сущность не мертва в текущем WorldState — воскрешение не требуется"
                        .to_string(),
                );
            }
        }

        // Шаг 3: ищем событие Resurrect для этой сущности после смерти.
        let has_resurrect = facts.all_events().iter().any(|e| {
            e.actor == death.entity
                && matches!(e.action, Action::Resurrect)
                && e.time.after(&death.valid_from)
        });

        if has_resurrect {
            HypothesisStatus::Accepted
        } else {
            HypothesisStatus::Rejected("Нет события воскрешения в нарративе".to_string())
        }
    }
}

// ============================================================================
// HypothesisLog
// ============================================================================

/// Журнал гипотез с монотонной нумерацией ID. Append-only.
///
/// Хранит все когда-либо добавленные гипотезы (включая отвергнутые — для
/// audit trail). ID назначаются автоматически при `add`, если `hyp.id == 0`.
#[derive(Debug, Clone, Default)]
pub struct HypothesisLog {
    hypotheses: Vec<Hypothesis>,
    next_id: HypothesisId,
}

impl HypothesisLog {
    /// Создать пустой журнал. Счётчик ID стартует с 1 (0 зарезервирован
    /// как «auto-assign me»).
    pub fn new() -> Self {
        Self {
            hypotheses: Vec::new(),
            next_id: 1,
        }
    }

    /// Добавить гипотезу. Если `hyp.id == 0` — присваивается следующий
    /// свободный ID. Возвращает ID, под которым гипотеза сохранена.
    pub fn add(&mut self, mut hyp: Hypothesis) -> HypothesisId {
        if hyp.id == 0 {
            hyp.id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
        }
        let id = hyp.id;
        self.hypotheses.push(hyp);
        id
    }

    /// Найти гипотезу по ID (immutable).
    pub fn get(&self, id: HypothesisId) -> Option<&Hypothesis> {
        self.hypotheses.iter().find(|h| h.id == id)
    }

    /// Найти гипотезу по ID (mutable) — для обновления статуса.
    pub fn get_mut(&mut self, id: HypothesisId) -> Option<&mut Hypothesis> {
        self.hypotheses.iter_mut().find(|h| h.id == id)
    }

    /// Все гипотезы в статусе `Pending`.
    pub fn pending(&self) -> Vec<&Hypothesis> {
        self.hypotheses
            .iter()
            .filter(|h| matches!(h.status, HypothesisStatus::Pending))
            .collect()
    }

    /// Все гипотезы в статусе `Accepted`.
    pub fn accepted(&self) -> Vec<&Hypothesis> {
        self.hypotheses
            .iter()
            .filter(|h| matches!(h.status, HypothesisStatus::Accepted))
            .collect()
    }

    /// Все гипотезы в статусе `Rejected`.
    pub fn rejected(&self) -> Vec<&Hypothesis> {
        self.hypotheses
            .iter()
            .filter(|h| matches!(h.status, HypothesisStatus::Rejected(_)))
            .collect()
    }

    /// Доступ ко всем гипотезам (в порядке добавления).
    pub fn all(&self) -> &[Hypothesis] {
        &self.hypotheses
    }
}

// ============================================================================
// Юнит-тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::facts::{Event, Fact, Provenance};
    use crate::reasoning::state::StateTransition;
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

    /// Хелпер: установить у сущности `alive` булево значение в `WorldState`.
    fn set_alive(state: &mut WorldState, entity: &str, alive: bool, at_chapter: u32) {
        let at = anchor(at_chapter);
        state.set(
            entity,
            "alive".to_string(),
            FactValue::Bool(alive),
            StateTransition {
                entity: entity.to_string(),
                attribute: "alive".to_string(),
                old_value: None,
                new_value: FactValue::Bool(alive),
                caused_by_event: None,
                at,
            },
        );
    }

    /// Хелпер: добавить факт `alive=false` для сущности в `FactLog`,
    /// начиная с главы `from_chapter`. Возвращает FactId.
    fn assert_dead_fact(log: &mut FactLog, entity: &str, from_chapter: u32) -> FactId {
        log.assert_fact(Fact {
            id: 0,
            entity: entity.to_string(),
            attribute: "alive".to_string(),
            value: FactValue::Bool(false),
            derived_from: Vec::new(),
            valid_from: anchor(from_chapter),
            valid_until: None,
            provenance: Provenance::Verified,
        })
    }

    /// Хелпер: построить `ConstraintViolation` для тестов (мёртвый персонаж
    /// пытается говорить в главе `at_chapter`).
    fn make_violation(event_id: EventId, actor: &str, at_chapter: u32) -> ConstraintViolation {
        ConstraintViolation {
            constraint_name: "dead_cannot_speak".to_string(),
            event_id,
            actor: actor.to_string(),
            attempted_action: crate::reasoning::facts::Action::Speak { topic: None },
            reason: "Невозможно: персонаж мёртв, но пытается говорить".to_string(),
            conflicting_fact: None,
            at: anchor(at_chapter),
        }
    }

    /// Хелпер: построить `TemporalParadox` для тестов.
    fn make_paradox(
        earlier_fact: FactId,
        later_event: EventId,
        earlier_chapter: u32,
        later_chapter: u32,
    ) -> TemporalParadox {
        TemporalParadox {
            description: format!("Пётр мёртв с Г{} но действует в Г{}", earlier_chapter, later_chapter),
            earlier_fact,
            later_event,
            earlier_at: anchor(earlier_chapter),
            later_at: anchor(later_chapter),
        }
    }

    #[test]
    fn test_generate_for_violation_proposes_flashback_and_dream() {
        let generator = HypothesisGenerator::new();
        let mut log = FactLog::new();
        // Пётр мёртв с главы 12.
        let _dead_id = assert_dead_fact(&mut log, "peter", 12);

        let violation = make_violation(7, "peter", 15);
        let hyps = generator.generate_for_violation(&violation, &log);

        // Должно быть 3 гипотезы.
        assert_eq!(hyps.len(), 3, "генератор должен предложить 3 гипотезы");

        // Все с id=0 (назначается HypothesisLog позже).
        assert!(hyps.iter().all(|h| h.id == 0), "все id должны быть 0 до добавления в log");

        // Все с статусом Pending и source=Algorithm.
        assert!(
            hyps.iter().all(|h| matches!(h.status, HypothesisStatus::Pending)),
            "все гипотезы должны быть Pending"
        );
        assert!(
            hyps.iter().all(|h| matches!(h.source, HypothesisSource::Algorithm)),
            "все гипотезы должны быть Algorithm"
        );

        // H1: flashback.
        let flashback = &hyps[0];
        assert!(
            flashback.statement.contains("воспоминанием"),
            "H1 должен упоминать воспоминание: {}",
            flashback.statement
        );
        match &flashback.proposed_resolution {
            Some(Resolution::MarkEventAs { event_id, kind }) => {
                assert_eq!(*event_id, 7, "H1 должен ссылаться на event_id=7");
                assert!(
                    matches!(kind, EventKind::Flashback),
                    "H1 должен помечать как Flashback"
                );
            }
            other => panic!("H1: ожидалось MarkEventAs Flashback, получено {:?}", other),
        }

        // H2: dream.
        let dream = &hyps[1];
        assert!(
            dream.statement.contains("сном"),
            "H2 должен упоминать сон: {}",
            dream.statement
        );
        match &dream.proposed_resolution {
            Some(Resolution::MarkEventAs { event_id, kind }) => {
                assert_eq!(*event_id, 7);
                assert!(matches!(kind, EventKind::Dream));
            }
            other => panic!("H2: ожидалось MarkEventAs Dream, получено {:?}", other),
        }

        // H3: text error, без Resolution.
        let text_err = &hyps[2];
        assert!(
            text_err.statement.contains("Ошибка в тексте"),
            "H3 должен быть об ошибке в тексте: {}",
            text_err.statement
        );
        assert!(
            text_err.proposed_resolution.is_none(),
            "H3 не должен иметь Resolution"
        );

        // evidence_for должен содержать ID факта смерти.
        assert!(
            !flashback.evidence_for.is_empty(),
            "evidence_for не должен быть пуст (есть факт смерти)"
        );
    }

    #[test]
    fn test_generate_for_paradox_proposes_resurrect() {
        let generator = HypothesisGenerator::new();
        let mut log = FactLog::new();
        let dead_id = assert_dead_fact(&mut log, "peter", 12);

        let paradox = make_paradox(dead_id, 9, 12, 15);
        let hyps = generator.generate_for_paradox(&paradox, &log);

        assert_eq!(hyps.len(), 3, "генератор должен предложить 3 гипотезы");

        // H1: resurrect, без Resolution.
        let resurrect = &hyps[0];
        assert!(
            resurrect.statement.contains("воскрес"),
            "H1 должен упоминать воскрешение: {}",
            resurrect.statement
        );
        assert!(
            resurrect.proposed_resolution.is_none(),
            "H1 (resurrect) не должен иметь Resolution"
        );
        assert!(
            resurrect.evidence_for.contains(&dead_id),
            "evidence_for должен содержать ID факта смерти"
        );

        // H2: flashback.
        let flashback = &hyps[1];
        match &flashback.proposed_resolution {
            Some(Resolution::MarkEventAs { event_id, kind }) => {
                assert_eq!(*event_id, 9, "H2 должен ссылаться на later_event=9");
                assert!(matches!(kind, EventKind::Flashback));
            }
            other => panic!("H2: ожидалось MarkEventAs Flashback, получено {:?}", other),
        }

        // H3: dream.
        let dream = &hyps[2];
        match &dream.proposed_resolution {
            Some(Resolution::MarkEventAs { event_id, kind }) => {
                assert_eq!(*event_id, 9);
                assert!(matches!(kind, EventKind::Dream));
            }
            other => panic!("H3: ожидалось MarkEventAs Dream, получено {:?}", other),
        }
    }

    #[test]
    fn test_verifier_accepts_flashback_resolution() {
        let verifier = HypothesisVerifier::new();
        let mut world = WorldState::new();
        let log = FactLog::new();
        set_alive(&mut world, "peter", false, 12);

        let hyp = Hypothesis {
            id: 0,
            statement: "Событие 7 является воспоминанием/flashback'ом".to_string(),
            proposed_resolution: Some(Resolution::MarkEventAs {
                event_id: 7,
                kind: EventKind::Flashback,
            }),
            evidence_for: Vec::new(),
            evidence_against: Vec::new(),
            status: HypothesisStatus::Pending,
            source: HypothesisSource::Algorithm,
        };

        let status = verifier.verify(&hyp, &world, &log);
        assert!(
            matches!(status, HypothesisStatus::Accepted),
            "Flashback должен быть Accepted, получено {:?}",
            status
        );

        // То же для Dream.
        let hyp_dream = Hypothesis {
            proposed_resolution: Some(Resolution::MarkEventAs {
                event_id: 7,
                kind: EventKind::Dream,
            }),
            ..hyp.clone()
        };
        let status = verifier.verify(&hyp_dream, &world, &log);
        assert!(matches!(status, HypothesisStatus::Accepted));

        // И для Vision и StoryWithinStory.
        for kind in [EventKind::Vision, EventKind::StoryWithinStory] {
            let hyp_kind = Hypothesis {
                proposed_resolution: Some(Resolution::MarkEventAs {
                    event_id: 7,
                    kind: kind.clone(),
                }),
                ..hyp.clone()
            };
            let status = verifier.verify(&hyp_kind, &world, &log);
            assert!(
                matches!(status, HypothesisStatus::Accepted),
                "{:?} должен быть Accepted",
                kind
            );
        }

        // Canonical — должен быть Rejected.
        let hyp_canon = Hypothesis {
            proposed_resolution: Some(Resolution::MarkEventAs {
                event_id: 7,
                kind: EventKind::Canonical,
            }),
            ..hyp.clone()
        };
        let status = verifier.verify(&hyp_canon, &world, &log);
        assert!(
            matches!(status, HypothesisStatus::Rejected(_)),
            "Canonical должен быть Rejected"
        );
    }

    #[test]
    fn test_verifier_rejects_resurrect_without_event() {
        let verifier = HypothesisVerifier::new();
        let mut world = WorldState::new();
        let mut log = FactLog::new();

        // Пётр мёртв в WorldState.
        set_alive(&mut world, "peter", false, 12);
        // Факт смерти в FactLog.
        let dead_id = assert_dead_fact(&mut log, "peter", 12);

        // Гипотеза «воскрес» без события Resurrect в нарративе.
        let hyp = Hypothesis {
            id: 0,
            statement: "Персонаж peter воскрес между Глава 12 и Глава 15".to_string(),
            proposed_resolution: None,
            evidence_for: vec![dead_id],
            evidence_against: Vec::new(),
            status: HypothesisStatus::Pending,
            source: HypothesisSource::Algorithm,
        };

        let status = verifier.verify(&hyp, &world, &log);
        match &status {
            HypothesisStatus::Rejected(reason) => {
                assert!(
                    reason.contains("воскрешения"),
                    "причина должна упоминать воскрешение: {}",
                    reason
                );
            }
            other => panic!(
                "Ожидалось Rejected без события воскрешения, получено {:?}",
                other
            ),
        }

        // Теперь добавим событие Resurrect после смерти — должно стать Accepted.
        let resurrect_event = Event {
            id: 0,
            actor: "peter".to_string(),
            action: Action::Resurrect,
            target: None,
            instrument: None,
            time: anchor(14),
            source_text: "Пётр воскрес".to_string(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        };
        log.record_event(resurrect_event);

        let status = verifier.verify(&hyp, &world, &log);
        assert!(
            matches!(status, HypothesisStatus::Accepted),
            "С событием Resurrect гипотеза должна быть Accepted, получено {:?}",
            status
        );
    }

    #[test]
    fn test_hypothesis_log_assigns_sequential_ids() {
        let mut log = HypothesisLog::new();

        let make_hyp = |statement: &str| Hypothesis {
            id: 0,
            statement: statement.to_string(),
            proposed_resolution: None,
            evidence_for: Vec::new(),
            evidence_against: Vec::new(),
            status: HypothesisStatus::Pending,
            source: HypothesisSource::Algorithm,
        };
        let h1 = make_hyp("Гипотеза 1");
        let h2 = make_hyp("Гипотеза 2");
        let h3 = make_hyp("Гипотеза 3");
        let h_template = make_hyp("шаблон");

        let id1 = log.add(h1);
        let id2 = log.add(h2);
        let id3 = log.add(h3);

        assert_eq!(id1, 1, "первая гипотеза должна получить id=1");
        assert_eq!(id2, 2, "вторая — id=2");
        assert_eq!(id3, 3, "третья — id=3");

        // Проверим, что ID записаны в сами гипотезы.
        assert_eq!(log.get(id1).unwrap().id, 1);
        assert_eq!(log.get(id2).unwrap().id, 2);
        assert_eq!(log.get(id3).unwrap().id, 3);

        // Несуществующий ID → None.
        assert!(log.get(999).is_none());

        // all() возвращает все 3.
        assert_eq!(log.all().len(), 3);

        // Все Pending → pending() возвращает 3, accepted()/rejected() — 0.
        assert_eq!(log.pending().len(), 3);
        assert_eq!(log.accepted().len(), 0);
        assert_eq!(log.rejected().len(), 0);

        // Обновим статус одной через get_mut.
        log.get_mut(id2).unwrap().status = HypothesisStatus::Accepted;
        assert_eq!(log.pending().len(), 2, "после accept одной — 2 pending");
        assert_eq!(log.accepted().len(), 1, "и 1 accepted");

        // Предзаданный ID не перезаписывается.
        let h_with_id = Hypothesis {
            id: 42,
            ..h_template
        };
        let id_pre = log.add(h_with_id);
        assert_eq!(id_pre, 42, "предзаданный id должен сохраниться");
        assert!(log.get(42).is_some());
    }
}
