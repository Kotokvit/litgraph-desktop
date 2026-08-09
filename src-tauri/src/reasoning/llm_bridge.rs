//! llm_bridge.rs — LLM-as-generator bridge with state enforcement.
//!
//! Этот модуль — **единственное место** в reasoning engine, где формируются
//! промпты для LLM и где сгенерированный текст проверяется на соответствие
//! состоянию мира. Любой другой модуль, который захочет позвать LLM, обязан
//! идти через [`LlmBridge`] (SPEC §5.1: «❌ Вызывать LLM из модулей кроме
//! `llm_bridge.rs`»).
//!
//! # Архитектурный принцип
//!
//! **Понимание — это свойство алгоритма, а не LLM.** Поэтому LlmBridge
//! работает так:
//!
//! 1. **build_prompt** — собирает системный и пользовательский промпт из
//!    `ActionRequest` + `WorldState` + `FactLog`. Промпт содержит только
//!    **актуальное** состояние мира (active facts) и человекочитаемые
//!    ограничения / разрешения / запреты. LLM получает как «истину» — то,
//!    что уже алгоритмически установлено.
//! 2. LLM генерирует текст (это ответственность Tauri command layer,
//!    который вызывает `crate::ai::chat` асинхронно).
//! 3. **validate_response** — прогоняет сгенерированный текст через
//!    [`parse_text_fallback`] (semantic_parser), извлекает события,
//!    проверяет каждое через `ConstraintEngine`. Если есть нарушения —
//!    строит feedback-промпт и возвращает `Reject`. LLM должна
//!    сгенерировать заново.
//!
//! # Sync / async boundary
//!
//! **КРИТИЧНО:** этот модуль полностью синхронный. LLM-вызовы (`chat` в
//! `crate::ai`) — асинхронные (через `reqwest` + `tokio`). Поэтому
//! `LlmBridge` НЕ вызывает LLM сам: он только
//!
//! - **строит** промпт (`build_prompt` → `(system, user)`);
//! - **парсит** ответ LLM (`validate_response` → `ValidationResult`).
//!
//! Асинхронный `chat`-вызов делает Tauri command layer (или
//! `tokio::task::spawn_blocking`), который:
//!
//! ```text
//! let (system, user) = bridge.build_prompt(&req, &world, &facts);
//! let generated = ai::chat(&provider, vec![
//!     ChatMessage { role: "system".into(), content: system },
//!     ChatMessage { role: "user".into(), content: user },
//! ]).await?;
//! match bridge.validate_response(&generated, &req, &world, &facts, &resolver, &chapters) {
//!     ValidationResult::Accept { events, .. } => { /* commit events */ }
//!     ValidationResult::Reject { feedback_prompt, .. } => {
//!         // retry with feedback_prompt as new user message
//!     }
//!     ValidationResult::Retry { reason } => {
//!         // retry with different approach
//!     }
//! }
//! ```
//!
//! # Связь с другими модулями
//!
//! - [`ActionRequest`] импортируется из `planner.rs` (Wave 4 sibling).
//! - `ConstraintEngine` / `ContradictionDetector` / `parse_text_fallback` /
//!   `EntityResolver` — из Wave 2/3 модулей.
//! - `WorldState` / `FactLog` — из Wave 1.
//! - `ParsedChapter` — из существующего `crate::parser::chapters`.
//! - **НЕ импортирует** `crate::ai::*` (sync/async boundary).

use crate::parser::chapters::ParsedChapter;
use crate::reasoning::constraints::{ConstraintEngine, ConstraintViolation};
use crate::reasoning::contradictions::{ContradictionDetector, ContradictionReport};
use crate::reasoning::facts::{Event, Fact, FactLog, FactValue};
use crate::reasoning::planner::ActionRequest;
use crate::reasoning::semantic_parser::{parse_text_fallback, EntityResolver};
use crate::reasoning::state::WorldState;

// ============================================================================
// ValidationResult — исход проверки сгенерированного текста
// ============================================================================

/// Результат валидации сгенерированного LLM текста.
///
/// Варианты упорядочены по «желательности»:
/// - [`ValidationResult::Accept`] — текст прошёл все проверки, события можно
///   коммитить в `FactLog` / `WorldState`.
/// - [`ValidationResult::Reject`] — текст нарушил ограничения, нужно
///   перегенерировать с feedback-промптом.
/// - [`ValidationResult::Retry`] — мягкая неудача (например, не удалось
///   извлечь события — LLM вернула пустой/нечитаемый текст). Тоже
///   перегенерировать, но без явного списка нарушений.
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Текст прошёл проверки: события извлечены и не нарушают ограничения.
    ///
    /// `events` — извлечённые события (готовы к `FactLog::record_event`).
    /// `report` — полный отчёт противоречий (включая temporal_paradoxes,
    /// если они есть — даже при пустом violations они могут быть
    /// информативными для UI).
    Accept {
        events: Vec<Event>,
        report: ContradictionReport,
    },

    /// Текст нарушил одно или несколько ограничений. LLM должна
    /// перегенерировать текст, используя `feedback_prompt` как
    /// дополнительный промпт.
    ///
    /// `violations` — список нарушений (для логирования / UI).
    /// `feedback_prompt` — готовый промпт с перечнем нарушений.
    Reject {
        violations: Vec<ConstraintViolation>,
        feedback_prompt: String,
    },

    /// Мягкая неудача: события не извлечены вообще, или текст пустой, или
    /// парсер не смог найти ни одного известного глагола. LLM должна
    /// попробовать другой подход (например, описать то же самое иначе).
    ///
    /// `reason` — человекочитаемое объяснение на русском.
    Retry {
        reason: String,
    },
}

