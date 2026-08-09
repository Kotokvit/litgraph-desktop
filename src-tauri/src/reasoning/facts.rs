//! Слой данных «факты и события» для Reasoning Engine.
//!
//! Этот модуль — фундамент всей системы рассуждений LitGraph. Здесь живут
//! типы `Event` (что произошло в нарративе) и `Fact` (что является истиной
//! о сущности в данный момент времени), а также `FactLog` — простой
//! append-only журнал, который их хранит.
//!
//! # Архитектурный принцип
//!
//! **Понимание — это свойство алгоритма, а не LLM.** Поэтому `FactLog` не
//! вызывает LLM и не задаёт вопросов внешнему миру: это чистая, синхронная,
//! детерминированная структура данных. Любая «правда» о мире выражается как
//! `Fact` и хранится здесь; любое изменение мира логируется как `Event`.
//!
//! # Связь с другими модулями
//!
//! - [`TemporalAnchor`] заимствуется из `timeline.rs` (модуль timeline owns
//!   ordering semantics). Мы не переопределяем его здесь.
//! - `EntityId = String` совпадает с `LitNode.id`, чтобы reasoning engine
//!   работал поверх графа без копирования (см. SPEC §2.1, §3.2).
//! - `Provenance` отмечает происхождение каждого факта/события: парсер ли
//!   нашёл, LLM ли предложила, пользователь ли ввёл. Это нужно для audit
//!   trail и для решения конфликтов достоверности.
//!
//! # Retraction semantics
//!
//! `FactLog` не хранит «текущее время» явно (это ответственность
//! [`crate::reasoning::state::WorldState`]). При ретракции факта в качестве
//! «now» используется временная метка последнего записанного события — это
//! разумный прокси для «текущего момента нарратива». Если событий пока нет,
//! fallback — собственный `valid_from` факта (мгновенная ретракция).

use serde::{Deserialize, Serialize};

use crate::reasoning::timeline::TemporalAnchor;

/// Идентификатор сущности. Совпадает с `LitNode.id` (строка).
/// См. SPEC §2.1 — это позволяет reasoning engine работать поверх графа.
pub type EntityId = String;

/// Монотонно возрастающий счётчик фактов внутри `FactLog`.
pub type FactId = u64;

/// Монотонно возрастающий счётчик событий внутри `FactLog`.
pub type EventId = u64;

/// Полярность глагола для `Action::Custom`. Используется, когда verb не
/// попал в лексикон, но мы хотим сохранить семантическую окраску.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum VerbPolarity {
    /// Позитивное действие («спас», «помог»).
    Positive,
    /// Негативное действие («предал», «ударил»).
    Negative,
    /// Нейтральное действие («сказал», «пошёл»).
    Neutral,
}

/// Тип действия в событии. Перечисление покрывает основные литературные
/// глаголы, разбитые на 7 категорий (см. SPEC §2.4).
///
/// Для неизвестных глаголов используется вариант `Custom` с полярностью —
/// это позволяет rules.rs и constraints.rs применять общие эвристики.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Action {
    // ── Физические действия над целью ──────────────────────────────────
    /// Убить.
    Kill,
    /// Ранить.
    Wound,
    /// Ударить (без летального исхода).
    Hit,
    /// Захватить (в плен, в плену).
    Capture,
    /// Заключить под стражу.
    Imprison,
    /// Освободить (из плена/тюрьмы).
    Free,
    /// Вылечить.
    Heal,
    /// Касаться (тактильный контакт).
    Touch,

    // ── Перемещение ────────────────────────────────────────────────────
    /// Двигаться к месту назначения.
    Move { destination: String },
    /// Прибыть в место назначения.
    Arrive { destination: String },
    /// Покинуть место.
    Leave { source: String },

    // ── Коммуникация ───────────────────────────────────────────────────
    /// Говорить (тема опциональна — не всегда извлекается из текста).
    Speak { topic: Option<String> },
    /// Спрашивать о теме.
    Ask { topic: String },
    /// Сообщать тему конкретному адресату.
    Tell { topic: String, to: EntityId },

    // ── Социальные отношения ───────────────────────────────────────────
    /// Жениться / сочетаться с партнёром.
    Marry { partner: EntityId },
    /// Предать жертву.
    Betray { victim: EntityId },
    /// Союз с партнёром.
    Ally { partner: EntityId },

    // ── Когнитивные действия ───────────────────────────────────────────
    /// Узнать факт.
    Know { fact: String },
    /// Забыть факт.
    Forget { fact: String },
    /// Желать цели.
    Want { goal: String },
    /// Планировать цель.
    Plan { goal: String },

    // ── Эмоциональные действия ─────────────────────────────────────────
    /// Влюбиться в партнёра.
    FallInLove { partner: EntityId },
    /// Возненавидеть цель.
    Hate { target: EntityId },

    // ── Мета-действия (сюжетно важные, но не физические) ──────────────
    /// Обнаружить факт (new knowledge entering the narrative).
    Discover { fact: String },
    /// Трансформироваться в новую форму (оборотень, вампир, etc.).
    Transform { new_form: String },
    /// Умереть (без явного убийцы — естественная смерть / suicide).
    Die,
    /// Воскреснуть.
    Resurrect,

    // ── Fallback для неизвестных глаголов ──────────────────────────────
    /// Кастомное действие: verb не из лексикона, но сохранены лемма и
    /// полярность для эвристик.
    Custom {
        verb_lemma: String,
        polarity: VerbPolarity,
    },
}

