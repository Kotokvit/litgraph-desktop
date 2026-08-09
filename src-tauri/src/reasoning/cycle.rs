//! # Reasoning Cycle — оркестратор всего reasoning engine
//!
//! `ReasoningCycle` — это верхний уровень reasoning engine, связывающий
//! вместе все модули Wave 1–4: `FactLog`, `WorldState`, `RuleSet`,
//! `ConstraintEngine`, `KnowledgeBase`, `InferenceEngine`,
//! `ContradictionDetector`, `CausalityEngine`, `HypothesisGenerator`,
//! `HypothesisVerifier`.
//!
//! ## Pipeline `run_cycle`
//!
//! Полный цикл рассуждения над порцией новых событий:
//!
//! 1. **`observe(events)`** — записывает события в `FactLog`, сдвигает
//!    `world.now` на момент последнего события. Правила НЕ применяются.
//! 2. **`build_state()`** — для каждого ещё не обработанного события
//!    применяет правила через `InferenceEngine`, мутирует `WorldState`,
//!    фиксирует выведенные факты в `FactLog`. Возвращает `Vec<InferredFact>`.
//! 3. **`reason()`** — проверяет все события на ограничения, ищет временные
//!    парадоксы и причинные петли. Возвращает `ContradictionReport`.
//! 4. **`generate_hypotheses(report)`** — для каждого нарушения и парадокса
//!    генерирует 3 гипотезы (flashback / dream / text-error или resurrect
//!    / flashback / dream), добавляет их в `HypothesisLog`.
//! 5. **`verify_all_pending()`** — верифицирует все Pending-гипотезы против
//!    текущего `WorldState` и `FactLog`.
//! 6. **`update_state(accepted)`** — для каждой принятой гипотезы с
//!    `Resolution::MarkEventAs` записывает классификацию события во
//!    внутренний `classifications` map (см. SPEC deviation ниже).
//!
//! ## SPEC deviation: `classifications` map
//!
//! SPEC §2.12 предлагает применять `Resolution::MarkEventAs` через
//! обновление `Event.provenance`. Но `Provenance` (Wave 1, `facts.rs`) —
//! закрытый enum без варианта `Flashback`/`Dream`, и мы не имеем права
//! модифицировать sibling-модули. Поэтому классификации событий хранятся
//! в отдельном `HashMap<EventId, EventKind>` на `ReasoningCycle`, а сам
//! `Event` остаётся неизменным. Геттеры `event_classification` и
//! `classifications` предоставляют доступ к этой карте.
//!
//! ## SPEC deviation: `memory` и `facts` — два независимых FactLog
//!
//! `KnowledgeBase::from_project` забирает владение `FactLog` (см. Wave 3
//! `memory.rs`). Мы не можем модифицировать sibling-модуль, чтобы
//! разделить владение. Поэтому в `ReasoningCycle` есть два независимых
//! `FactLog`: `self.facts` (активный, растёт по мере `observe`) и
//! `self.memory.facts` (изначально пустой, не синхронизируется
//! автоматически). Wave 5 integration при необходимости может
//! пересоздавать `memory` через `KnowledgeBase::from_project(project,
//! self.facts.clone())` — но для этого нужно добавить `Clone` для `FactLog`
//! в Wave 5, что выходит за рамки текущей задачи.
//!
//! См. `docs/reasoning/SPEC.md` §2.12 для формального контракта.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::models::Project;
use crate::reasoning::causality::CausalityEngine;
use crate::reasoning::constraints::{ConstraintEngine, ConstraintViolation};
use crate::reasoning::contradictions::{ContradictionDetector, ContradictionReport};
use crate::reasoning::facts::{Event, EventId, FactLog, FactValue};
use crate::reasoning::hypotheses::{
    EventKind, HypothesisGenerator, HypothesisId, HypothesisLog, HypothesisStatus,
    HypothesisVerifier, Resolution,
};
use crate::reasoning::inference::{InferenceEngine, InferredFact};
use crate::reasoning::memory::KnowledgeBase;
use crate::reasoning::rules::RuleSet;
use crate::reasoning::state::{StateTransition, WorldSnapshot, WorldState};

// ============================================================================
// CycleReport
// ============================================================================

