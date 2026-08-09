//! planner.rs — Action planner для Reasoning Engine.
//!
//! Планировщик решает, какую операцию должен выполнить reasoning engine на
//! следующем шаге цикла. Это «входная точка» [`crate::reasoning::cycle`]:
//! цикл спрашивает планировщик `next_operation(&context)`, получает
//! [`Operation`] и исполняет его.
//!
//! # Архитектурный принцип
//!
//! **Понимание — это свойство алгоритма, а не LLM.** Планировщик НЕ вызывает
//! LLM и НЕ решает «о чём писать». Он только выбирает ОДНУ из детерминированных
//! операций: разобрать текст, построить состояние, проверить ограничения,
//! сгенерировать гипотезу и т.д. LLM подключается только когда выбрана
//! [`Operation::Act`] — и то через отдельный мост [`crate::reasoning::llm_bridge`].
//!
//! # Decision tree (SPEC §6 / brief §6)
//!
//! ```text
//! if user_query.is_some() and pending_events == 0:
//!     return Operation::Act { AnswerQuestion }
//! if pending_events > 0:
//!     return Operation::BuildState
//! if last_contradiction_count > 0 and unverified_hypotheses == 0:
//!     return Operation::Hypothesize
//! if unverified_hypotheses > 0:
//!     return Operation::Verify { hypothesis_id: <first pending> }
//! if user_query.is_some():
//!     return Operation::Act { AnswerQuestion }
//! return Operation::Idle
//! ```
//!
//! Дерево простое и намеренно пессимистичное: всегда сначала «перевариваем»
//! новые события (`BuildState`), потом разбираемся с противоречиями
//! (`Hypothesize` → `Verify`), и только потом отвечаем на пользовательский
//! запрос (`Act`). Это гарантирует, что ответ LLM строится на актуальном
//! состоянии мира, а не на устаревшем снимке.
//!
//! # Расширяемость
//!
//! Полноценный planner с reinforcement learning / priority queue может прийти
//! позже — текущий API (`next_operation(&PlannerContext) -> Operation`) уже
//! это поддерживает: контекст передаётся снаружи, сам `Planner` stateless.
//!
//! # Связь с другими модулями
//!
//! - [`ActionRequest`] ссылается на [`crate::reasoning::memory::Subgraph`] —
//!   релевантный фрагмент базы знаний для контекста LLM.
//! - [`Operation::Verify`] использует `hypothesis_id: u64` — это
//!   `HypothesisId` из `hypotheses.rs` (Wave 4 sibling). Здесь тип оставлен
//!   как `u64`, чтобы не вводить жёсткую зависимость от ещё не готового
//!   модуля (планировщик не должен знать внутренности хранилища гипотез).
//! - Никаких `pub use` из других reasoning-модулей (SPEC §4.6).

use serde::{Deserialize, Serialize};

use crate::reasoning::memory::Subgraph;

// ============================================================================
// Operation — что делать дальше
// ============================================================================