/// Происхождение факта или события. Используется для audit trail и для
/// принятия решений в конфликтах достоверности (см. SPEC §2.3).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Provenance {
    /// Извлечено Python SVO-парсером (высокая достоверность).
    SvoParser,
    /// Извлечено Rust regex-парсером (средняя достоверность).
    RustParser,
    /// Предложено LLM, не проверено reasoning engine.
    LlmSuggested,
    /// Проверено reasoning engine и принято как истина.
    Verified,
    /// Введено пользователем вручную (максимальный приоритет).
    User,
}

/// Событие нарратива: «кто, что, над кем, когда, откуда знаем».
///
/// События иммутабельны после записи в `FactLog` (за исключением `id`,
/// который назначается при записи). Они являются «источником истины» —
/// факты выводятся из событий через rules + inference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Назначается `FactLog::record_event`. Если 0 на входе — авто-присвоение.
    pub id: EventId,
    /// Кто действовал (EntityId == LitNode.id).
    pub actor: EntityId,
    /// Что сделал actor (см. [`Action`]).
    pub action: Action,
    /// На ком/чём выполнено действие (если есть).
    pub target: Option<EntityId>,
    /// Инструмент действия («нож», «оружие») — свободный текст.
    pub instrument: Option<String>,
    /// Когда произошло в нарративе.
    pub time: TemporalAnchor,
    /// Исходное предложение текста, из которого извлечено событие.
    pub source_text: String,
    /// Достоверность: 1.0 = из SVO Python, 0.5 = из LLM hypothesis и т.д.
    pub confidence: f32,
    /// Откуда пришло событие.
    pub provenance: Provenance,
}

/// Значение атрибута факта. Поддерживает примитивные типы, ссылку на
/// сущность, список и специальный `Unknown` для утраченных значений
/// (например, location мертвеца).
///
/// `PartialEq` реализован вручную: равенство только при совпадении тега
/// варианта И внутреннего значения. `Bool(true)` != `Int(1)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FactValue {
    /// Булево значение (alive = true/false).
    Bool(bool),
    /// Строка (location = "Замок").
    Str(String),
    /// Целое (age = 42).
    Int(i64),
    /// Дробное (height = 1.82).
    Float(f64),
    /// Ссылка на другую сущность (spouse = EntityId).
    #[serde(rename = "Entity", alias = "EntityRef")]
    EntityRef(EntityId),
    /// Список значений (knowledge = [fact1, fact2, ...]).
    List(Vec<FactValue>),
    /// Значение утеряно / неприменимо (location после смерти).
    Unknown,
}