/// Отчёт о выполненном цикле рассуждения. SPEC §2.12.
///
/// Возвращается из [`ReasoningCycle::run_cycle`] и содержит сводные счётчики
/// и снимок финального состояния мира. Сам `ContradictionReport` (с
/// парадоксами и причинными петлями) раскладывается на поля violations и
/// temporal_paradoxes прямо в структуру отчёта для удобства потребителя.
///
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleReport {
    /// Количество новых событий, обработанных в этом цикле (без дубликатов
    /// — повторная передача тех же событий через `run_cycle` даст 0).
    pub events_processed: usize,
    /// Количество выведенных фактов (длина `Vec<InferredFact>` из `build_state`).
    pub facts_asserted: usize,
    /// Нарушения ограничений, обнаруженные в `reason`.
    pub violations: Vec<ConstraintViolation>,
    /// Временные парадоксы, обнаруженные в `reason`.
    pub temporal_paradoxes: Vec<crate::reasoning::contradictions::TemporalParadox>,
    /// Количество сгенерированных гипотез в этом цикле.
    pub hypotheses_generated: usize,
    /// Количество принятых гипотез (статус `Accepted`).
    pub hypotheses_accepted: usize,
    /// Снимок `WorldState` на момент завершения цикла.
    pub final_state_snapshot: WorldSnapshot,
}

// ============================================================================
// ReasoningCycle
// ============================================================================

/// Оркестратор reasoning engine. Хранит все подсистемы и предоставляет
/// pipeline `run_cycle` для обработки порции событий.
///
/// Поля публичны (SPEC §2.12) — внешний код может читать/мутировать их
/// напрямую для тонкой настройки (например, добавить кастомное правило
/// в `rules` перед `run_cycle`).
pub struct ReasoningCycle {
    /// Текущее состояние мира — единственный источник истины.
    pub world: WorldState,
    /// Журнал фактов и событий (активный, растёт через `observe`).
    pub facts: FactLog,
    /// Набор правил для inference (по умолчанию `default_literary`).
    pub rules: RuleSet,
    /// Движок ограничений (по умолчанию `default_literary`).
    pub constraints: ConstraintEngine,
    /// База знаний для subgraph retrieval. Владеет собственным `FactLog`,
    /// отдельным от `self.facts` (см. SPEC deviation в модуле doc).
    pub memory: KnowledgeBase,
    /// Журнал гипотез.
    pub hypotheses: HypothesisLog,
    /// Forward-chaining inference engine.
    pub inference: InferenceEngine,
    /// Детектор противоречий (stateless).
    pub detector: ContradictionDetector,
    /// Движок каузальной пропагации.
    pub causality: CausalityEngine,
    /// Генератор гипотез (stateless).
    pub generator: HypothesisGenerator,
    /// Верификатор гипотез (stateless).
    pub verifier: HypothesisVerifier,

    /// ID событий, уже обработанных в `build_state`. Защищает от
    /// повторного применения правил при многократных вызовах `build_state`.
    processed_event_ids: HashSet<EventId>,

    /// Классификации событий, применённые через `update_state` (например,
    /// событие помечено как `Flashback`). Замещает несуществующий вариант
    /// `Provenance::Flashback` — см. SPEC deviation в модуле doc.
    classifications: HashMap<EventId, EventKind>,
}

impl ReasoningCycle {
    /// Создать цикл с пустыми данными и литературными наборами правил /
    /// ограничений по умолчанию. `world.now` = глава 1, offset 0.
    pub fn new() -> Self {
        Self {
            world: WorldState::new(),
            facts: FactLog::new(),
            rules: RuleSet::default_literary(),
            constraints: ConstraintEngine::default_literary(),
            memory: KnowledgeBase::new(),
            hypotheses: HypothesisLog::new(),
            inference: InferenceEngine::default_literary(),
            detector: ContradictionDetector::new(),
            causality: CausalityEngine::new(),
            generator: HypothesisGenerator::new(),
            verifier: HypothesisVerifier::new(),
            processed_event_ids: HashSet::new(),
            classifications: HashMap::new(),
        }
    }

