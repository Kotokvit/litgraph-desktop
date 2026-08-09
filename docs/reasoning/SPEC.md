# LitGraph Reasoning Engine — Specification v0.1

> **Этот документ — обязательный контракт для ВСЕХ субагентов**, работающих над
> модулем `src-tauri/src/reasoning/`. Любой код, не соответствующий этой спеке,
> будет отправлен на переделку без обсуждений.
>
> Архитектурный принцип: **понимание — это свойство алгоритма, а не LLM.**
> LLM — речевой генератор, подчиняющийся WorldState. Алгоритм никогда не
> спрашивает LLM «что является истиной?» — он либо принимает факт от LLM как
> hypothesis (которую потом проверяет), либо отвергает её за нарушение
> ограничений.

---

## 0. Главные принципы

1. **State is truth.** `WorldState` — единственный источник истины. Любой
   вывод (character dead, location changed, knowledge gained) живёт в
   состоянии, а не в тексте.
2. **LLM ≠ oracle.** LLM может предлагать гипотезы и писать текст, но **никогда**
   не утверждает факты напрямую. Все её текстовые утверждения проходят через
   semantic parser → facts → constraints → commit/reject.
3. **No implicit conversions.** Все переходы между слоями (text → events →
   facts → state transitions) — явные, типизированные, логируемые.
4. **Determinism first.** Если алгоритм может вывести факт без LLM — он обязан
   это сделать. LLM подключается только когда правила исчерпаны.
5. **Module independence.** Каждый модуль компилируется отдельно. Зависимости
   только через `use crate::reasoning::...` по заранее согласованному API.
6. **Russian-first UI strings** (тексты в `Display`/`Debug`/сообщениях для
   пользователя — на русском). Идентификаторы, поля, имена функций — английские.

---

## 1. Карта модулей

```
src-tauri/src/reasoning/
├── mod.rs              — публичный入口, ReasoningEngine, run_cycle()
├── facts.rs            — Fact, Event, FactId, FactLog
├── state.rs            — WorldState, EntityState, attribute store
├── rules.rs            — Rule, RuleSet, verb→effect mapping
├── inference.rs        — forward-chaining inference engine
├── causality.rs        — cause-edge propagation (A→B→C)
├── timeline.rs         — TemporalAnchor, ordering, intervals
├── constraints.rs      — Constraint, ConstraintEngine
├── contradictions.rs   — ContradictionDetector, ContradictionReport
├── semantic_parser.rs  — text → events via SVO + verb lexicon
├── memory.rs           — KnowledgeBase, subgraph retrieval
├── hypotheses.rs       — Hypothesis, HypothesisGenerator, verifier
├── planner.rs          — ActionPlan, decision: which op next
├── cycle.rs            — ReasoningCycle orchestration
└── llm_bridge.rs       — LLM-as-generator bridge with state enforcement
```

---

## 2. Базовые типы (контракт для всех модулей)

### 2.1. IDs

```rust
// facts.rs
pub type FactId = u64;           // monotonic counter inside FactLog
pub type EventId = u64;

// state.rs
pub type EntityId = String;       // == LitNode.id (reuse graph node IDs)
pub type Attribute = String;      // "alive", "location", "knowledge", ...
```

**ВАЖНО:** `EntityId = String` и совпадает с `LitNode.id`. Это позволяет
reasoning engine работать поверх существующего графа без копирования.

### 2.2. TemporalAnchor (timeline.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TemporalAnchor {
    pub chapter_num: u32,         // Глава 12, 15, 28б → numeric part
    pub chapter_suffix: Option<String>, // "б" для суб-глав
    pub scene_index: Option<u32>, // индекс сцены внутри главы
    pub char_offset: usize,       // байтовое смещение в исходном тексте
}

impl TemporalAnchor {
    pub fn before(&self, other: &TemporalAnchor) -> bool { /* ... */ }
    pub fn after(&self, other: &TemporalAnchor) -> bool { /* ... */ }
    pub fn same_chapter(&self, other: &TemporalAnchor) -> bool { /* ... */ }
}
```

Порядок: chapter_num → chapter_suffix (lex) → scene_index → char_offset.

### 2.3. Event (facts.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub actor: EntityId,            // кто действовал
    pub action: Action,             // что сделал (см. 2.4)
    pub target: Option<EntityId>,   // на ком/чём
    pub instrument: Option<String>, // "нож", "оружие" — свободный текст
    pub time: TemporalAnchor,
    pub source_text: String,        // исходное предложение
    pub confidence: f32,            // 0.0..=1.0 (1.0 = из SVO Python, 0.5 = из LLM hypothesis)
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Provenance {
    /// Извлечено Python SVO-парсером (высокая достоверность)
    SvoParser,
    /// Извлечено Rust regex-парсером (средняя достоверность)
    RustParser,
    /// Предложено LLM, не проверено
    LlmSuggested,
    /// Проверено reasoning engine'ом и принято
    Verified,
    /// Введено пользователем вручную
    User,
}
```