impl PartialEq for FactValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (FactValue::Bool(a), FactValue::Bool(b)) => a == b,
            (FactValue::Str(a), FactValue::Str(b)) => a == b,
            (FactValue::Int(a), FactValue::Int(b)) => a == b,
            (FactValue::Float(a), FactValue::Float(b)) => a == b,
            (FactValue::EntityRef(a), FactValue::EntityRef(b)) => a == b,
            (FactValue::List(a), FactValue::List(b)) => a == b,
            (FactValue::Unknown, FactValue::Unknown) => true,
            // Разные теги вариантов — не равны, даже если «значения» похожи.
            // Bool(true) != Int(1), Float(2.0) != Int(2).
            _ => false,
        }
    }
}

/// Факт о сущности в narративе: «entity.attribute = value на интервале
/// [valid_from, valid_until)».
///
/// Факты выводятся из событий через rules.rs и inference.rs. Они
/// составляют «текущую правду» о мире; constraints.rs проверяет новые
/// события против фактов.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    /// Назначается `FactLog::assert_fact`. Если 0 на входе — авто-присвоение.
    pub id: FactId,
    /// О ком/чём факт (EntityId == LitNode.id).
    pub entity: EntityId,
    /// Какой атрибут («alive», «location», «knowledge», ...).
    pub attribute: String,
    /// Значение атрибута.
    pub value: FactValue,
    /// Какие события породили этот факт (audit trail).
    pub derived_from: Vec<EventId>,
    /// С какого момента нарратива факт валиден.
    pub valid_from: TemporalAnchor,
    /// До какого момента валиден. `None` = текущий (активный) факт.
    pub valid_until: Option<TemporalAnchor>,
    /// Происхождение факта.
    pub provenance: Provenance,
}

/// Внутреннее хранилище фактов и событий: append-only журнал с
/// монотонными ID-счётчиками.
///
/// См. SPEC §2.6. Хранит ВСЕ когда-либо записанные факты (включая
/// ретракнутые — для history) и ВСЕ события. Текущее состояние мира
/// вычисляется как «latest non-retracted fact per (entity, attribute)».
pub struct FactLog {
    facts: Vec<Fact>,
    next_id: FactId,
    events: Vec<Event>,
    next_event_id: EventId,
}

impl FactLog {
    /// Создать пустой журнал. Счётчики ID стартуют с 1 (0 зарезервирован
    /// как «auto-assign me»).
    pub fn new() -> Self {
        Self {
            facts: Vec::new(),
            next_id: 1,
            events: Vec::new(),
            next_event_id: 1,
        }
    }

    /// Записать событие в журнал.
    ///
    /// Если `event.id == 0` — присваивается следующий свободный ID и
    /// счётчик инкрементируется. Если ID уже задан (ненулевой) —
    /// используется как есть (caller несёт ответственность за уникальность).
    ///
    /// Возвращает ID, под которым событие было сохранено.
    pub fn record_event(&mut self, mut event: Event) -> EventId {
        if event.id == 0 {
            event.id = self.next_event_id;
            self.next_event_id = self.next_event_id.saturating_add(1);
        }
        let id = event.id;
        self.events.push(event);
        id
    }

    /// Утвердить факт (добавить в журнал).
    ///
    /// Аналогично [`Self::record_event`]: если `fact.id == 0`, ID
    /// присваивается автоматически. Факт сохраняется как активный
    /// (`valid_until = None`); для ретракции используйте [`Self::retract_fact`].
    pub fn assert_fact(&mut self, mut fact: Fact) -> FactId {
        if fact.id == 0 {
            fact.id = self.next_id;
            self.next_id = self.next_id.saturating_add(1);
        }
        let id = fact.id;
        self.facts.push(fact);
        id
    }

    /// Ретрактировать факт: установить `valid_until`, если он ещё активен.
    ///
    /// «Now» (временная метка ретракции) определяется как время последнего
    /// записанного события — это разумный прокси для текущего момента
    /// нарратива, т.к. `FactLog` сам по себе не хранит clock. Если событий
    /// пока нет, fallback — собственный `valid_from` факта (мгновенная
    /// ретракция).
    ///
    /// Возвращает:
    /// - `Some(anchor)` — если факт был найден и активен; anchor это
    ///   установленное `valid_until`.
    /// - `None` — если факт с таким ID не найден ИЛИ уже ретракнут.
    pub fn retract_fact(&mut self, fact_id: FactId) -> Option<TemporalAnchor> {
        // Ищем последний активный факт с этим ID (iter_mut + rev, чтобы
        // в случае коллизии ID взять самый свежий).
        let now_anchor = self.events.last().map(|e| e.time.clone());
        let fact = self
            .facts
            .iter_mut()
            .rev()
            .find(|f| f.id == fact_id && f.valid_until.is_none())?;
        let anchor = now_anchor.unwrap_or_else(|| fact.valid_from.clone());
        fact.valid_until = Some(anchor.clone());
        Some(anchor)
    }