    /// Инициализировать цикл из проекта: строит `KnowledgeBase` из узлов /
    /// рёбер проекта, предзаполняет `WorldState` фактом `alive=true` для
    /// каждого узла-персонажа, оставляет курсор времени на главе 1.
    ///
    /// **NB:** `KnowledgeBase::from_project` забирает владение `FactLog`.
    /// Мы передаём ему пустой `FactLog::new()`, поэтому `self.memory.facts`
    /// остаётся пустым и не синхронизируется с `self.facts` автоматически.
    /// См. SPEC deviation в модуле doc.
    pub fn from_project(project: &Project) -> Self {
        let mut cycle = Self::new();

        // Предзаполняем WorldState: каждый персонаж изначально жив.
        let now = cycle.world.now().clone();
        for node in &project.nodes {
            if node.node_type == "character" {
                let entity = &node.id;
                cycle.world.set(
                    entity,
                    "alive".to_string(),
                    FactValue::Bool(true),
                    StateTransition {
                        entity: entity.clone(),
                        attribute: "alive".to_string(),
                        old_value: None,
                        new_value: FactValue::Bool(true),
                        caused_by_event: None,
                        at: now.clone(),
                    },
                );
            }
        }

        // Инициализируем базу знаний из проекта. KB забирает владение
        // FactLog — передаём пустой, чтобы не конфликтовать с cycle.facts.
        cycle.memory = KnowledgeBase::from_project(project, FactLog::new());

        // Причинно-следственные рёбра: causality engine извлекает cause-рёбра
        // и сопоставляет их с событиями. На этом этапе событий пока нет,
        // поэтому links будет пустым — causal loops не обнаружатся.
        cycle.causality = CausalityEngine::from_edges(&project.edges, &cycle.facts);

        // world.now остаётся на главе 1 (WorldState::new default).
        cycle
    }

    /// Записать новые события в `FactLog` (без применения правил) и
    /// сдвинуть `world.now` на момент последнего события.
    ///
    /// События с `id == 0` получают авто-ID от `FactLog::record_event`.
    /// События с предзаданным ID сохраняются как есть (caller отвечает за
    /// уникальность).
    pub fn observe(&mut self, events: Vec<Event>) {
        let mut max_time = self.world.now().clone();
        for event in events {
            let event_time = event.time.clone();
            if event_time.after(&max_time) {
                max_time = event_time;
            }
            self.facts.record_event(event);
        }
        // Сдвигаем world.now только вперёд (advance_to паникует при откате).
        if max_time.after(self.world.now()) {
            self.world.advance_to(&max_time);
        }
    }

    /// Применить inference rules ко всем ещё не обработанным событиям.
    ///
    /// Идемпотентен: повторный вызов без новых `observe` не применит
    /// правила повторно (благодаря `processed_event_ids`).
    ///
    /// Возвращает `Vec<InferredFact>` — все выведенные факты (с их ID и
    /// ID правила, породившего каждый).
    pub fn build_state(&mut self) -> Vec<InferredFact> {
        // Собираем невыполненные события (клонируем, чтобы избежать
        // borrow-конфликта с self.facts в apply_event).
        let unprocessed: Vec<Event> = self
            .facts
            .all_events()
            .iter()
            .filter(|e| !self.processed_event_ids.contains(&e.id))
            .cloned()
            .collect();

        let mut all_inferred = Vec::new();
        for event in &unprocessed {
            let inferred = self.inference.apply_event(event, &mut self.world, &mut self.facts);
            all_inferred.extend(inferred);
            self.processed_event_ids.insert(event.id);
        }
        all_inferred
    }

    /// Запустить детекцию противоречий: проверить все события против
    /// ограничений, найти временные парадоксы и причинные петли.
    ///
    /// Возвращает полный `ContradictionReport`. Не мутирует состояние
    /// (но объявлен как `&mut self` для будущих расширений и консистентности
    /// с pipeline).
    pub fn reason(&mut self) -> ContradictionReport {
        // Клонируем события для check_all (он принимает &[Event]).
        let events: Vec<Event> = self.facts.all_events().to_vec();

        // 1. Проверка ограничений: для каждого события сверяемся с WorldState.
        let violations = self.constraints.check_all(&self.world, &events);

        // 2. Причинные петли: ищем циклы в графе CausalLink.
        let causal_loops = self.causality.detect_causal_loops();

        // 3. Временные парадоксы + агрегация в ContradictionReport.
        self.detector
            .detect_all(violations, &self.facts, &events, causal_loops)
    }