### 2.4. Action (facts.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Action {
    /// Физическое действие над целью
    Kill,
    Wound,
    Hit,
    Capture,
    Imprison,
    Free,
    Heal,
    Touch,
    /// Перемещение
    Move { destination: String },
    Arrive { destination: String },
    Leave { source: String },
    /// Коммуникация
    Speak { topic: Option<String> },
    Ask { topic: String },
    Tell { topic: String, to: EntityId },
    /// Социальные отношения
    Marry { partner: EntityId },
    Betray { victim: EntityId },
    Ally { partner: EntityId },
    /// Когнитивные
    Know { fact: String },
    Forget { fact: String },
    Want { goal: String },
    Plan { goal: String },
    /// Эмоциональные
    FallInLove { partner: EntityId },
    Hate { target: EntityId },
    /// Мета-действия (не физические, но сюжетно важные)
    Discover { fact: String },
    Transform { new_form: String },
    Die,
    Resurrect,
    /// Кастомное действие (verb не из лексикона)
    Custom { verb_lemma: String, polarity: VerbPolarity },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum VerbPolarity {
    Positive,
    Negative,
    Neutral,
}
```

### 2.5. Fact (facts.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub id: FactId,
    pub entity: EntityId,
    pub attribute: Attribute,        // "alive", "location", "knowledge", ...
    pub value: FactValue,
    pub derived_from: Vec<EventId>,  // какие события породили факт
    pub valid_from: TemporalAnchor,
    pub valid_until: Option<TemporalAnchor>, // None = current
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FactValue {
    Bool(bool),
    Str(String),
    Int(i64),
    Float(f64),
    EntityRef(EntityId),
    List(Vec<FactValue>),
    /// "unknown" — факт, который был установлен, но значение потеряно
    /// (например, после смерти — location становится Unknown)
    Unknown,
}
```

### 2.6. FactLog (facts.rs)

```rust
pub struct FactLog {
    facts: Vec<Fact>,
    next_id: FactId,
    events: Vec<Event>,
    next_event_id: EventId,
}

impl FactLog {
    pub fn new() -> Self;
    pub fn record_event(&mut self, event: Event) -> EventId;
    pub fn assert_fact(&mut self, fact: Fact) -> FactId;
    pub fn retract_fact(&mut self, fact_id: FactId) -> Option<TemporalAnchor>;
    pub fn get_facts_for(&self, entity: &EntityId) -> Vec<&Fact>;
    pub fn get_current_value(&self, entity: &EntityId, attr: &Attribute) -> Option<&FactValue>;
    pub fn get_events_in_chapter(&self, chapter: u32) -> Vec<&Event>;
    pub fn events_between(&self, from: &TemporalAnchor, to: &TemporalAnchor) -> Vec<&Event>;
}
```

### 2.7. WorldState (state.rs)

```rust
pub struct WorldState {
    /// Текущее состояние каждой сущности: entity_id → attribute → value
    current: HashMap<EntityId, HashMap<Attribute, FactValue>>,
    /// История изменений (для отката и audit trail)
    history: Vec<StateTransition>,
    /// Текущий момент времени в нарративе
    now: TemporalAnchor,
}

impl WorldState {
    pub fn new() -> Self;
    pub fn get(&self, entity: &EntityId, attr: &Attribute) -> Option<&FactValue>;
    pub fn set(&mut self, entity: &EntityId, attr: Attribute, value: FactValue, transition: StateTransition);
    pub fn advance_to(&mut self, anchor: &TemporalAnchor);
    pub fn now(&self) -> &TemporalAnchor;
    pub fn snapshot(&self) -> WorldSnapshot;
    pub fn restore(&mut self, snapshot: WorldSnapshot);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub entity: EntityId,
    pub attribute: Attribute,
    pub old_value: Option<FactValue>,
    pub new_value: FactValue,
    pub caused_by_event: Option<EventId>,
    pub at: TemporalAnchor,
}
```

### 2.8. Rule (rules.rs)