/// Одна операция, которую reasoning engine должен выполнить на текущем шаге.
///
/// Варианты упорядочены по «фазам» цикла рассуждения:
/// 1. **Восприятие** — [`Operation::Observe`] / [`Operation::BuildState`].
/// 2. **Рассуждение** — [`Operation::Reason`] / [`Operation::Hypothesize`] /
///    [`Operation::Verify`] / [`Operation::UpdateState`].
/// 3. **Действие** — [`Operation::Act`] (LLM-генерация) / [`Operation::Query`]
///    (чисто алгоритмический ответ из памяти).
/// 4. **Простой** — [`Operation::Idle`].
///
/// Варианты не обязаны выполняться в этом порядке — планировщик выбирает
/// следующий шаг по контексту. Но эта классификация помогает понимать
/// «слой» операции.
// `clippy::large_enum_variant`: вариант `Act` содержит `ActionRequest` с
// `Option<Subgraph>` (несколько Vec). Варианты имеют разный размер, но
// `Operation` почти всегда передаётся по ссылке или сразу разбирается —
// boxing существенно усложнит API без заметного выигрыша в памяти.
// SPEC/brief требует именно такую структуру (без Box).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum Operation {
    /// Распарсить новый текст в события (`raw_text` → semantic_parser →
    /// `Vec<Event>`). После этого события попадают в `FactLog` и ждут
    /// `BuildState`.
    Observe {
        /// Исходный текст для разбора (глава, сцена, отдельное предложение).
        raw_text: String,
    },

    /// Применить inference rules к pending-событиям: для каждого события
    /// найти matching `Rule`, применить `RuleEffect` к `WorldState`,
    /// утвердить новые факты в `FactLog`.
    BuildState,

    /// Запустить проверку ограничений + детектор противоречий на текущем
    /// состоянии. Результат — `Vec<ConstraintViolation>` +
    /// `ContradictionReport`. Если найдены противоречия, следующий шаг
    /// обычно [`Operation::Hypothesize`].
    Reason,

    /// Сгенерировать гипотезы для обнаруженных противоречий (например:
    /// «Пётр каким-то образом выжил после Г12» — возможно, это flashback).
    /// Гипотезы сохраняются в хранилище со статусом `Pending` и
    /// верифицируются на следующем шаге.
    Hypothesize,

    /// Проверить конкретную гипотезу (`hypothesis_id`) — собрать
    /// evidence_for / evidence_against, обновить статус до `Accepted` или
    /// `Rejected(reason)`.
    Verify {
        /// ID гипотезы для проверки (HypothesisId из hypotheses.rs).
        hypothesis_id: u64,
    },

    /// Применить принятые гипотезы к `WorldState` (например, пометить
    /// событие как `Flashback`, что ретрактит факт «alive = false» для
    /// соответствующего персонажа).
    UpdateState,

    /// Ответить на пользовательский вопрос, извлекая данные из памяти
    /// (`KnowledgeBase::retrieve_for_question`). Без LLM — чистый
    /// алгоритмический retrieval.
    Query {
        /// Вопрос пользователя в исходной формулировке.
        question: String,
    },

    /// Передать запрос на запись/генерацию в LLM-мост. Сам планировщик НЕ
    /// вызывает LLM — он только формирует [`ActionRequest`] с разрешениями
    /// и запретами. Вызов происходит в `llm_bridge.rs` + Tauri command layer.
    Act {
        /// Подготовленный запрос к LLM-мосту.
        action_request: ActionRequest,
    },

    /// Нечего делать — все события обработаны, противоречий нет, пользователь
    /// ничего не спрашивал. Цикл может ждать нового ввода.
    Idle,
}

// ============================================================================
// ActionRequest — запрос к LLM-мосту
// ============================================================================

/// Подготовленный запрос к LLM-мосту: что сгенерировать, какие ограничения
/// соблюдать, какой контекст использовать.
///
/// Планировщик (или Tauri command layer) собирает этот запрос и передаёт
/// в [`crate::reasoning::llm_bridge::LlmBridge::build_prompt`] для
/// формирования промпта. После генерации текста LLM ответ возвращается в
/// [`crate::reasoning::llm_bridge::LlmBridge::validate_response`] для
/// проверки на соответствие ограничениям.
///
/// Поля `constraints` / `allowed` / `forbidden` — человекочитаемые строки
/// на русском. Они попадают в промпт как есть (см. brief §3) и служат
/// «гuardrails» для LLM. Алгоритмическая проверка идёт через
/// `ConstraintEngine` в `validate_response` — эти поля только для LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    /// Тип действия (определяет структуру промпта и ожидаемый формат ответа).
    pub kind: ActionKind,
    /// Человекочитаемые ограничения на русском («Пётр мёртв с Главы 12»,
    /// «Анна в заточении с Главы 5»). Попадают в секцию «ОГРАНИЧЕНИЯ»
    /// промпта.
    pub constraints: Vec<String>,
    /// Что разрешено в генерации («flashback», «воспоминание персонажа о
    /// Петре», «упоминание тела»). Попадает в секцию «РАЗРЕШЕНО».
    pub allowed: Vec<String>,
    /// Что запрещено («Пётр не может говорить», «Пётр не может двигаться»,
    /// «Пётр не может появляться живым»). Попадает в секцию «ЗАПРЕЩЕНО».
    pub forbidden: Vec<String>,
    /// Исходная задача пользователя («Напиши сцену, где Иван вспоминает
    /// Петра»). Попадает в секцию «ЗАДАЧА».
    pub task: String,
    /// Релевантный подграф базы знаний (`KnowledgeBase::retrieve_for_question`
    /// или `subgraph(center, hops)`). Если `None` — контекст будет извлечён
    /// из `FactLog` напрямую. Если `Some` — попадает в секцию «КОНТЕКСТ».
    pub context_subgraph: Option<Subgraph>,
}