    /// Сгенерировать гипотезы для каждого нарушения и парадокса в отчёте,
    /// добавить их в `HypothesisLog`. Возвращает ID добавленных гипотез.
    ///
    /// Для `ConstraintViolation` — 3 гипотезы (flashback / dream / text-error).
    /// Для `TemporalParadox` — 3 гипотезы (resurrect / flashback / dream).
    pub fn generate_hypotheses(&mut self, report: &ContradictionReport) -> Vec<HypothesisId> {
        let mut ids = Vec::new();

        // Гипотезы для нарушений ограничений.
        for violation in &report.violations {
            let hyps = self.generator.generate_for_violation(violation, &self.facts);
            for hyp in hyps {
                let id = self.hypotheses.add(hyp);
                ids.push(id);
            }
        }

        // Гипотезы для временных парадоксов.
        for paradox in &report.temporal_paradoxes {
            let hyps = self.generator.generate_for_paradox(paradox, &self.facts);
            for hyp in hyps {
                let id = self.hypotheses.add(hyp);
                ids.push(id);
            }
        }

        ids
    }

    /// Верифицировать одну гипотезу (по ID) против текущего мира.
    /// Обновляет статус гипотезы в `HypothesisLog` и возвращает его.
    ///
    /// Если гипотезы с таким ID нет — возвращает `Rejected` с причиной.
    pub fn verify(&mut self, hyp_id: HypothesisId) -> HypothesisStatus {
        // Клонируем гипотезу, чтобы избежать borrow-конфликта:
        // verifier.verify требует &Hypothesis + &WorldState + &FactLog,
        // а затем нужно &mut HypothesisLog для записи статуса.
        let hyp_clone = self.hypotheses.get(hyp_id).cloned();
        let Some(hyp) = hyp_clone else {
            return HypothesisStatus::Rejected(format!(
                "Гипотеза с id={} не найдена в журнале",
                hyp_id
            ));
        };

        let status = self.verifier.verify(&hyp, &self.world, &self.facts);

        // Записываем новый статус в журнал.
        if let Some(stored) = self.hypotheses.get_mut(hyp_id) {
            stored.status = status.clone();
        }

        status
    }

    /// Верифицировать все Pending-гипотезы. Возвращает пары (ID, новый статус).
    pub fn verify_all_pending(&mut self) -> Vec<(HypothesisId, HypothesisStatus)> {
        // Собираем ID pending-гипотез заранее (immutable borrow заканчивается
        // до mutable borrow в self.verify).
        let pending_ids: Vec<HypothesisId> = self
            .hypotheses
            .pending()
            .iter()
            .map(|h| h.id)
            .collect();

        let mut results = Vec::with_capacity(pending_ids.len());
        for id in pending_ids {
            let status = self.verify(id);
            results.push((id, status));
        }
        results
    }

    /// Применить Resolution для каждой принятой гипотезы.
    ///
    /// Для `Resolution::MarkEventAs { event_id, kind }` — записывает
    /// `classifications[event_id] = kind`. Сам `Event` не модифицируется
    /// (см. SPEC deviation в модуле doc).
    ///
    /// **Конфликт разрешений:** несколько принятых гипотез могут
    /// предлагать разную классификацию для одного и того же события
    /// (например, Flashback и Dream для одного «мёртвый говорит»).
    /// Используется стратегия **first-write-wins**: первое применённое
    /// разрешение для данного `event_id` выигрывает, последующие
    /// игнорируются. Порядок обхода соответствует порядку ID в `accepted`
    /// (обычно — порядок генерации: violation-гипотезы раньше paradox-гипотез).
    pub fn update_state(&mut self, accepted: &[HypothesisId]) {
        for hyp_id in accepted {
            // Клонируем Resolution, чтобы не держать borrow на self.hypotheses
            // дольше необходимого.
            let resolution = self
                .hypotheses
                .get(*hyp_id)
                .and_then(|h| h.proposed_resolution.clone());

            if let Some(Resolution::MarkEventAs { event_id, kind }) = resolution {
                // First-write wins: не перезаписываем существующую
                // классификацию. Это предотвращает «дрейф» когда Dream
                // (сгенерированный позже) затирает Flashback.
                self.classifications.entry(event_id).or_insert(kind);
            }
        }
    }