// ============================================================================
// LlmBridge — stateless мост «запрос → промпт → валидация»
// ============================================================================

/// LLM-as-generator мост: строит промпты из `ActionRequest` + состояния
/// мира и валидирует ответ LLM на соответствие ограничениям.
///
/// **Stateless** — все данные приходят через параметры. Можно создавать через
/// `Default` и переиспользовать между вызовами. Это соответствует принципу
/// SPEC §0.4 («Determinism first»): одинаковый ввод даёт одинаковый промпт
/// и одинаковый вердикт.
///
/// # Example
///
/// ```ignore
/// use litgraph_desktop_lib::reasoning::llm_bridge::LlmBridge;
/// use litgraph_desktop_lib::reasoning::planner::{ActionKind, ActionRequest};
/// use litgraph_desktop_lib::reasoning::state::WorldState;
/// use litgraph_desktop_lib::reasoning::facts::FactLog;
///
/// let bridge = LlmBridge::new();
/// let req = ActionRequest {
///     kind: ActionKind::WriteScene,
///     constraints: vec!["Пётр мёртв с Главы 12".to_string()],
///     allowed: vec!["flashback".to_string(), "воспоминание".to_string()],
///     forbidden: vec!["Пётр не может говорить".to_string()],
///     task: "Напиши сцену, где Иван вспоминает Петра".to_string(),
///     context_subgraph: None,
/// };
/// let (system, user) = bridge.build_prompt(&req, &world, &facts);
/// // ... LLM generates text ...
/// // let result = bridge.validate_response(&text, &req, &world, &facts, &resolver, &chapters);
/// ```
#[derive(Debug, Clone, Default)]
pub struct LlmBridge;

impl LlmBridge {
    /// Создать новый мост. Stateful-состояния нет — `new` и `default`
    /// эквивалентны.
    pub fn new() -> Self {
        Self
    }

    /// Построить (system_prompt, user_prompt) для LLM.
    ///
    /// System prompt — фиксированная инструкция (см. brief §3) на русском:
    /// задаёт роль писателя, принцип «не объясняй состояние мира, а пиши
    /// в его рамках», и механизм отказа (`[REJECTED]`).
    ///
    /// User prompt — структура из 6 секций:
    /// 1. `СОСТОЯНИЕ МИРА (relevant subset)` — все активные факты из
    ///    `FactLog` (valid_until == None), отформатированные как
    ///    `entity.attribute = value (since Глава N)`.
    /// 2. `ОГРАНИЧЕНИЯ` — `request.constraints` (человекочитаемые строки).
    /// 3. `РАЗРЕШЕНО` — `request.allowed`.
    /// 4. `ЗАПРЕЩЕНО` — `request.forbidden`.
    /// 5. `КОНТЕКСТ (subgraph)` — если `request.context_subgraph` — `Some`,
    ///    краткое summary подграфа (через `Subgraph::summary()`).
    /// 6. `ЗАДАЧА` — `request.task`.
    ///
    /// # Почему `facts: &FactLog`, а не `&WorldState`
    ///
    /// `WorldState` хранит только текущие значения атрибутов, без
    /// временных меток. А для секции «СОСТОЯНИЕ МИРА» важно показать, с
    /// какой главы факт валиден («Пётр.alive = false (since Глава 12)» —
    /// это понятнее, чем просто «alive = false»). Поэтому идём в `FactLog`,
    /// у каждого `Fact` есть `valid_from: TemporalAnchor`.
    pub fn build_prompt(
        &self,
        request: &ActionRequest,
        _world: &WorldState,
        facts: &FactLog,
    ) -> (String, String) {
        let system_prompt = SYSTEM_PROMPT_TEMPLATE.to_string();

        let user_prompt = self.build_user_prompt(request, facts);

        (system_prompt, user_prompt)
    }