// ============================================================================
// ActionKind — тип действия для LLM
// ============================================================================

/// Тип LLM-действия. Определяет формат ожидаемого ответа и набор
/// ограничений по умолчанию.
///
/// `PartialEq` выводится явно — используется в тестах для сравнения с
/// ожидаемым `ActionKind`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ActionKind {
    /// Написать новую сцену с заданными персонажами / локацией / сюжетной
    /// точкой.
    WriteScene,
    /// Дописать главу — продолжить существующий нарратив с текущего места.
    ContinueChapter,
    /// Проанализировать сюжет: найти слабые места, логические дыры,
    /// недоразвитых персонажей.
    AnalyzePlot,
    /// Ответить на вопрос пользователя по нарративу (с опорой на состояние
    /// мира и базу знаний).
    AnswerQuestion,
    /// LLM предлагает альтернативное объяснение противоречию (например:
    /// «возможно, это flashback»). Гипотеза потом проверяется
    /// алгоритмически.
    GenerateHypothesis,
}

// ============================================================================
// PlannerContext — снимок состояния цикла для планировщика
// ============================================================================

/// Снимок состояния reasoning cycle, на основе которого планировщик принимает
/// решение. Все поля — счётчики/опции, без ссылок на внутренние структуры:
/// это позволяет передавать контекст через Tauri-команды и сериализовать
/// (для логирования / UI).
///
/// Заполняется вызывающим кодом (`cycle.rs` или Tauri command) перед вызовом
/// [`Planner::next_operation`].
#[derive(Debug, Clone, Default)]
pub struct PlannerContext {
    /// Количество необработанных событий в `FactLog` (ожидают `BuildState`).
    /// Если > 0 — приоритет у `BuildState` над всем остальным.
    pub pending_events: usize,
    /// Количество непроверенных гипотез. Если > 0 — приоритет у `Verify`.
    pub unverified_hypotheses: usize,
    /// Количество противоречий, найденных при последнем вызове `reason()`.
    /// Если > 0 и `unverified_hypotheses == 0` — пора генерировать гипотезы.
    pub last_contradiction_count: usize,
    /// Активный запрос пользователя (`Some`, если пользователь что-то спросил
    /// и ждёт ответа). Планировщик учитывает это при выборе между
    /// `AnswerQuestion` и алгоритмическими операциями.
    pub user_query: Option<String>,
}

// ============================================================================
// Planner — stateless decision maker
// ============================================================================

/// Stateless планировщик операций. Не хранит состояние — все данные
/// приходят через [`PlannerContext`]. Можно создавать через `Default` и
/// переиспользовать между вызовами.
///
/// # Example
///
/// ```ignore
/// use litgraph_desktop_lib::reasoning::planner::{Planner, PlannerContext, Operation};
///
/// let planner = Planner::new();
/// let ctx = PlannerContext {
///     pending_events: 3,
///     ..Default::default()
/// };
/// match planner.next_operation(&ctx) {
///     Operation::BuildState => { /* применить inference rules */ }
///     other => panic!("ожидался BuildState, получено {:?}", other),
/// }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Planner;