    /// Полный pipeline: observe → build_state → reason → generate_hypotheses
    /// → verify_all_pending → update_state. Возвращает `CycleReport`.
    ///
    /// **Полностью идемпотентен**: повторный вызов с теми же событиями не
    /// приводит ни к повторной обработке правил, ни к дублированию записей
    /// в `FactLog`. Дедупликация выполняется по сигнатуре
    /// `(actor, action, target, time, source_text)` — события, чья
    /// сигнатура уже присутствует в журнале, отфильтровываются до `observe`.
    pub fn run_cycle(&mut self, events: Vec<Event>) -> CycleReport {
        // 0. Дедупликация: пропускаем события, чья сигнатура уже записана
        // в FactLog. Клонируем существующие события в Vec, чтобы снять
        // immutable borrow с self.facts перед &mut self.observe.
        let existing: Vec<Event> = self.facts.all_events().to_vec();
        let new_events: Vec<Event> = events
            .into_iter()
            .filter(|e| {
                !existing.iter().any(|ex| {
                    ex.actor == e.actor
                        && ex.action == e.action
                        && ex.target == e.target
                        && ex.time == e.time
                        && ex.source_text == e.source_text
                })
            })
            .collect();
        let events_processed = new_events.len();

        // 1. Записать только новые события.
        self.observe(new_events);

        // 2. Применить правила inference.
        let inferred = self.build_state();
        let facts_asserted = inferred.len();

        // 3. Найти противоречия.
        let report = self.reason();

        // 4. Сгенерировать гипотезы.
        let hyp_ids = self.generate_hypotheses(&report);
        let hypotheses_generated = hyp_ids.len();

        // 5. Верифицировать все pending-гипотезы.
        let verified = self.verify_all_pending();

        // 6. Применить Resolution для принятых.
        let accepted: Vec<HypothesisId> = verified
            .iter()
            .filter(|(_, status)| matches!(status, HypothesisStatus::Accepted))
            .map(|(id, _)| *id)
            .collect();
        let hypotheses_accepted = accepted.len();
        self.update_state(&accepted);

        CycleReport {
            events_processed,
            facts_asserted,
            violations: report.violations.clone(),
            temporal_paradoxes: report.temporal_paradoxes.clone(),
            hypotheses_generated,
            hypotheses_accepted,
            final_state_snapshot: self.world.snapshot(),
        }
    }

    /// Классификация события (если была применена через `update_state`).
    ///
    /// Возвращает `Some(EventKind::Flashback)` если гипотеза «это flashback»
    /// была принята для данного события. `None` — если событие не
    /// классифицировано (остаётся каноническим).
    pub fn event_classification(&self, event_id: EventId) -> Option<EventKind> {
        self.classifications.get(&event_id).cloned()
    }

    /// Доступ ко всем классификациям событий (read-only).
    pub fn classifications(&self) -> &HashMap<EventId, EventKind> {
        &self.classifications
    }
}