    /// Валидировать сгенерированный LLM текст.
    ///
    /// Алгоритм (см. brief §4):
    /// 1. `parse_text_fallback(generated_text, resolver, chapters)` → events.
    /// 2. Если events пустой → `Retry { reason: "Не удалось извлечь..." }`.
    /// 3. Для каждого event: `ConstraintEngine::check(world, event)`.
    ///    Собрать все violation'ы.
    /// 4. Если violations пустой → `Accept { events, report:
    ///    ContradictionDetector::detect_all(...) }`.
    /// 5. Иначе → `Reject { violations, feedback_prompt:
    ///    build_feedback_prompt(...) }`.
    ///
    /// # Constraint engine
    ///
    /// Используется `ConstraintEngine::default_literary()` — стандартный
    /// набор из 16 инвариантов (dead_cannot_speak, imprisoned_cannot_move,
    /// ...). Если в будущем потребуется передать кастомный engine, можно
    /// расширить сигнатуру (или хранить engine в поле `LlmBridge`).
    pub fn validate_response(
        &self,
        generated_text: &str,
        request: &ActionRequest,
        world: &WorldState,
        facts: &FactLog,
        resolver: &EntityResolver,
        chapters: &[ParsedChapter],
    ) -> ValidationResult {
        // 1. Парсим текст в события.
        let events = parse_text_fallback(generated_text, resolver, chapters);

        // 2. Нет событий → мягкая неудача.
        if events.is_empty() {
            return ValidationResult::Retry {
                reason: "Не удалось извлечь события из сгенерированного текста"
                    .to_string(),
            };
        }

        // 3. Проверяем каждое событие против ограничений.
        let engine = ConstraintEngine::default_literary();
        let mut all_violations: Vec<ConstraintViolation> = Vec::new();
        for ev in &events {
            let mut v = engine.check(world, ev);
            all_violations.append(&mut v);
        }

        // 4. Нет нарушений → Accept + полный contradiction report.
        if all_violations.is_empty() {
            let detector = ContradictionDetector::new();
            // Передаём пустые constraint_violations — мы только что
            // убедились, что их нет. Detect_all ещё построит temporal
            // paradoxes / causal loops, если они есть.
            let report = detector.detect_all(
                Vec::new(),
                facts,
                &events,
                Vec::new(),
            );
            return ValidationResult::Accept { events, report };
        }

        // 5. Есть нарушения → Reject + feedback-промпт.
        let feedback_prompt =
            self.build_feedback_prompt(request, generated_text, &all_violations);
        ValidationResult::Reject {
            violations: all_violations,
            feedback_prompt,
        }
    }

    /// Построить feedback-промпт для перегенерации текста.
    ///
    /// Формат (см. brief §5):
    /// ```text
    /// Твой предыдущий текст нарушает ограничения:
    ///
    /// 1. <violation 1 reason>
    /// 2. <violation 2 reason>
    ///
    /// Перепиши текст, устранив эти нарушения. Не пытайся "объяснить" их —
    /// просто не совершай запрещённых действий.
    ///
    /// === ИСХОДНАЯ ЗАДАЧА ===
    /// <original task>
    /// ```
    ///
    /// `original_request.task` — исходная задача (та, что была в первом
    /// промпте). `generated_text` здесь не используется напрямую (он уже
    /// был отправлен LLM как «предыдущий ответ»), но сохранён в сигнатуре
    /// для будущего расширения (например, можно включить цитату
    /// нарушившего предложения).
    pub fn build_feedback_prompt(
        &self,
        original_request: &ActionRequest,
        _generated_text: &str,
        violations: &[ConstraintViolation],
    ) -> String {
        let mut s = String::with_capacity(512);

        s.push_str("Твой предыдущий текст нарушает ограничения:\n\n");
        for (i, v) in violations.iter().enumerate() {
            // 1-based numbering для человекочитаемости.
            s.push_str(&format!("{}. {}\n", i + 1, v.reason));
        }

        s.push_str("\nПерепиши текст, устранив эти нарушения. Не пытайся \"объяснить\" их — просто\n");
        s.push_str("не совершай запрещённых действий.\n\n");

        s.push_str("=== ИСХОДНАЯ ЗАДАЧА ===\n");
        s.push_str(&original_request.task);

        s
    }

    // ── Внутренние хелперы ───────────────────────────────────────────────