impl Planner {
    /// Создать новый планировщик. Stateful-состояния нет — `new` и
    /// `default` эквивалентны.
    pub fn new() -> Self {
        Self
    }

    /// Главная функция планировщика: по снимку состояния цикла выбирает
    /// следующую операцию.
    ///
    /// См. decision tree в модуле-doc. Порядок проверок важен — первые
    /// сработавшие условия выигрывают.
    ///
    /// # Idempotence
    ///
    /// Функция чистая: один и тот же `PlannerContext` всегда даёт один и
    /// тот же `Operation`. Это позволяет тестировать планировщик
    /// детерминированно и гарантировать повторяемость цикла рассуждения.
    pub fn next_operation(&self, ctx: &PlannerContext) -> Operation {
        // 1. Если есть активный пользовательский запрос и нет pending events —
        //    можно сразу отвечать (BuildState не нужен).
        if ctx.user_query.is_some() && ctx.pending_events == 0 {
            return Operation::Act {
                action_request: self.answer_question_request(ctx),
            };
        }

        // 2. Pending events имеют приоритет над всем остальным — пока мир не
        //    построен, любые ответы будут на устаревшем снимке.
        if ctx.pending_events > 0 {
            return Operation::BuildState;
        }

        // 3. Если после последнего reason() найдены противоречия и нет
        //    непроверенных гипотез — пора генерировать новые гипотезы.
        if ctx.last_contradiction_count > 0 && ctx.unverified_hypotheses == 0 {
            return Operation::Hypothesize;
        }

        // 4. Если есть непроверенные гипотезы — проверяем первую из них.
        //    Идентификатор «первой pending» планировщик не знает (хранилище
        //    гипотез живёт в `hypotheses.rs`), поэтому используем sentinel
        //    `1`. Wave 5 integration подставит реальный ID через расширение
        //    PlannerContext полем `first_pending_hypothesis_id: Option<u64>`.
        if ctx.unverified_hypotheses > 0 {
            return Operation::Verify { hypothesis_id: 1 };
        }

        // 5. Если мы сюда дошли — pending_events == 0, противоречий нет,
        //    гипотез нет. Если есть пользовательский запрос — отвечаем.
        //    (Эта ветка формально дублирует ветку 1, но оставлена для
        //    robustness: если decision tree эволюционирует, fallback на
        //    AnswerQuestion остаётся.)
        if ctx.user_query.is_some() {
            return Operation::Act {
                action_request: self.answer_question_request(ctx),
            };
        }

        // 6. Нечего делать.
        Operation::Idle
    }

    /// Convenience: построить `Operation::Act` для пользовательского запроса
    /// без учёта состояния цикла. Полезно, когда Tauri command получает
    /// вопрос пользователя и хочет сразу передать его в LLM-мост, минуя
    /// полный цикл рассуждения.
    ///
    /// Возвращает [`Operation::Act`] с [`ActionKind::AnswerQuestion`].
    /// Остальные поля `ActionRequest` остаются пустыми (Tauri command
    /// layer заполнит `constraints` / `allowed` / `forbidden` /
    /// `context_subgraph` перед вызовом LlmBridge).
    pub fn plan_for_user_query(&self, query: &str) -> Operation {
        Operation::Act {
            action_request: ActionRequest {
                kind: ActionKind::AnswerQuestion,
                constraints: Vec::new(),
                allowed: Vec::new(),
                forbidden: Vec::new(),
                task: query.to_string(),
                context_subgraph: None,
            },
        }
    }