impl Default for ReasoningCycle {
    /// Делегирует в [`ReasoningCycle::new`].
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Юнит-тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LitNode, LitNodeData, Position};
    use crate::reasoning::facts::{Action, Provenance};
    use crate::reasoning::timeline::TemporalAnchor;

    /// Хелпер: `TemporalAnchor` для главы.
    fn anchor(chapter: u32) -> TemporalAnchor {
        TemporalAnchor {
            chapter_num: chapter,
            chapter_suffix: None,
            scene_index: None,
            char_offset: 0,
        }
    }

    /// Хелпер: построить минимальное событие.
    fn make_event(
        id: EventId,
        actor: &str,
        action: Action,
        target: Option<&str>,
        chapter: u32,
    ) -> Event {
        Event {
            id,
            actor: actor.to_string(),
            action,
            target: target.map(|s| s.to_string()),
            instrument: None,
            time: anchor(chapter),
            source_text: String::new(),
            confidence: 1.0,
            provenance: Provenance::SvoParser,
        }
    }

    /// Хелпер: построить узел-персонажа.
    fn make_character_node(id: &str, title: &str) -> LitNode {
        LitNode {
            id: id.to_string(),
            node_type: "character".to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: title.to_string(),
                body: String::new(),
                node_type: "character".to_string(),
                tags: Vec::new(),
                meta: None,
                full_text: None,
                versions: None,
            },
        }
    }

    /// Хелпер: построить минимальный проект с 2 персонажами.
    fn make_project() -> Project {
        Project {
            title: "Тест".to_string(),
            author: "Тест".to_string(),
            description: String::new(),
            nodes: vec![
                make_character_node("ivan", "Иван"),
                make_character_node("peter", "Пётр"),
            ],
            edges: Vec::new(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn test_run_cycle_with_kill_event_marks_target_dead() {
        let mut cycle = ReasoningCycle::new();

        // Иван убивает Петра в главе 12.
        let kill = make_event(0, "ivan", Action::Kill, Some("peter"), 12);
        let report = cycle.run_cycle(vec![kill]);

        // Цикл отработал: 1 событие обработано, >= 1 факт выведен.
        assert_eq!(report.events_processed, 1);
        assert!(
            report.facts_asserted >= 1,
            "kill должен породить хотя бы 1 факт (alive=false), получено {}",
            report.facts_asserted
        );

        // Пётр теперь мёртв в WorldState.
        let alive = cycle.world.get("peter", "alive");
        assert!(
            matches!(alive, Some(FactValue::Bool(false))),
            "Пётр должен быть мёртв после kill, получено {:?}",
            alive
        );

        // Иван всё ещё жив (kill не убивает актёра).
        let ivan_alive = cycle.world.get("ivan", "alive");
        assert!(
            matches!(ivan_alive, Some(FactValue::Bool(true))) || ivan_alive.is_none(),
            "Иван не должен умереть от собственного kill, получено {:?}",
            ivan_alive
        );

        // В kill-сценарии не должно быть противоречий (Иван жив, Пётр —
        // объект, не актёр).
        assert!(
            report.violations.is_empty(),
            "kill не должен порождать violations, получено {}",
            report.violations.len()
        );
        assert!(
            report.temporal_paradoxes.is_empty(),
            "kill не должен порождать temporal paradoxes"
        );
    }

    #[test]
    fn test_run_cycle_detects_dead_speaking_paradox() {
        let mut cycle = ReasoningCycle::new();

        // Событие 1: Иван убивает Петра в главе 12.
        let kill = make_event(0, "ivan", Action::Kill, Some("peter"), 12);
        // Событие 2: Пётр говорит в главе 15 (но он мёртв!).
        let speak = make_event(0, "peter", Action::Speak { topic: None }, None, 15);

        let report = cycle.run_cycle(vec![kill, speak]);

        assert_eq!(report.events_processed, 2);

        // Должен быть хотя бы один temporal paradox (Пётр мёртв, но говорит).
        assert!(
            !report.temporal_paradoxes.is_empty(),
            "Должен обнаружиться temporal paradox (мёртвый Пётр говорит)"
        );

        // Должно быть и нарушение ограничения dead_cannot_speak.
        let has_dead_speak = report
            .violations
            .iter()
            .any(|v| v.constraint_name == "dead_cannot_speak");
        assert!(
            has_dead_speak,
            "Должно быть нарушение dead_cannot_speak, получено violations: {:?}",
            report
                .violations
                .iter()
                .map(|v| &v.constraint_name)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_run_cycle_generates_hypothesis_for_paradox() {
        let mut cycle = ReasoningCycle::new();

        let kill = make_event(0, "ivan", Action::Kill, Some("peter"), 12);
        let speak = make_event(0, "peter", Action::Speak { topic: None }, None, 15);

        let report = cycle.run_cycle(vec![kill, speak]);

        // Гипотезы должны быть сгенерированы (3 на paradox + 3 на violation).
        assert!(
            report.hypotheses_generated >= 3,
            "Должно быть >= 3 сгенерированных гипотез, получено {}",
            report.hypotheses_generated
        );

        // В журнале гипотез столько же.
        assert_eq!(
            cycle.hypotheses.all().len(),
            report.hypotheses_generated,
            "HypothesisLog должен содержать все сгенерированные гипотезы"
        );

        // Хотя бы одна гипотеза должна упоминать воскрешение (из paradox).
        let has_resurrect = cycle
            .hypotheses
            .all()
            .iter()
            .any(|h| h.statement.contains("воскрес"));
        assert!(
            has_resurrect,
            "Среди гипотез должна быть «воскрес» (из temporal paradox)"
        );

        // Хотя бы одна гипотеза должна быть flashback-резолюцией.
        let has_flashback = cycle.hypotheses.all().iter().any(|h| {
            matches!(
                &h.proposed_resolution,
                Some(crate::reasoning::hypotheses::Resolution::MarkEventAs {
                    kind: crate::reasoning::hypotheses::EventKind::Flashback,
                    ..
                })
            )
        });
        assert!(has_flashback, "Среди гипотез должен быть flashback");
    }

    #[test]
    fn test_run_cycle_accepts_flashback_hypothesis() {
        let mut cycle = ReasoningCycle::new();

        let kill = make_event(0, "ivan", Action::Kill, Some("peter"), 12);
        let speak = make_event(0, "peter", Action::Speak { topic: None }, None, 15);

        let report = cycle.run_cycle(vec![kill, speak]);

        // Хотя бы одна гипотеза должна быть Accepted.
        assert!(
            report.hypotheses_accepted >= 1,
            "Хотя бы 1 гипотеза должна быть Accepted, получено {}",
            report.hypotheses_accepted
        );

        // Найдём принятую flashback-гипотезу.
        // Используем into_iter(), чтобы владение Vec перешло в итератор
        // (iter() занял бы borrow у временного значения).
        let accepted_flashback = cycle
            .hypotheses
            .accepted()
            .into_iter()
            .find(|h| {
                matches!(
                    &h.proposed_resolution,
                    Some(crate::reasoning::hypotheses::Resolution::MarkEventAs {
                        kind: crate::reasoning::hypotheses::EventKind::Flashback,
                        ..
                    })
                )
            })
            .cloned();

        let flashback = accepted_flashback.expect(
            "Среди Accepted-гипотез должна быть хотя бы одна с MarkEventAs Flashback",
        );

        // Проверим, что update_state записал классификацию.
        if let Some(crate::reasoning::hypotheses::Resolution::MarkEventAs {
            event_id,
            kind: _,
        }) = &flashback.proposed_resolution
        {
            let classification = cycle.event_classification(*event_id);
            assert!(
                matches!(
                    classification,
                    Some(crate::reasoning::hypotheses::EventKind::Flashback)
                ),
                "Событие {} должно быть классифицировано как Flashback, получено {:?}",
                event_id,
                classification
            );
        }

        // Карта классификаций не пуста.
        assert!(
            !cycle.classifications().is_empty(),
            "classifications map не должен быть пуст после update_state"
        );
    }

    #[test]
    fn test_from_project_initializes_character_alive_facts() {
        let project = make_project();
        let cycle = ReasoningCycle::from_project(&project);

        // Оба персонажа изначально живы в WorldState.
        let ivan_alive = cycle.world.get("ivan", "alive");
        assert!(
            matches!(ivan_alive, Some(FactValue::Bool(true))),
            "Иван должен быть жив после from_project, получено {:?}",
            ivan_alive
        );

        let peter_alive = cycle.world.get("peter", "alive");
        assert!(
            matches!(peter_alive, Some(FactValue::Bool(true))),
            "Пётр должен быть жив после from_project, получено {:?}",
            peter_alive
        );

        // Курсор времени — на главе 1.
        assert_eq!(
            cycle.world.now().chapter_num,
            1,
            "world.now должен быть на главе 1 после from_project"
        );

        // База знаний инициализирована: 2 узла.
        assert_eq!(
            cycle.memory.node_count(),
            2,
            "KB должна содержать 2 узла из проекта"
        );

        // Оба узла доступны по ID.
        assert!(cycle.memory.get_node("ivan").is_some());
        assert!(cycle.memory.get_node("peter").is_some());

        // Causality engine инициализирован (пустой — cause-рёбер нет).
        assert_eq!(cycle.causality.links().len(), 0);

        // HypothesisLog пуст.
        assert_eq!(cycle.hypotheses.all().len(), 0);

        // processed_event_ids пуст.
        assert!(cycle.processed_event_ids.is_empty());
    }
}