    /// Все активные (не ретракнутые) факты для сущности.
    ///
    /// Возвращает ссылки на факты в порядке вставки (хронология утверждения).
    pub fn get_facts_for(&self, entity: &str) -> Vec<&Fact> {
        self.facts
            .iter()
            .filter(|f| f.entity == entity && f.valid_until.is_none())
            .collect()
    }

    /// Текущее значение атрибута для сущности: последний активный факт
    /// (по `valid_from`, при равенстве — последний вставленный) с
    /// matching entity+attribute.
    ///
    /// Возвращает `None`, если активного факта с таким атрибутом нет.
    pub fn get_current_value(&self, entity: &str, attr: &str) -> Option<&FactValue> {
        self.facts
            .iter()
            .filter(|f| f.entity == entity && f.attribute == attr && f.valid_until.is_none())
            .max_by(|a, b| {
                if a.valid_from.before(&b.valid_from) {
                    std::cmp::Ordering::Less
                } else if a.valid_from.after(&b.valid_from) {
                    std::cmp::Ordering::Greater
                } else {
                    // При равных valid_from последний вставленный должен
                    // победить — max_by у Rust возвращает последний элемент
                    // при равенстве, так что возвращаем Equal.
                    std::cmp::Ordering::Equal
                }
            })
            .map(|f| &f.value)
    }

    /// Все события, произошедшие в заданной главе (по `time.chapter_num`).
    pub fn get_events_in_chapter(&self, chapter: u32) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| e.time.chapter_num == chapter)
            .collect()
    }

    /// Все события, попавшие во временной интервал `[from, to]` включительно.
    ///
    /// Использует `TemporalAnchor::before` / `after` для сравнения.
    /// События на границах (== from, == to) включаются.
    pub fn events_between(&self, from: &TemporalAnchor, to: &TemporalAnchor) -> Vec<&Event> {
        self.events
            .iter()
            .filter(|e| !e.time.before(from) && !e.time.after(to))
            .collect()
    }

    /// Доступ ко всем записанным событиям (включая все ID/время/данные).
    pub fn all_events(&self) -> &[Event] {
        &self.events
    }

    /// Доступ ко всем фактам (включая ретракнутые — для history/audit).
    pub fn all_facts(&self) -> &[Fact] {
        &self.facts
    }
}

impl Default for FactLog {
    fn default() -> Self {
        Self::new()
    }
}