    /// Внутренний хелпер: построить `ActionRequest` для `AnswerQuestion` из
    /// `ctx.user_query`. Если `user_query` — `None`, возвращается пустой
    /// task (это ветка не должна срабатывать, но defensive).
    fn answer_question_request(&self, ctx: &PlannerContext) -> ActionRequest {
        ActionRequest {
            kind: ActionKind::AnswerQuestion,
            constraints: Vec::new(),
            allowed: Vec::new(),
            forbidden: Vec::new(),
            task: ctx
                .user_query
                .clone()
                .unwrap_or_default(),
            context_subgraph: None,
        }
    }
}

// ============================================================================
// Юнит-тесты
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Шелпер: построить `PlannerContext` с указанными полями.
    fn ctx(
        pending: usize,
        unverified: usize,
        contradictions: usize,
        query: Option<&str>,
    ) -> PlannerContext {
        PlannerContext {
            pending_events: pending,
            unverified_hypotheses: unverified,
            last_contradiction_count: contradictions,
            user_query: query.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_next_operation_builds_state_when_pending_events() {
        // Есть pending events → BuildState, даже если есть пользовательский
        // запрос (сначала построим состояние, потом ответим).
        let planner = Planner::new();
        let context = ctx(5, 0, 0, Some("Что случилось с Анной?"));
        let op = planner.next_operation(&context);
        assert!(
            matches!(op, Operation::BuildState),
            "ожидался BuildState при pending_events > 0, получено {:?}",
            op
        );
    }

    #[test]
    fn test_next_operation_hypothesizes_when_contradictions() {
        // Есть противоречия, нет непроверенных гипотез → Hypothesize.
        let planner = Planner::new();
        let context = ctx(0, 0, 3, None);
        let op = planner.next_operation(&context);
        assert!(
            matches!(op, Operation::Hypothesize),
            "ожидался Hypothesize при last_contradiction_count > 0 и \
             unverified_hypotheses == 0, получено {:?}",
            op
        );
    }

    #[test]
    fn test_next_operation_verifies_when_pending_hypotheses() {
        // Есть непроверенные гипотезы → Verify { hypothesis_id: 1 }.
        let planner = Planner::new();
        let context = ctx(0, 2, 0, None);
        let op = planner.next_operation(&context);
        match op {
            Operation::Verify { hypothesis_id } => {
                assert_eq!(
                    hypothesis_id, 1,
                    "планировщик должен вернуть hypothesis_id = 1 (первая pending)"
                );
            }
            other => panic!(
                "ожидался Operation::Verify, получено {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_next_operation_idle_when_nothing_to_do() {
        // Всё обработано, пользователь ничего не спрашивает → Idle.
        let planner = Planner::new();
        let context = ctx(0, 0, 0, None);
        let op = planner.next_operation(&context);
        assert!(
            matches!(op, Operation::Idle),
            "ожидался Idle при пустом контексте, получено {:?}",
            op
        );
    }

    #[test]
    fn test_plan_for_user_query_returns_act_operation() {
        // Convenience-метод должен вернуть Operation::Act с AnswerQuestion.
        let planner = Planner::new();
        let op = planner.plan_for_user_query("Где Иван встретил Анну?");
        match op {
            Operation::Act { action_request } => {
                assert_eq!(
                    action_request.kind,
                    ActionKind::AnswerQuestion,
                    "ожидался ActionKind::AnswerQuestion"
                );
                assert_eq!(
                    action_request.task,
                    "Где Иван встретил Анну?",
                    "task должен совпадать с запросом"
                );
                assert!(
                    action_request.constraints.is_empty(),
                    "constraints должен быть пуст (заполняется caller'ом)"
                );
                assert!(
                    action_request.allowed.is_empty(),
                    "allowed должен быть пуст"
                );
                assert!(
                    action_request.forbidden.is_empty(),
                    "forbidden должен быть пуст"
                );
                assert!(
                    action_request.context_subgraph.is_none(),
                    "context_subgraph должен быть None"
                );
            }
            other => panic!(
                "ожидался Operation::Act, получено {:?}",
                other
            ),
        }
    }

    // ── Дополнительные coverage-тесты (не в brief, но помогают ловить
    //    регрессии в decision tree). ──────────────────────────────────────

    #[test]
    fn test_user_query_with_no_pending_events_returns_act_immediately() {
        // Ветка 1 decision tree: user_query + pending_events == 0 → Act.
        let planner = Planner::new();
        let context = ctx(0, 0, 0, Some("Расскажи о Петре"));
        match planner.next_operation(&context) {
            Operation::Act { action_request } => {
                assert_eq!(action_request.kind, ActionKind::AnswerQuestion);
                assert_eq!(action_request.task, "Расскажи о Петре");
            }
            other => panic!("ожидался Act, получено {:?}", other),
        }
    }

    #[test]
    fn test_contradictions_with_unverified_hypotheses_prefers_verify() {
        // last_contradiction_count > 0, но unverified > 0 → Verify (не
        // Hypothesize). Сначала проверяем старые гипотезы, потом генерируем
        // новые.
        let planner = Planner::new();
        let context = ctx(0, 1, 5, None);
        match planner.next_operation(&context) {
            Operation::Verify { hypothesis_id } => {
                assert_eq!(hypothesis_id, 1);
            }
            other => panic!(
                "ожидался Verify (unverified > 0), получено {:?}",
                other
            ),
        }
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)] // brief mandates Default impl
    fn test_default_context_returns_idle() {
        // Default PlannerContext — все нули / None → Idle.
        let planner = Planner::default();
        let context = PlannerContext::default();
        assert!(matches!(
            planner.next_operation(&context),
            Operation::Idle
        ));
    }

    #[test]
    #[allow(clippy::default_constructed_unit_structs)] // brief mandates Default impl
    fn test_planner_default_equals_new() {
        // Default и new должны давать идентичные планировщики (stateless).
        let p1 = Planner::new();
        let p2 = Planner::default();
        let ctx = ctx(1, 0, 0, None);
        assert_eq!(
            planner_op_kind(&p1.next_operation(&ctx)),
            planner_op_kind(&p2.next_operation(&ctx))
        );
    }

    /// Хелпер: превратить Operation в строку-идентификатор варианта
    /// (для сравнения без точных payload-ов).
    fn planner_op_kind(op: &Operation) -> &'static str {
        match op {
            Operation::Observe { .. } => "Observe",
            Operation::BuildState => "BuildState",
            Operation::Reason => "Reason",
            Operation::Hypothesize => "Hypothesize",
            Operation::Verify { .. } => "Verify",
            Operation::UpdateState => "UpdateState",
            Operation::Query { .. } => "Query",
            Operation::Act { .. } => "Act",
            Operation::Idle => "Idle",
        }
    }

    #[test]
    fn test_action_request_serializes_to_json() {
        // Smoke: ActionRequest должен сериализоваться в JSON без ошибок
        // (нужно для Tauri command boundary).
        let req = ActionRequest {
            kind: ActionKind::WriteScene,
            constraints: vec!["Пётр мёртв с Главы 12".to_string()],
            allowed: vec!["flashback".to_string()],
            forbidden: vec!["Пётр не может говорить".to_string()],
            task: "Напиши сцену воспоминания Ивана о Петре".to_string(),
            context_subgraph: None,
        };
        let json = serde_json::to_string(&req).expect("сериализация не должна падать");
        assert!(json.contains("WriteScene"));
        assert!(json.contains("Пётр мёртв с Главы 12"));
        assert!(json.contains("Напиши сцену воспоминания"));
    }

    #[test]
    fn test_operation_serializes_to_json() {
        // Smoke: Operation тоже должен сериализоваться (для логирования).
        let op = Operation::Verify { hypothesis_id: 42 };
        let json = serde_json::to_string(&op).expect("сериализация не должна падать");
        assert!(json.contains("Verify"));
        assert!(json.contains("42"));
    }
}