    /// Собрать пользовательский промпт из `ActionRequest` + `FactLog`.
    fn build_user_prompt(&self, request: &ActionRequest, facts: &FactLog) -> String {
        let mut s = String::with_capacity(2048);

        // ── 1. СОСТОЯНИЕ МИРА (relevant subset) ─────────────────────────
        // Идём по активным фактам (valid_until == None). Это «текущая
        // правда» о мире. Если facts пуст — пишем заглушку.
        s.push_str("=== СОСТОЯНИЕ МИРА (relevant subset) ===\n");
        let active_facts: Vec<&Fact> = facts
            .all_facts()
            .iter()
            .filter(|f| f.valid_until.is_none())
            .collect();
        if active_facts.is_empty() {
            s.push_str("(пока нет установленных фактов)\n");
        } else {
            for f in &active_facts {
                s.push_str(&format!(
                    "- {}.{} = {} (since {})\n",
                    f.entity,
                    f.attribute,
                    format_fact_value(&f.value),
                    f.valid_from.display_chapter(),
                ));
            }
        }

        // ── 2. ОГРАНИЧЕНИЯ ──────────────────────────────────────────────
        s.push_str("\n=== ОГРАНИЧЕНИЯ ===\n");
        if request.constraints.is_empty() {
            s.push_str("(нет явных ограничений)\n");
        } else {
            for c in &request.constraints {
                s.push_str(&format!("- {}\n", c));
            }
        }

        // ── 3. РАЗРЕШЕНО ────────────────────────────────────────────────
        s.push_str("\n=== РАЗРЕШЕНО ===\n");
        if request.allowed.is_empty() {
            s.push_str("(нет явных разрешений)\n");
        } else {
            for a in &request.allowed {
                s.push_str(&format!("- {}\n", a));
            }
        }

        // ── 4. ЗАПРЕЩЕНО ────────────────────────────────────────────────
        s.push_str("\n=== ЗАПРЕЩЕНО ===\n");
        if request.forbidden.is_empty() {
            s.push_str("(нет явных запретов)\n");
        } else {
            for f in &request.forbidden {
                s.push_str(&format!("- {}\n", f));
            }
        }

        // ── 5. КОНТЕКСТ (subgraph) ─────────────────────────────────────
        s.push_str("\n=== КОНТЕКСТ (subgraph) ===\n");
        match &request.context_subgraph {
            Some(sg) => {
                // Если подграф пустой — явно пишем об этом, чтобы LLM не
                // «додумывала» контекст.
                if sg.is_empty() {
                    s.push_str("(подграф пуст)\n");
                } else {
                    s.push_str(&format!("{}\n", sg.summary()));
                    // Дополнительная детализация по фактам/событиям —
                    // краткая, чтобы не разбухал промпт.
                    if !sg.facts.is_empty() {
                        s.push_str("Факты:\n");
                        for f in &sg.facts {
                            s.push_str(&format!(
                                "- {}.{} = {}\n",
                                f.entity,
                                f.attribute,
                                format_fact_value(&f.value),
                            ));
                        }
                    }
                    if !sg.events.is_empty() {
                        s.push_str("События:\n");
                        for e in &sg.events {
                            s.push_str(&format!(
                                "- {} — {:?} ({})\n",
                                e.actor,
                                e.action,
                                e.time.display_chapter(),
                            ));
                        }
                    }
                }
            }
            None => {
                s.push_str("(не предоставлен)\n");
            }
        }

        // ── 6. ЗАДАЧА ───────────────────────────────────────────────────
        s.push_str("\n=== ЗАДАЧА ===\n");
        s.push_str(&request.task);

        s
    }
}

// ============================================================================
// System prompt template (фиксированная инструкция)
// ============================================================================

/// Системный промпт — фиксированная инструкция для LLM. Не зависит от
/// запроса / состояния мира. См. brief §3.
///
/// Ключевые элементы:
/// - Роль: «писатель в строгих рамках установленного состояния мира».
/// - Правила: не объяснять состояние, мёртвый не может действовать,
///   разрешены только ALLOWED, запрещены FORBIDDEN, отказ через `[REJECTED]`.
const SYSTEM_PROMPT_TEMPLATE: &str = "\
Ты — писатель, работающий в строгих рамках установленного состояния мира.
Твоя задача — генерировать текст, который НЕ противоречит фактам и ограничениям.

ПРАВИЛА:
1. Не пытайся \"объяснить\" или \"исправить\" состояние мира — оно задано как истина.
2. Если персонаж мёртв — он не может говорить, двигаться, действовать физически.
3. Разрешены только действия, перечисленные в ALLOWED.
4. Запрещены любые действия из FORBIDDEN.
5. Если не можешь выполнить задачу в рамках ограничений — верни \"[REJECTED]\".";

// ============================================================================
// Хелперы для форматирования
// ============================================================================

/// Человекочитаемое представление [`FactValue`] для промпта.
///
/// - `Bool(true)` → `"true"`, `Bool(false)` → `"false"`.
/// - `Str(s)` → саму строку (без кавычек — короче в промпте).
/// - `Int(n)` / `Float(f)` → строковое представление.
/// - `EntityRef(id)` → `id`.
/// - `List([...])` → `[item1, item2, ...]` (рекурсивно).
/// - `Unknown` → `"unknown"` (важный маркер — например, location мертвеца).
fn format_fact_value(v: &FactValue) -> String {
    match v {
        FactValue::Bool(b) => b.to_string(),
        FactValue::Str(s) => s.clone(),
        FactValue::Int(n) => n.to_string(),
        FactValue::Float(f) => f.to_string(),
        FactValue::EntityRef(id) => id.clone(),
        FactValue::List(items) => {
            let parts: Vec<String> = items.iter().map(format_fact_value).collect();
            format!("[{}]", parts.join(", "))
        }
        FactValue::Unknown => "unknown".to_string(),
    }
}