// ──────────────────────────────────────────────────────────────────────
// Тесты
// ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Хелпер: TemporalAnchor для главы (без суффикса/сцены/offset).
    fn anchor(chapter: u32) -> TemporalAnchor {
        TemporalAnchor {
            chapter_num: chapter,
            chapter_suffix: None,
            scene_index: None,
            char_offset: 0,
        }
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

    #[test]
    fn test_record_event_assigns_sequential_ids() {
        let mut log = FactLog::new();
        let e1 = dummy_event("alice", Action::Move { destination: "kitchen".into() }, anchor(1));
        let e2 = dummy_event("bob", Action::Speak { topic: None }, anchor(2));

        let id1 = log.record_event(e1);
        let id2 = log.record_event(e2);

        // ID должны быть последовательными: 1, 2, 3, ...
        assert_eq!(id1, 1, "первое событие должно получить id=1");
        assert_eq!(id2, 2, "второе событие должно получить id=2");

        // И ID должны быть записаны в сами события.
        let events = log.all_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].id, 1);
        assert_eq!(events[1].id, 2);
        assert_eq!(events[0].actor, "alice");
        assert_eq!(events[1].actor, "bob");

        // Запись с предзаданным ненулевым ID не должна перезаписывать его.
        let e3 = Event {
            id: 42,
            ..dummy_event("carol", Action::Arrive { destination: "garden".into() }, anchor(3))
        };
        let id3 = log.record_event(e3);
        assert_eq!(id3, 42, "предзаданный id должен сохраниться");
    }

    #[test]
    fn test_assert_and_retract_fact() {
        let mut log = FactLog::new();

        // Сначала запишем событие (чтобы у ретракции был «now» anchor).
        log.record_event(dummy_event(
            "alice",
            Action::Arrive { destination: "castle".into() },
            anchor(5),
        ));

        // Утверждаем факт: alice.alive = true с главы 1.
        let fact_id = log.assert_fact(dummy_fact(
            "alice",
            "alive",
            FactValue::Bool(true),
            anchor(1),
        ));
        assert_eq!(fact_id, 1, "первый факт должен получить id=1");

        // Факт активен и доступен через get_facts_for.
        let active = log.get_facts_for("alice");
        assert_eq!(active.len(), 1);
        assert!(active[0].valid_until.is_none());

        // Ретракция: должна вернуть Some(anchor) — время последнего события.
        let retracted_at = log.retract_fact(fact_id);
        assert!(retracted_at.is_some(), "ретракция активного факта должна вернуть Some");
        let anchor = retracted_at.unwrap();
        assert_eq!(anchor.chapter_num, 5, "now anchor должен быть временем последнего события");

        // После ретракции факт больше не возвращается get_facts_for.
        let still_active = log.get_facts_for("alice");
        assert!(
            still_active.is_empty(),
            "ретракнутый факт не должен возвращаться get_facts_for"
        );

        // Повторная ретракция того же ID возвращает None (уже не активен).
        let second = log.retract_fact(fact_id);
        assert!(second.is_none(), "повторная ретракция должна вернуть None");

        // Ретракция несуществующего ID тоже возвращает None.
        let missing = log.retract_fact(9999);
        assert!(missing.is_none(), "ретракция несуществующего факта должна вернуть None");
    }

    #[test]
    fn test_get_current_value_returns_latest() {
        let mut log = FactLog::new();

        // Утверждаем два факта для alice.location с разным valid_from.
        log.assert_fact(dummy_fact(
            "alice",
            "location",
            FactValue::Str("kitchen".into()),
            anchor(1),
        ));
        log.assert_fact(dummy_fact(
            "alice",
            "location",
            FactValue::Str("garden".into()),
            anchor(3),
        ));

        // Текущее значение — последнее (по valid_from).
        let current = log.get_current_value("alice", "location");
        assert!(current.is_some());
        match current.unwrap() {
            FactValue::Str(s) => assert_eq!(s, "garden", "должно вернуть значение более позднего факта"),
            other => panic!("ожидалось FactValue::Str, получено {:?}", other),
        }

        // Другой атрибут — None.
        assert!(
            log.get_current_value("alice", "alive").is_none(),
            "несуществующий атрибут должен дать None"
        );

        // Ретракция позднего факта откатывает текущее значение к раннему.
        let late_fact_id = log
            .all_facts()
            .iter()
            .find(|f| f.entity == "alice" && f.attribute == "location" && f.valid_from.chapter_num == 3)
            .map(|f| f.id)
            .expect("поздний факт должен быть в журнале");
        log.retract_fact(late_fact_id);

        let rolled_back = log.get_current_value("alice", "location");
        assert!(rolled_back.is_some());
        match rolled_back.unwrap() {
            FactValue::Str(s) => assert_eq!(s, "kitchen", "после ретракции должно вернуться предыдущее значение"),
            other => panic!("ожидалось FactValue::Str, получено {:?}", other),
        }
    }

    #[test]
    fn test_get_events_in_chapter() {
        let mut log = FactLog::new();
        log.record_event(dummy_event("alice", Action::Arrive { destination: "x".into() }, anchor(1)));
        log.record_event(dummy_event("bob", Action::Speak { topic: None }, anchor(2)));
        log.record_event(dummy_event("carol", Action::Leave { source: "x".into() }, anchor(1)));
        log.record_event(dummy_event("dave", Action::Die, anchor(3)));

        let ch1 = log.get_events_in_chapter(1);
        assert_eq!(ch1.len(), 2, "в главе 1 должно быть 2 события");
        let ch1_actors: Vec<&str> = ch1.iter().map(|e| e.actor.as_str()).collect();
        assert!(ch1_actors.contains(&"alice"));
        assert!(ch1_actors.contains(&"carol"));

        let ch2 = log.get_events_in_chapter(2);
        assert_eq!(ch2.len(), 1);
        assert_eq!(ch2[0].actor, "bob");

        let ch3 = log.get_events_in_chapter(3);
        assert_eq!(ch3.len(), 1);
        assert_eq!(ch3[0].actor, "dave");

        let ch_empty = log.get_events_in_chapter(99);
        assert!(ch_empty.is_empty(), "несуществующая глава должна дать пустой список");
    }

    #[test]
    fn test_fact_value_partial_eq() {
        // Одинаковые теги + одинаковые значения → равны.
        assert_eq!(FactValue::Bool(true), FactValue::Bool(true));
        assert_eq!(FactValue::Bool(false), FactValue::Bool(false));
        assert_eq!(FactValue::Str("x".into()), FactValue::Str("x".into()));
        assert_eq!(FactValue::Int(5), FactValue::Int(5));
        assert_eq!(FactValue::Float(3.14), FactValue::Float(3.14));
        assert_eq!(FactValue::EntityRef("alice".into()), FactValue::EntityRef("alice".into()));
        assert_eq!(FactValue::Unknown, FactValue::Unknown);
        assert_eq!(
            FactValue::List(vec![FactValue::Int(1), FactValue::Str("a".into())]),
            FactValue::List(vec![FactValue::Int(1), FactValue::Str("a".into())])
        );

        // Одинаковые теги, разные значения → не равны.
        assert_ne!(FactValue::Bool(true), FactValue::Bool(false));
        assert_ne!(FactValue::Str("a".into()), FactValue::Str("b".into()));
        assert_ne!(FactValue::Int(1), FactValue::Int(2));
        assert_ne!(FactValue::EntityRef("a".into()), FactValue::EntityRef("b".into()));
        assert_ne!(
            FactValue::List(vec![FactValue::Int(1)]),
            FactValue::List(vec![FactValue::Int(2)])
        );

        // Разные теги — не равны, даже если «значения» похожи.
        assert_ne!(FactValue::Bool(true), FactValue::Int(1));
        assert_ne!(FactValue::Int(2), FactValue::Float(2.0));
        assert_ne!(FactValue::Str("5".into()), FactValue::Int(5));
        assert_ne!(FactValue::Unknown, FactValue::Bool(false));
        assert_ne!(
            FactValue::EntityRef("x".into()),
            FactValue::Str("x".into())
        );
    }

    /// Дополнительный smoke-тест для events_between — покрывает
    /// инклюзивность границ и использование before/after.
    #[test]
    fn test_events_between_inclusive() {
        let mut log = FactLog::new();
        log.record_event(dummy_event("a", Action::Die, anchor(1)));
        log.record_event(dummy_event("b", Action::Die, anchor(2)));
        log.record_event(dummy_event("c", Action::Die, anchor(3)));
        log.record_event(dummy_event("d", Action::Die, anchor(4)));

        let between = log.events_between(&anchor(2), &anchor(3));
        assert_eq!(
            between.len(),
            2,
            "events_between должен быть инклюзивным на обеих границах"
        );
        let actors: Vec<&str> = between.iter().map(|e| e.actor.as_str()).collect();
        assert!(actors.contains(&"b"));
        assert!(actors.contains(&"c"));

        // Диапазон шире — все 4 события.
        let all = log.events_between(&anchor(1), &anchor(4));
        assert_eq!(all.len(), 4);

        // Пустой диапазон.
        let none = log.events_between(&anchor(10), &anchor(20));
        assert!(none.is_empty());
    }

    /// Smoke-тест: FactLog реализует Default.
    #[test]
    fn test_fact_log_default() {
        let log = FactLog::default();
        assert!(log.all_events().is_empty());
        assert!(log.all_facts().is_empty());
    }
}