```rust
/// Правило: «если произошло событие action → применить effect к WorldState»
#[derive(Debug, Clone)]
pub struct Rule {
    pub name: &'static str,
    pub matches: Action,           // чему соответствует (Custom или конкретный variant)
    pub effects: Vec<RuleEffect>,
    pub preconditions: Vec<Precondition>,
}

#[derive(Debug, Clone)]
pub enum RuleEffect {
    SetAttribute { entity: RuleEntity, attribute: Attribute, value: FactValue },
    SetAttributeFromEvent { entity: RuleEntity, attribute: Attribute, source: EventField },
    AppendToList { entity: RuleEntity, attribute: Attribute, value: FactValue },
    InvalidateAttribute { entity: RuleEntity, attribute: Attribute },
    /// Создаёт новый fact: target получил knowledge о событии
    RecordKnowledge { knower: RuleEntity, about_event: bool },
}

#[derive(Debug, Clone)]
pub enum RuleEntity {
    Actor,
    Target,
    /// Конкретная сущность по ID
    Specific(EntityId),
}

#[derive(Debug, Clone)]
pub enum EventField {
    Actor,
    Target,
    Instrument,
    SourceText,
}

#[derive(Debug, Clone)]
pub struct Precondition {
    pub entity: RuleEntity,
    pub attribute: Attribute,
    pub expected: FactValue,
}

pub struct RuleSet {
    rules: Vec<Rule>,
}

impl RuleSet {
    pub fn default_literary() -> Self;  // базовый набор для литературных текстов
    pub fn find_matching(&self, action: &Action) -> Vec<&Rule>;
    pub fn add(&mut self, rule: Rule);
}
```

### 2.9. Constraint (constraints.rs)

```rust
#[derive(Debug, Clone)]
pub struct Constraint {
    pub name: &'static str,
    /// Если у сущности attribute == expected — действие action невозможно
    pub when: ConstraintCondition,
    pub forbids: Action,
    pub reason: String,             // "Невозможно: персонаж мёртв с Главы N"
}

#[derive(Debug, Clone)]
pub struct ConstraintCondition {
    pub attribute: Attribute,
    pub equals: FactValue,
}

pub struct ConstraintEngine {
    constraints: Vec<Constraint>,
}

impl ConstraintEngine {
    pub fn default_literary() -> Self;
    pub fn check(&self, state: &WorldState, event: &Event) -> Vec<ConstraintViolation>;
    pub fn add(&mut self, c: Constraint);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstraintViolation {
    pub constraint_name: String,
    pub event_id: EventId,
    pub actor: EntityId,
    pub attempted_action: Action,
    pub reason: String,
    pub conflicting_fact: Option<FactId>,
    pub at: TemporalAnchor,
}
```