// ============================================================================
// Юнит-тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{LitNode, LitNodeData, Position};
    use crate::reasoning::facts::{EventId, FactId, Provenance};
    use crate::reasoning::planner::ActionKind;
    use crate::reasoning::state::StateTransition;
    use crate::reasoning::timeline::TemporalAnchor;

    // ── Хелперы для фикстур ──────────────────────────────────────────────

    /// Хелпер: `TemporalAnchor` для главы (без суффикса/сцены/offset).
    fn anchor(chapter: u32) -> TemporalAnchor {
        TemporalAnchor {
            chapter_num: chapter,
            chapter_suffix: None,
            scene_index: None,
            char_offset: 0,
        }
    }

    /// Хелпер: построить `LitNode` для персонажа.
    fn make_character_node(id: &str, title: &str) -> LitNode {
        LitNode {
            id: id.to_string(),
            node_type: "character".to_string(),
            position: Position { x: 0.0, y: 0.0 },
            data: LitNodeData {
                title: title.to_string(),
                body: String::new(),
                node_type: "character".to_string(),
                tags: vec![],
                meta: None,
                full_text: None,
                versions: None,
            },
        }
    }

    /// Хелпер: построить `ParsedChapter` с диапазоном pos..end, покрывающим
    /// весь текст (для тестов).
    fn make_chapter(num: u32, pos: usize, end: usize) -> ParsedChapter {
        ParsedChapter {
            num,
            title: format!("Глава {}", num),
            body: String::new(),
            full_text: String::new(),
            pos,
            end,
        }
    }

    /// Хелпер: построить `Fact` (active, без derived_from).
    fn make_fact(
        id: FactId,
        entity: &str,
        attr: &str,
        value: FactValue,
        from_chapter: u32,
    ) -> Fact {
        Fact {
            id,
            entity: entity.to_string(),
            attribute: attr.to_string(),
            value,
            derived_from: Vec::new(),
            valid_from: anchor(from_chapter),
            valid_until: None,
            provenance: Provenance::SvoParser,
        }
    }

    /// Хелпер: построить `ActionRequest` для теста (AnswerQuestion).
    fn answer_request(task: &str) -> ActionRequest {
        ActionRequest {
            kind: ActionKind::AnswerQuestion,
            constraints: vec![],
            allowed: vec![],
            forbidden: vec![],
            task: task.to_string(),
            context_subgraph: None,
        }
    }

    /// Хелпер: построить `WorldState` с Пётром мёртвым (alive = false) с Главы 12.
    fn world_with_dead_petr() -> WorldState {
        let mut world = WorldState::new();
        // Сначала Пётр был жив (с Главы 1).
        world.set(
            "Petr",
            "alive".to_string(),
            FactValue::Bool(true),
            StateTransition {
                entity: "Petr".to_string(),
                attribute: "alive".to_string(),
                old_value: None,
                new_value: FactValue::Bool(true),
                caused_by_event: Some(1),
                at: anchor(1),
            },
        );
        // С Главы 12 Пётр мёртв.
        world.advance_to(&anchor(12));
        world.set(
            "Petr",
            "alive".to_string(),
            FactValue::Bool(false),
            StateTransition {
                entity: "Petr".to_string(),
                attribute: "alive".to_string(),
                old_value: Some(FactValue::Bool(true)),
                new_value: FactValue::Bool(false),
                caused_by_event: Some(2),
                at: anchor(12),
            },
        );
        world
    }

    // ── Основные тесты (по brief) ────────────────────────────────────────

    #[test]
    fn test_build_prompt_includes_state_and_constraints() {
        // Подготовка: FactLog с активным фактом «Пётр.alive = false с Главы 12».
        let mut facts = FactLog::new();
        facts.assert_fact(make_fact(
            1,
            "Petr",
            "alive",
            FactValue::Bool(false),
            12,
        ));

        let bridge = LlmBridge::new();
        let request = ActionRequest {
            kind: ActionKind::WriteScene,
            constraints: vec!["Пётр мёртв с Главы 12".to_string()],
            allowed: vec!["flashback".to_string(), "воспоминание о Петре".to_string()],
            forbidden: vec!["Пётр не может говорить".to_string()],
            task: "Напиши сцену, где Иван вспоминает Петра".to_string(),
            context_subgraph: None,
        };
        let world = WorldState::new();
        let (system, user) = bridge.build_prompt(&request, &world, &facts);

        // System prompt содержит ключевые правила.
        assert!(
            system.contains("писатель"),
            "system prompt должен содержать 'писатель'"
        );
        assert!(
            system.contains("[REJECTED]"),
            "system prompt должен упоминать [REJECTED]"
        );
        assert!(
            system.contains("ALLOWED"),
            "system prompt должен упоминать ALLOWED"
        );
        assert!(
            system.contains("FORBIDDEN"),
            "system prompt должен упоминать FORBIDDEN"
        );

        // User prompt содержит все 6 секций.
        assert!(
            user.contains("=== СОСТОЯНИЕ МИРА"),
            "user prompt должен иметь секцию СОСТОЯНИЕ МИРА"
        );
        assert!(
            user.contains("Petr.alive = false"),
            "user prompt должен содержать активный факт 'Petr.alive = false'"
        );
        assert!(
            user.contains("since Глава 12"),
            "user prompt должен указывать 'since Глава 12'"
        );

        assert!(
            user.contains("=== ОГРАНИЧЕНИЯ ==="),
            "user prompt должен иметь секцию ОГРАНИЧЕНИЯ"
        );
        assert!(
            user.contains("Пётр мёртв с Главы 12"),
            "user prompt должен содержать constraint 'Пётр мёртв с Главы 12'"
        );

        assert!(
            user.contains("=== РАЗРЕШЕНО ==="),
            "user prompt должен иметь секцию РАЗРЕШЕНО"
        );
        assert!(
            user.contains("flashback"),
            "user prompt должен содержать разрешение 'flashback'"
        );

        assert!(
            user.contains("=== ЗАПРЕЩЕНО ==="),
            "user prompt должен иметь секцию ЗАПРЕЩЕНО"
        );
        assert!(
            user.contains("Пётр не может говорить"),
            "user prompt должен содержать запрет 'Пётр не может говорить'"
        );

        assert!(
            user.contains("=== КОНТЕКСТ (subgraph) ==="),
            "user prompt должен иметь секцию КОНТЕКСТ"
        );
        assert!(
            user.contains("(не предоставлен)"),
            "при None context_subgraph ожидаем '(не предоставлен)'"
        );

        assert!(
            user.contains("=== ЗАДАЧА ==="),
            "user prompt должен иметь секцию ЗАДАЧА"
        );
        assert!(
            user.contains("Напиши сцену, где Иван вспоминает Петра"),
            "user prompt должен содержать исходную задачу"
        );
    }

    #[test]
    fn test_validate_response_accepts_compliant_text() {
        // Сценарий: Иван жив, говорит в Главе 5 → должно быть Accepted.
        let mut world = WorldState::new();
        world.set(
            "Ivan",
            "alive".to_string(),
            FactValue::Bool(true),
            StateTransition {
                entity: "Ivan".to_string(),
                attribute: "alive".to_string(),
                old_value: None,
                new_value: FactValue::Bool(true),
                caused_by_event: Some(1),
                at: anchor(1),
            },
        );

        let facts = FactLog::new();
        let resolver = EntityResolver::from_nodes(&[make_character_node("Ivan", "Иван")]);
        // Текст целиком укладывается в chapter 5 (pos 0, end 1000).
        let chapters = vec![make_chapter(5, 0, 1000)];

        // «Иван сказал...» — Speak от живого персонажа → не нарушает dead_cannot_speak.
        let generated = "Иван сказал Анне привет.";

        let bridge = LlmBridge::new();
        let request = answer_request("Напиши сцену");
        let result = bridge.validate_response(
            generated,
            &request,
            &world,
            &facts,
            &resolver,
            &chapters,
        );

        match result {
            ValidationResult::Accept { events, report } => {
                // Должно быть извлечено хотя бы одно событие (Speak от Ивана).
                assert!(
                    !events.is_empty(),
                    "ожидалось хотя бы одно событие в Accept"
                );
                // Speak от живого → violations пустой, temporal_paradoxes
                // тоже пустой (alive = true).
                assert!(
                    report.violations.is_empty(),
                    "violations должен быть пуст, получено: {:?}",
                    report.violations
                );
                assert!(
                    report.temporal_paradoxes.is_empty(),
                    "temporal_paradoxes должен быть пуст"
                );
            }
            other => panic!(
                "ожидался Accept для compliant text, получено {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_validate_response_rejects_dead_character_speaking() {
        // Сценарий: Пётр мёртв с Главы 12, в Главе 15 «Пётр сказал...» → Reject.
        let world = world_with_dead_petr();
        let facts = FactLog::new();
        let resolver = EntityResolver::from_nodes(&[make_character_node("Petr", "Пётр")]);
        // Глава 15 (pos 0..2000) — после смерти Петра (Глава 12).
        let chapters = vec![make_chapter(15, 0, 2000)];

        // «Пётр сказал...» — Speak от мертвеца → нарушает dead_cannot_speak.
        let generated = "Пётр сказал Анне о своей смерти.";

        let bridge = LlmBridge::new();
        let request = answer_request("Напиши сцену с Петром");
        let result = bridge.validate_response(
            generated,
            &request,
            &world,
            &facts,
            &resolver,
            &chapters,
        );

        match result {
            ValidationResult::Reject {
                violations,
                feedback_prompt,
            } => {
                // Должно быть хотя бы одно нарушение dead_cannot_speak.
                assert!(
                    !violations.is_empty(),
                    "ожидалось хотя бы одно нарушение для мёртвого, говорящего"
                );
                let names: Vec<&str> =
                    violations.iter().map(|v| v.constraint_name.as_str()).collect();
                assert!(
                    names.contains(&"dead_cannot_speak"),
                    "ожидалось нарушение 'dead_cannot_speak', получены: {:?}",
                    names
                );

                // Feedback-промпт должен содержать reason нарушения и
                // исходную задачу.
                assert!(
                    feedback_prompt.contains("нарушает ограничения"),
                    "feedback должен начинаться с 'нарушает ограничения'"
                );
                assert!(
                    feedback_prompt.contains("мёртв"),
                    "feedback должен упоминать 'мёртв' (из reason нарушения)"
                );
                assert!(
                    feedback_prompt.contains("=== ИСХОДНАЯ ЗАДАЧА ==="),
                    "feedback должен содержать секцию ИСХОДНАЯ ЗАДАЧА"
                );
                assert!(
                    feedback_prompt.contains("Напиши сцену с Петром"),
                    "feedback должен содержать исходную задачу"
                );
            }
            other => panic!(
                "ожидался Reject для мёртвого, говорящего, получено {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_validate_response_retries_when_no_events_extracted() {
        // Сценарий: текст без известных глаголов → нет событий → Retry.
        let world = WorldState::new();
        let facts = FactLog::new();
        let resolver = EntityResolver::from_nodes(&[make_character_node("Ivan", "Иван")]);
        let chapters = vec![make_chapter(1, 0, 1000)];

        // Текст без «убил/сказал/умер/воскрес/пришёл» — fallback-парсер
        // не найдёт ни одного известного глагола.
        let generated = "Тишина. Только ветер гуляет по полю.";

        let bridge = LlmBridge::new();
        let request = answer_request("Опиши пейзаж");
        let result = bridge.validate_response(
            generated,
            &request,
            &world,
            &facts,
            &resolver,
            &chapters,
        );

        match result {
            ValidationResult::Retry { reason } => {
                assert!(
                    reason.contains("Не удалось извлечь события"),
                    "reason должен содержать 'Не удалось извлечь события', получено: {}",
                    reason
                );
            }
            other => panic!(
                "ожидался Retry для текста без событий, получено {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_build_feedback_prompt_lists_violations() {
        // Подготовка: два нарушения (dead_cannot_speak + dead_cannot_move).
        let violations = vec![
            ConstraintViolation {
                constraint_name: "dead_cannot_speak".to_string(),
                event_id: EventId::default(),
                actor: "Petr".to_string(),
                attempted_action: crate::reasoning::facts::Action::Speak { topic: None },
                reason: "Невозможно: персонаж мёртв, но пытается говорить".to_string(),
                conflicting_fact: None,
                at: anchor(15),
            },
            ConstraintViolation {
                constraint_name: "dead_cannot_move".to_string(),
                event_id: EventId::default(),
                actor: "Petr".to_string(),
                attempted_action: crate::reasoning::facts::Action::Move {
                    destination: "Замок".to_string(),
                },
                reason: "Невозможно: персонаж мёртв, но перемещается".to_string(),
                conflicting_fact: None,
                at: anchor(15),
            },
        ];

        let bridge = LlmBridge::new();
        let request = ActionRequest {
            kind: ActionKind::WriteScene,
            constraints: vec![],
            allowed: vec![],
            forbidden: vec![],
            task: "Напиши сцену с Петром".to_string(),
            context_subgraph: None,
        };

        let prompt = bridge.build_feedback_prompt(&request, "старый текст", &violations);

        // Заголовок.
        assert!(
            prompt.starts_with("Твой предыдущий текст нарушает ограничения:"),
            "feedback должен начинаться с заголовка"
        );

        // Оба нарушения перечислены (1-based numbering).
        assert!(
            prompt.contains("1. Невозможно: персонаж мёртв, но пытается говорить"),
            "первое нарушение должно быть в списке: {}",
            prompt
        );
        assert!(
            prompt.contains("2. Невозможно: персонаж мёртв, но перемещается"),
            "второе нарушение должно быть в списке: {}",
            prompt
        );

        // Инструкция по переписыванию.
        assert!(
            prompt.contains("Перепиши текст, устранив эти нарушения"),
            "feedback должен содержать инструкцию 'Перепиши текст'"
        );
        assert!(
            prompt.contains("не совершай запрещённых действий"),
            "feedback должен содержать 'не совершай запрещённых действий'"
        );

        // Исходная задача.
        assert!(
            prompt.contains("=== ИСХОДНАЯ ЗАДАЧА ==="),
            "feedback должен содержать секцию ИСХОДНАЯ ЗАДАЧА"
        );
        assert!(
            prompt.contains("Напиши сцену с Петром"),
            "feedback должен содержать исходную задачу"
        );
    }

    // ── Дополнительные coverage-тесты ────────────────────────────────────

    #[test]
    #[allow(clippy::default_constructed_unit_structs)] // brief mandates Default impl
    fn test_default_bridge_equals_new() {
        // Default и new должны давать идентичные мосты (stateless).
        let b1 = LlmBridge::new();
        let b2 = LlmBridge::default();
        let req = answer_request("test");
        let world = WorldState::new();
        let facts = FactLog::new();
        let (s1, u1) = b1.build_prompt(&req, &world, &facts);
        let (s2, u2) = b2.build_prompt(&req, &world, &facts);
        assert_eq!(s1, s2);
        assert_eq!(u1, u2);
    }

    #[test]
    fn test_build_prompt_handles_empty_factlog() {
        // Пустой FactLog → секция состояния мира с заглушкой.
        let bridge = LlmBridge::new();
        let request = answer_request("Что-нибудь");
        let world = WorldState::new();
        let facts = FactLog::new();
        let (_system, user) = bridge.build_prompt(&request, &world, &facts);
        assert!(
            user.contains("(пока нет установленных фактов)"),
            "при пустом FactLog ожидаем заглушку в СОСТОЯНИЕ МИРА"
        );
    }

    #[test]
    fn test_build_prompt_with_subgraph_includes_summary() {
        // Если передан context_subgraph — его summary попадает в промпт.
        use crate::models::LitEdge;
        use crate::reasoning::memory::Subgraph;

        let subgraph = Subgraph {
            center: "Petr".to_string(),
            nodes: vec![make_character_node("Petr", "Пётр")],
            edges: Vec::<LitEdge>::new(),
            facts: vec![make_fact(1, "Petr", "alive", FactValue::Bool(false), 12)],
            events: Vec::new(),
            max_hops: 2,
        };

        let bridge = LlmBridge::new();
        let request = ActionRequest {
            kind: ActionKind::AnswerQuestion,
            constraints: vec![],
            allowed: vec![],
            forbidden: vec![],
            task: "Расскажи о Петре".to_string(),
            context_subgraph: Some(subgraph),
        };
        let world = WorldState::new();
        let facts = FactLog::new();
        let (_system, user) = bridge.build_prompt(&request, &world, &facts);

        assert!(
            user.contains("Подграф вокруг «Petr»"),
            "user prompt должен содержать summary подграфа"
        );
        assert!(
            user.contains("Petr.alive = false"),
            "user prompt должен содержать факт из подграфа"
        );
    }

    #[test]
    fn test_format_fact_value_all_variants() {
        // Smoke: format_fact_value должен покрывать все варианты FactValue.
        assert_eq!(format_fact_value(&FactValue::Bool(true)), "true");
        assert_eq!(format_fact_value(&FactValue::Bool(false)), "false");
        assert_eq!(format_fact_value(&FactValue::Str("Замок".into())), "Замок");
        assert_eq!(format_fact_value(&FactValue::Int(42)), "42");
        assert_eq!(
            format_fact_value(&FactValue::Float(1.5)),
            "1.5"
        );
        assert_eq!(
            format_fact_value(&FactValue::EntityRef("char-anna".into())),
            "char-anna"
        );
        assert_eq!(
            format_fact_value(&FactValue::List(vec![
                FactValue::Str("a".into()),
                FactValue::Str("b".into()),
            ])),
            "[a, b]"
        );
        assert_eq!(format_fact_value(&FactValue::Unknown), "unknown");
    }

    #[test]
    fn test_validate_response_accept_has_temporal_paradox_for_resurrect_without_death() {
        // Сценарий: персонаж не был мёртв, но воскресает — temporal paradox.
        // Constraint engine не имеет ограничения на resurrect-without-death,
        // поэтому violations будет пустой → Accept. Но report должен
        // содержать temporal_paradox (детектор их ищет).
        let mut world = WorldState::new();
        // Иван жив (alive = true), никогда не умирал.
        world.set(
            "Ivan",
            "alive".to_string(),
            FactValue::Bool(true),
            StateTransition {
                entity: "Ivan".to_string(),
                attribute: "alive".to_string(),
                old_value: None,
                new_value: FactValue::Bool(true),
                caused_by_event: Some(1),
                at: anchor(1),
            },
        );

        let facts = FactLog::new();
        let resolver = EntityResolver::from_nodes(&[make_character_node("Ivan", "Иван")]);
        let chapters = vec![make_chapter(5, 0, 1000)];

        // «Иван воскрес» — Resurrect для живого → temporal paradox.
        let generated = "Иван воскрес из мёртвых.";

        let bridge = LlmBridge::new();
        let request = answer_request("Напиши чудесное воскресение");
        let result = bridge.validate_response(
            generated,
            &request,
            &world,
            &facts,
            &resolver,
            &chapters,
        );

        // Constraint engine не запрещает Resurrect → violations пустой → Accept.
        // Но ContradictionDetector должен найти temporal paradox.
        match result {
            ValidationResult::Accept { events, report } => {
                assert!(!events.is_empty(), "ожидалось событие (Resurrect)");
                // Temporal paradox должен быть в отчёте.
                assert!(
                    !report.temporal_paradoxes.is_empty(),
                    "ожидался temporal_paradox для воскресения без смерти, \
                     отчёт: {:?}",
                    report
                );
            }
            other => panic!(
                "ожидался Accept (constraint engine не запрещает Resurrect), \
                 получено {:?}",
                other
            ),
        }
    }
}