### 2.10. ContradictionReport (contradictions.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionReport {
    pub violations: Vec<ConstraintViolation>,
    pub temporal_paradoxes: Vec<TemporalParadox>,
    pub causal_loops: Vec<CausalLoop>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalParadox {
    pub description: String,
    pub earlier_fact: FactId,
    pub later_event: EventId,
    pub earlier_at: TemporalAnchor,
    pub later_at: TemporalAnchor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalLoop {
    pub description: String,
    pub chain: Vec<EventId>,    // A → B → C → A
}
```

### 2.11. Hypothesis (hypotheses.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hypothesis {
    pub id: HypothesisId,
    pub statement: String,         // "Пётр каким-то образом выжил после Г12"
    pub proposed_resolution: Option<Resolution>,
    pub evidence_for: Vec<FactId>,
    pub evidence_against: Vec<FactId>,
    pub status: HypothesisStatus,
    pub source: HypothesisSource,
}

pub type HypothesisId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HypothesisStatus {
    Pending,
    Accepted,
    Rejected(String),  // reason
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HypothesisSource {
    Algorithm,
    Llm,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Resolution {
    MarkEventAs { event_id: EventId, kind: EventKind },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    Canonical,       // реальное событие в нарративе
    Flashback,       // воспоминание
    Dream,           // сон / галлюцинация
    Vision,          // видение
    StoryWithinStory,// рассказ в рассказе
}
```

### 2.12. ReasoningCycle (cycle.rs)

```rust
pub struct ReasoningCycle {
    pub world: WorldState,
    pub facts: FactLog,
    pub rules: RuleSet,
    pub constraints: ConstraintEngine,
    pub memory: KnowledgeBase,
    pub hypotheses: Vec<Hypothesis>,
    pub llm: Option<LlmBridge>,
}

impl ReasoningCycle {
    pub fn new() -> Self;
    pub fn observe(&mut self, events: Vec<Event>);
    pub fn build_state(&mut self);
    pub fn reason(&mut self) -> Vec<ConstraintViolation>;
    pub fn generate_hypotheses(&mut self, violations: &[ConstraintViolation]) -> Vec<HypothesisId>;
    pub fn verify(&mut self, hyp_id: HypothesisId) -> HypothesisStatus;
    pub fn update_state(&mut self, accepted: &[HypothesisId]);
    pub fn run_cycle(&mut self, events: Vec<Event>) -> CycleReport;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CycleReport {
    pub events_processed: usize,
    pub facts_asserted: usize,
    pub violations: Vec<ConstraintViolation>,
    pub temporal_paradoxes: Vec<TemporalParadox>,
    pub hypotheses_generated: usize,
    pub hypotheses_accepted: usize,
    pub final_state_snapshot: WorldSnapshot,
}
```

---

## 3. Интеграционный boundary с существующим кодом

### 3.1. Откуда берутся входные данные

```text
parse_md_full.rs  →  ParseResult { nodes, edges, stats }
                              │
                              ▼
ner.rs           →  NerResult { entities, stats, model, ... }
                              │
                              ▼
svo_extract.py   →  triplets [{ subject, subjectLemma, verb, verbLemma,
                                object, objectLemma, sentence, position,
                                tense, polarity, negated, ... }]
                              │
                              ▼
conflict.rs      →  ConflictGraph { nodes, edges, matrix, ... }
                              │
                              ▼
              ┌───────────────────────────────┐
              │   reasoning::ReasoningEngine  │
              └───────────────────────────────┘
                              │
                              ▼
                  WorldState + FactLog + ContradictionReport
```

### 3.2. Точки вызова

- **После `parse_md_full`** — reasoning engine инициализируется из `ParseResult`
  (узлы становятся `EntityId`, chapters дают `TemporalAnchor`).
- **После `extract_svo`** — triplets конвертируются в `Event` через
  `semantic_parser::triplets_to_events()`.
- **После `get_conflict_graph`** — conflict edges маппятся на `Action::Hit/Wound/...`.
- **В `ai/prompts.rs`** — `build_assistant_prompt` теперь опционально принимает
  `&ReasoningEngine`, чтобы подставлять в промпт вместо "статистики" реальные
  facts и constraints.

### 3.3. LLM bridge

`llm_bridge.rs` принимает `Action::Write*` запросы и формирует prompt:

```text
ACTION: write_scene
WORLD STATE (relevant subset):
  - Пётр.alive = false (since Глава 12)
  - Пётр.location = unknown
  - Иван.alive = true
  - Иван.location = Замок
CONSTRAINTS:
  - Пётр не может: speak, move, appear_alive
  - Разрешено: flashback, dream, mention, body
TASK: <user's request>
```

LLM генерирует текст. Текст возвращается в `semantic_parser::parse_text()`,
извлечённые события проходят через `constraints.check()`. Если LLM написала
«Пётр сказал...» — получаем `ConstraintViolation`, сцена **отвергается**, LLM
получает feedback и должна сгенерировать заново (с retry-лимитом).

---

## 4. Что каждый модуль ДОЛЖЕН экспортировать

Каждый модуль обязан иметь:
1. `pub struct ...` / `pub enum ...` для всех публичных типов
2. `#[derive(Debug, Clone, Serialize, Deserialize)]` для всех типов данных
3. `impl Default` где это уместно
4. Юнит-тесты `#[cfg(test)] mod tests { ... }` — минимум 3 теста на модуль
5. Заголовок `//! ...` с описанием модуля и принципом работы
6. **Никаких `pub use` из других reasoning-модулей** в `lib.rs` стиле —
   только `use crate::reasoning::facts::Fact` etc. Это предотвращает
   циклические зависимости.

---

## 5. Анти-паттерны (ЗАПРЕЩЕНО)

1. ❌ Вызывать LLM из модулей кроме `llm_bridge.rs`.
2. ❌ Использовать `String` как тип для attribute/action — только `Attribute`/`Action`.
3. ❌ Мутабельные глобальные переменные (`static mut`).
4. ❌ `unwrap()` / `expect()` на данных, пришедших из Python SVO (только `?` с fallback).
5. ❌ Создавать новые сущности в reasoning engine — только enrich существующих `LitNode.id`.
6. ❌ Текст на английском в `Display`/`Debug` выводе для пользователя.
7. ❌ Зависимости от `tokio` / async — reasoning engine полностью синхронный.
   LLM-вызовы обёртываются в `tokio::task::spawn_blocking` на уровне Tauri command.

---

## 6. План сборки (для координатора)

- **Wave 1 (data layer, параллельно):** `facts.rs`, `state.rs`, `timeline.rs`, `rules.rs`
- **Wave 2 (logic layer, параллельно):** `inference.rs`, `causality.rs`, `constraints.rs`, `contradictions.rs`
- **Wave 3 (semantic layer, параллельно):** `semantic_parser.rs`, `memory.rs`
- **Wave 4 (orchestration):** `cycle.rs`, `hypotheses.rs`, `planner.rs`, `llm_bridge.rs`
- **Wave 5 (integration):** `mod.rs` + integration-тесты

Каждую волну координатор:
1. Дискатчит субагентов с этим SPEC.md и конкретным module brief.
2. Каждый субагент пишет только свой файл.
3. Координатор проверяет компиляцию, фиксит integration gaps.
4. Переходит к следующей волне.
