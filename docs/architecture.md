# LitGraph — Каноническая архитектура

> **Этот документ — единственный источник истины для архитектуры LitGraph.**
> Все изменения в схеме слоёв, математической модели, эпистемических типах и
> контрактах между модулями обязаны быть сначала отражены здесь, и только потом
> в коде.
>
> Документ-спутник для Reasoning Engine: [`docs/reasoning/SPEC.md`](reasoning/SPEC.md).
> Документ-спутник для математического ядра: [`docs/poler_math/POLER_SPEC.md`](poler_math/POLER_SPEC.md).
>
> Версия: 1.0.0 · Дата: 2026-08-10 · Статус: **canonical**.

---

## 0. Архитектурное правило проекта

> **Нейросети разрешены на восприятии. Нейросети запрещено определять истину
> World Model. LLM запрещено напрямую изменять World Model. Только Reasoning
> Engine имеет право коммитить состояние.**

Это правило — инвариант всего проекта. Любой код, нарушающий его, считается
дефектом. Если завтра полностью выкинуть LLM, система обязана уметь:

- читать текст,
- извлекать события,
- строить мир,
- выводить последствия,
- обнаруживать противоречия,
- проверять гипотезы.

LLM лишь добавляет способность генерировать естественный язык — и только в
замкнутом контуре валидации.

---

## 1. Принципы

1. **State is truth.** `WorldState` — единственный источник истины. Любой вывод
   (персонаж мёртв, локация изменена, знание получено) живёт в состоянии, а не
   в тексте.
2. **Neural = sensor, not oracle.** Нейросеть в NLP-слое — извлекатель
   признаков с `confidence ∈ [0, 1]`. World Model не обязана ей верить.
3. **Two-level semantics.** Committed facts имеют булеву семантику (`{0, 1}`),
   гипотезы — вероятностную (`[0, 1]`). Reasoning над committed = SAT (Z3),
   над гипотезами = Bayesian.
4. **No implicit conversions.** Все переходы между слоями — явные,
   типизированные, логируемые.
5. **Provenance is fundamental.** Каждый факт и событие несёт структурированный
   provenance с `derived_from`. Цепочка доказательства восстанавливается за
   `O(длина цепи)`.
6. **Determinism first.** Если алгоритм может вывести факт без LLM — он обязан
   это сделать. LLM подключается только когда правила исчерпаны.
7. **Budget-bound loops.** Любой цикл с участием LLM имеет явный бюджет
   `B ∈ ℕ` с декрементом и эскалацией при `B = 0`. Гарантия терминации.
8. **Python = Research Lab, не runtime.** Python используется только для
   прототипирования и калибровки алгоритмов. В продакшн-рантайме его нет.

---

## 2. Схема слоёв

Реальные вычислительные слои — четыре. Между L1 (NLP) и L2 (Event) находится
контракт Semantic IR. Planner и LLM Context Builder — отдельные модули между
Reasoning и LLM. Validator замыкает контур.

```
                         TEXT
                          │
                          ▼
                ┌─────────────────────────┐
                │   NLP LAYER  (L1)       │
                │                         │
                │ tokenizer               │
                │ morphology              │
                │ NER                     │
                │ coreference             │
                │ relations               │
                │ dependency              │
                │                         │
                │ backend:                │
                │  rule / statistical /   │
                │  neural (ONNX)          │
                └───────────┬─────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │  SEMANTIC IR  (L1.5)    │
                │                         │
                │ mentions                │
                │ predicates              │
                │ roles                   │
                │ polarities              │
                │ spans                   │
                │ confidence              │
                │                         │
                │ + normalization ν       │
                │ + negation handling     │
                │ + role regrouping       │
                │ + word-sense disambig.  │
                └───────────┬─────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │  EVENT LAYER  (L2)      │
                │                         │
                │ events                  │
                │ participants            │
                │ temporal                │
                │ location                │
                │ provenance (rich)       │
                └───────────┬─────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │  WORLD MODEL  (L3)      │
                │                         │
                │ Facts (typed)           │
                │ States (time-indexed)   │
                │ Relations               │
                │ Causality               │
                │ Timeline (DAG)          │
                │ Memory                  │
                │                         │
                │ epistemic: FACT /       │
                │   OBSERVATION /         │
                │   HYPOTHESIS /          │
                │   ASSUMPTION            │
                └───────────┬─────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │  REASONING ENGINE (L4)  │
                │                         │
                │ Inference               │
                │ Constraints (fast)      │
                │   → Rust rules          │
                │ Constraints (deep)      │
                │   → Z3 SMT              │
                │ Contradiction           │
                │   (relation +           │
                │    unsat core)          │
                │ Hypotheses lifecycle    │
                └───────────┬─────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │        PLANNER          │
                │                         │
                │ Task Specification:     │
                │  - gap / contradiction  │
                │  - target state         │
                │  - invariants           │
                │  - budget B             │
                └───────────┬─────────────┘
                            │
                            ▼
                ┌─────────────────────────┐
                │  LLM CONTEXT BUILDER    │
                │                         │
                │ WorldState slice        │
                │ Neighbor text           │
                │ Style / voice           │
                │ Goal                    │
                └───────────┬─────────────┘
                            │
                            ▼
                         LLM (write)
                            │
                            ▼
                    candidate text
                            │
                            ▼
                NLP → IR → Event
                            │
                            ▼
                     WorldState Δ
                            │
                            ▼
                ┌─────────────────────────┐
                │       VALIDATOR         │
                │                         │
                │ Consistent?             │
                │ Resolves gap?           │
                │ Provenance OK?          │
                │ Invariants kept?        │
                └───────┬─────────────────┘
                        │
                ┌───────┴───────┐
                ▼               ▼
             ACCEPT          REJECT
                │               │
                ▼               ▼
            commit       diagnostics
                              │
                              ▼
                          rewrite
                          (B := B − 1)
                          (B = 0 → escalate)
```

### 2.1. Назначение слоёв

| Слой | Имя | Назначение |
|------|-----|------------|
| L1 | NLP | Извлечение поверхностных признаков. Бекенды: rule, statistical, ONNX. |
| L1.5 | Semantic IR | Контракт L1→L2. Нормализация предикатов, полярность, ролевая перегруппировка, разрешение многозначности. |
| L2 | Event | Коммит семантических событий с participants, time, location, provenance. |
| L3 | World Model | Факты, состояния, отношения, причинность, timeline (DAG), memory. |
| L4 | Reasoning | Inference, constraints (fast+deep), contradiction, hypotheses lifecycle. |
| — | Planner | Task Specification для LLM. |
| — | LLM Context Builder | Сборка контекста: WorldState slice + neighbor text + style. |
| — | Validator | Проверка `ΔW` перед коммитом. |

---

## 3. Математика

### 3.1. Базовые типы

| Обозначение | Смысл |
|-------------|-------|
| $\Sigma^*$ | Все тексты над алфавитом $\Sigma$ |
| $T \in \Sigma^*$ | Входной текст |
| $\text{Span}(T) = \{[i, j) \mid 0 \leq i \leq j \leq |T|\}$ | Интервалы в тексте |
| $\mathcal{E}$ | Множество ID сущностей (счётное) |
| $\mathcal{P}$ | Канонические предикаты (`KILL`, `ENTER`, `ALIVE`, `KNOW`, `LOCATED`, ...) |
| $\Pi = \{+, -\}$ | Полярность |
| $\mathcal{R}$ | Типы отношений (`CAUSES`, `BEFORE`, `SUBEVENT_OF`, `CO-OCCURS`) |
| $\mathcal{G} = \mathbb{N}$ | Generation markers (chapter / scene) |
| $\mathcal{T} = \mathbb{R}_{\geq 0}$ | Время (потенциально непрерывное, на практике дискретное) |
| $\mathcal{K} = \{\text{FACT}, \text{OBSERVATION}, \text{HYPOTHESIS}, \text{ASSUMPTION}\}$ | Эпистемические типы |

### 3.2. NLP Layer (L1)

NLP — (возможно, стохастическая) функция:

$$\text{NLP}: \Sigma^* \to \mathcal{P}(\text{Mentions} \times \text{Relations} \times \text{Syntax} \times \text{Coref})$$

Для нейронного бекенда $\text{NLP}(T)$ — распределение, берём моду с confidence.

**Ключевое свойство**: NLP дозволено ошибаться. Её выход — *свидетельство*, не истина.

### 3.3. Semantic IR (L1.5)

Преобразование "лемма → предикат":

$$\nu: \text{Lemma} \times \text{Context} \rightharpoonup \mathcal{P}(\mathcal{P} \times \Pi)$$

$\nu$ — частичная функция (не у всех лемм есть канонический предикат). Для
неоднозначных — множество кандидатов с весами.

**Event candidate** (выход L1.5, вход L2):

$$c = (p \in \mathcal{P},\; \pi \in \Pi,\; \text{args}: \mathcal{E}^*,\; \text{span},\; t,\; \text{loc},\; \text{conf} \in [0, 1])$$

### 3.4. Event Layer (L2)

Event — committed candidate (прошёл локальную проверку):

$$e = (\text{EID},\; p,\; \pi,\; \text{args},\; \text{span},\; t,\; \text{loc},\; \text{prov})$$

**Provenance** (структурированный):

$$\text{Prov} = (\text{doc\_id},\; \text{span},\; \text{sent\_id},\; \text{para\_id},\; \text{method} \in \{\text{Direct}, \text{Inferred}\},\; \text{conf},\; \text{derived\_from}: [\text{FID}])$$

### 3.5. World Model (L3)

Состояние мира на generation $g$:

$$W_g = (F_g,\; S_g,\; R_g,\; C_g,\; \text{TL}_g,\; M_g)$$

| Компонент | Смысл |
|-----------|-------|
| $F_g$ | Факты |
| $S_g$ | Состояния (time-indexed predicates) |
| $R_g$ | Отношения между фактами / событиями |
| $C_g$ | Причинный граф |
| $\text{TL}_g$ | Timeline (DAG, с ветвями для гипотез) |
| $M_g$ | Memory |

**Fact**:

$$f = (\text{FID},\; p,\; \pi,\; \text{args},\; \tau \in \mathcal{K},\; \text{conf} \in [0, 1],\; \text{prov},\; \text{hyp\_id}?)$$

**State function** для сущности $e$ и предиката $p$:

$$\llbracket p \rrbracket(e, t) \in [0, 1]$$

**Двухуровневая семантика** (канонический выбор LitGraph):

- Для $\tau \in \{\text{FACT}, \text{OBSERVATION}\}$: $\llbracket p \rrbracket(e, t) \in \{0, 1\}$ (committed, deterministic).
- Для $\tau \in \{\text{HYPOTHESIS}, \text{ASSUMPTION}\}$: $\llbracket p \rrbracket(e, t) \in [0, 1]$ (probabilistic).

Reasoning над committed facts = SAT (Z3), над hypotheses = Bayesian update.
Это даёт чистый вывод для committed и graceful degradation для неопределённости.

#### 3.5.1. Branching Timeline (DAG)

$\text{TL}_g$ — это **DAG**, а не линейная структура:

- Корень — каноническая история.
- Для каждой активной гипотезы $H_i$ создаётся ветка, помеченная `hypothesis_id = i`.
- При подтверждении $H_i$ (см. §3.7) — merge в основную ветку.
- При опровержении — удаление ветки.

Это позволяет контрфактический вывод: "если бы $H$ было true, как бы выглядел timeline?"

### 3.6. Reasoning Engine (L4)

#### 3.6.1. Inference

**Схема правила вывода**:

$$\frac{f_1, \ldots, f_n \;\text{with confs}\; c_1, \ldots, c_n}{f_{n+1} \;\texttext{with conf}\; \kappa(c_1, \ldots, c_n) \cdot \rho(\text{rule})}$$

где $\rho(\text{rule}) \in [0, 1]$ — надёжность правила (калибруется в Python Lab).

**Функции $\kappa$** (выбирается per-rule):

| Имя | Формула | Семантика |
|-----|---------|-----------|
| $\kappa_{\min}$ | $\min(c_1, \ldots, c_n)$ | Пессимистичная (default) |
| $\kappa_{\text{prod}}$ | $\prod_{i=1}^{n} c_i$ | Независимые свидетельства |
| $\kappa_{\text{noisy-OR}}$ | $1 - \prod_{i=1}^{n}(1 - c_i)$ | Избыточные свидетельства |

#### 3.6.2. Constraint Engine — два уровня

**Fast (Rust rules)** — $O(|W_g|)$ проверки. Пример:

$$\text{dead}(e, t) \Rightarrow \neg\text{speak}(e, t') \quad \forall t' > t$$

**Deep (Z3 SMT)** — кодирование committed facts в булевы переменные, constraints
в клазулы:

$$\text{Z3-check}(W_g) \in \{\text{SAT}, \text{UNSAT}, \text{UNKNOWN}\} \times \text{UnsatCore?}$$

Если UNSAT — `Contradiction` создаётся со ссылкой на unsat core (минимальное
множество фактов, вызывающих конфликт). Это и есть explainability.

> **Z3 НЕ заменяет ConstraintEngine.** ConstraintEngine понимает предметную
> модель (`dead(Peter)`, `alive(Peter)`, `knows(Peter, Secret)`), Z3 — это
> solver-верификатор второго уровня.

### 3.7. Hypotheses lifecycle

HYPOTHESIS — это не "комментарий", а полноценный объект с жизненным циклом:

1. **Creation**: reasoning engine или planner создаёт $H$ с `conf ∈ [0, 1]`.
2. **Branching**: для $H$ создаётся ветка в $\text{TL}_g$.
3. **Testing**: reasoning над веткой + constraint check.
4. **Promotion**: если $H$ подтверждена $k \geq 2$ независимыми путями (через
   noisy-OR) с суммарной confidence $\geq \theta_{\text{fact}}$ — повышается до
   `FACT` и merge'ится в основную ветку timeline.
5. **Refutation**: если $H$ приводит к UNSAT — удаляется из $\text{TL}_g$.

Без проверки гипотеза не влияет на reasoning над committed facts.

### 3.8. LLM Loop — формальная спецификация

Пусть $W_g$ — текущее состояние, $\delta$ — gap/contradiction, выявленный Reasoning.

**Planner** строит задачу:

$$\text{Task} = (W_g,\; \delta,\; \text{target}: \text{Formula},\; \text{invariants}: [\text{Formula}],\; B \in \mathbb{N})$$

**LLM Context Builder**:

$$\text{Context} = \text{slice}(W_g, \delta) \cup \text{neighborText}(T, \delta) \cup \text{style}(T, \delta)$$

**LLM** (стохастическая):

$$T_{\text{cand}} \sim \text{LLM}(\text{prompt}(\text{Task}, \text{Context}))$$

**Pipeline** (тот же, что и для исходного текста):

$$T_{\text{cand}} \xrightarrow{\text{NLP}} \text{IR}_{\text{cand}} \xrightarrow{\nu} \text{Events}_{\text{cand}} \xrightarrow{\text{L2-L3}} \Delta W$$

$\Delta W$ — это **candidate delta**, не закоммичен в $W_g$.

**Validator**:

$$\text{Valid}(\Delta W, W_g, \text{Task}) = \underbrace{\text{Consistent}(W_g \uplus \Delta W)}_{\text{Z3 SAT}} \;\land\; \underbrace{\text{Resolves}(\Delta W, \delta)}_{\text{target выполнен}} \;\land\; \underbrace{\text{InvariantsHold}(W_g \uplus \Delta W, \text{Task.invariants})}_{\text{invariants не сломаны}} \;\land\; \underbrace{\text{ProvenanceOK}(\Delta W)}_{\text{derived\_from корректен}}$$

Если `Valid` → commit: $W_{g+1} = W_g \uplus \Delta W$.
Если не `Valid` → diagnostics, retry с бюджетом $B := B - 1$. При $B = 0$ →
escalation (gap помечается для человека, $W_g$ не меняется).

#### 3.8.1. Теорема (монотонность committed facts)

$$|F_g^{\text{committed}}| \text{ не убывает в } g \text{ (кроме явных retractions).}$$

*Доказательство*: $\Delta W$ либо committed (добавляет факты), либо
отбрасывается. Никакое Valid-решение не убирает факты. ⇒ $|F_{g+1}^{\text{committed}}| \geq |F_g^{\text{committed}}|$. $\square$

### 3.9. ε-importance — каноническая формула

Старая ε нормализовалась по max — непорівнимо между документами. Каноническая
формула:

$$\epsilon(e) = \alpha_1 \cdot \text{cf}(e) + \alpha_2 \cdot \text{sd}(e) + \alpha_3 \cdot \text{rar}(e)$$

где каждая компонента $\in [0, 1]$:

| Компонента | Формула | Смысл |
|------------|---------|-------|
| Causal fanout | $\text{cf}(e) = \frac{|\{e' : e \to^* e' \text{ in } C_g\}|}{|C_g|}$ | Доля последствий от общего числа событий |
| State delta | $\text{sd}(e) = \frac{\log(1 + |F_{g+1} \setminus F_g|)}{\log(1 + |F_g|)}$ | Относительный прирост фактов |
| Rarity | $\text{rar}(e) = \frac{\log P(\text{pred}(e))}{\log(1/|\mathcal{P}|)}$ | Относительная редкость предиката |

Ограничения: $\alpha_1 + \alpha_2 + \alpha_3 = 1$, $\alpha_i$ — глобальные
константы, не зависят от документа. Тогда $\epsilon \in [0, 1]$ **и сравнима
между документами**. $\alpha_i$ калибруются один раз через Python Lab.

### 3.10. Confidence propagation

Для derived fact $f_{n+1}$:

$$\text{conf}(f_{n+1}) = \kappa(\text{conf}(f_1), \ldots, \text{conf}(f_n)) \cdot \rho(\text{rule})$$

**Пороги**:

| Условие | Эпистемический тип |
|---------|-------------------|
| $\text{conf} \geq \theta_{\text{fact}}$ (default 0.8) | `FACT` |
| $\text{conf} < \theta_{\text{fact}}$ | `HYPOTHESIS` |
| Прямая цитата из текста без интерпретации | `OBSERVATION` |
| Default, может быть отменён явным фактом | `ASSUMPTION` |

---

## 4. Контракты слоёв (Rust-типы)

> Эти типы — канонический контракт. Любая реализация слоя обязана
> удовлетворять им. Сигнатуры приведены для справки; полные определения — в
> коде модулей.

### 4.1. Semantic IR

```rust
pub struct SemanticMention {
    pub entity_id: EntityId,
    pub span: TextSpan,
    pub lemma: String,
    pub grammatical_role: Role,
    pub confidence: f32,
}

pub struct SemanticRelation {
    pub source: EntityId,
    pub relation: RelationType,
    pub target: EntityId,
    pub span: TextSpan,
    pub confidence: f32,
}

pub struct SemanticIR {
    pub mentions: Vec<SemanticMention>,
    pub relations: Vec<SemanticRelation>,
    pub syntax: SyntaxForest,
    pub coreference: Vec<CorefChain>,
}
```

### 4.2. Provenance

```rust
pub enum ExtractionMethod {
    Direct,    // прямая цитата из текста
    Inferred,  // вывод Reasoning Engine
}

pub struct Provenance {
    pub document_id: DocumentId,
    pub source_span: Range<usize>,
    pub sentence_id: SentenceId,
    pub paragraph_id: ParagraphId,
    pub extraction_method: ExtractionMethod,
    pub confidence: Option<f32>,
    pub derived_from: Vec<FactId>,  // цепочка доказательства
}
```

### 4.3. Эпистемические типы

```rust
pub enum EpistemicType {
    Fact,         // committed, булева семантика
    Observation,  // прямая цитата, булева семантика
    Hypothesis,   // вероятностная, ветвь в TL
    Assumption,   // default, может быть отменён
}
```

> **CONTRADICTION НЕ является эпистемическим типом.** Это отношение между
> фактами, вычисляемое Reasoning Engine, со ссылкой на unsat core из Z3.

### 4.4. Confidence function trait

```rust
pub trait ConfidenceFn {
    fn combine(&self, confs: &[f32]) -> f32;
}

pub struct MinConfidence;     // κ_min
pub struct ProdConfidence;    // κ_prod
pub struct NoisyOr;           // κ_noisy-OR
```

Каждое правило inference-движка обязано указать свой `ConfidenceFn`.

### 4.5. Task Specification (Planner → LLM)

```rust
pub struct TaskSpec {
    pub world_snapshot: WorldSnapshot,   // срез W_g
    pub gap: Gap,                        // что не так
    pub target: Formula,                 // что должно стать true
    pub invariants: Vec<Formula>,        // что нельзя ломать
    pub budget: usize,                   // B ∈ ℕ
    pub style_context: StyleContext,
}
```

### 4.6. WorldState Delta

```rust
pub struct WorldStateDelta {
    pub added_facts: Vec<Fact>,
    pub added_events: Vec<Event>,
    pub added_relations: Vec<Relation>,
    pub retracted: Vec<FactId>,  // только для ASSUMPTION, отменённых явным фактом
}

impl WorldStateDelta {
    pub fn validate(&self, base: &WorldState, task: &TaskSpec) -> ValidationResult;
    pub fn commit(self, base: &mut WorldState);  // panics if !valid
}
```

---

## 5. Соответствие существующему коду

В таблице отмечено, какие части архитектуры уже есть в `src-tauri/src/reasoning/`,
а какие требуют реализации или рефакторинга.

| Компонент архитектуры | Файл в репо | Статус |
|----------------------|-------------|--------|
| L1 NLP | `src-tauri/src/parser/` | ⚠️ Дубль с `litgraph-core/`. Нужен workspace + adapter trait. |
| L1.5 Semantic IR | — | ❌ Отсутствует. Создать `src-tauri/src/semantic_ir/`. |
| L2 Event Layer | `reasoning/facts.rs`, `reasoning/state.rs` | ⚠️ Частично. Нет явного разделения Event vs Fact. |
| L3 World Model | `reasoning/state.rs`, `reasoning/memory.rs`, `reasoning/timeline.rs` | ⚠️ `timeline` линейный, не DAG. `memory` не синхронизирован с `facts`. |
| L4 Inference | `reasoning/inference.rs` | ⚠️ Нет `ConfidenceFn` trait, `κ` захардкожена. |
| L4 Constraints (fast) | `reasoning/constraints.rs` | ⚠️ `SetAttributeFromEvent` не реализован. |
| L4 Constraints (deep) | — | ❌ Z3 не подключён. |
| L4 Contradiction | `reasoning/contradictions.rs` | ⚠️ Нет unsat core, нет связи с Z3. |
| L4 Hypotheses | `reasoning/hypotheses.rs` | ⚠️ `hypothesis_id: 1` хардкод. Нет lifecycle. |
| L4 Causality | `reasoning/causality.rs` | ⚠️ Базовый, без формального причинного графа. |
| Planner | `reasoning/planner.rs` | ⚠️ Заглушка, нет TaskSpec. |
| LLM Context Builder | `reasoning/llm_bridge.rs` | ⚠️ Заглушка, нет slice/context assembly. |
| LLM Loop | `reasoning/llm_bridge.rs` | ❌ Нет budget, нет escalation, нет `WorldStateDelta`. |
| Validator | — | ❌ Отсутствует как отдельный модуль. |
| ε-importance | `reasoning/epsilon.rs` | ⚠️ Старая формула (нормализация по max). Заменить на каноническую. |
| Test coverage | `reasoning/integration_tests.rs` | ⚠️ `test_eval_sfera_predela_full` с 0 assert'ов. |
| Dead code masking | `lib.rs` | ⚠️ `#![allow(dead_code)]` маскирует стабы. Убрать после реализации. |

---

## 6. Roadmap (следующие шаги)

В порядке приоритета, согласно пробелам из §5:

1. **Cargo workspace**: объединить `litgraph-core` и `src-tauri` в общий
   workspace. Устранить дубль `parser/`.
2. **Semantic IR (L1.5)**: создать модуль `src-tauri/src/semantic_ir/` с
   `SemanticMention`, `SemanticRelation`, `ν` (normalization), negation handling.
3. **ConfidenceFn trait**: формализовать `κ`-функции. Каждое правило inference
   обязано указать свой `ConfidenceFn`.
4. **WorldStateDelta + Validator**: реализовать `ΔW` как тип с `validate()`
   перед `commit()`. Никаких прямых мутаций `WorldState`.
5. **Budget + escalation в LLM loop**: добавить `B ∈ ℕ`, декремент, escalation
   при `B = 0`. Гарантия терминации.
6. **Branching Timeline (DAG)**: перевести `timeline.rs` с линейной структуры
   на DAG с `hypothesis_id`-метками веток.
7. **Z3 integration**: подключить `z3 = "0.20"` как verifier-2. Реализовать
   кодирование committed facts → SMT, возвращать unsat core.
8. **ε-importance (canonical)**: заменить старую формулу на каноническую из §3.9.
9. **TODO-маркеры**: убрать `#![allow(dead_code)]`, заменить на `todo!()` с
   номерами задач.
10. **Adversarial fixtures**: E2E-тесты с заведомо противоречивыми текстами.
11. **CI**: GitHub Actions с `cargo test --workspace` на каждый PR.

---

## 7. Глоссарий

| Термин | Определение |
|--------|-------------|
| **Committed fact** | Факт с `τ ∈ {FACT, OBSERVATION}`, булева семантика |
| **Candidate delta** | `ΔW`, предлагаемое изменение WorldState, не закоммиченное |
| **Gap** | Дыра в сюжете: missing precondition, contradiction, unreachable state |
| **Unsat core** | Минимальное множество фактов, вызывающих UNSAT в Z3 |
| **Generation** | $g \in \mathcal{G}$, маркер главы/сцены/прохода LLM |
| **Hypothesis lifecycle** | Creation → Branching → Testing → Promotion/Refutation |
| **Provenance chain** | `FACT → EVENT → OBSERVATION → TEXT`, восстанавливается через `derived_from` |
| **Two-level semantics** | Committed = boolean, Hypothesis = probabilistic |
| **Task Specification** | Формальная задача для LLM: target + invariants + budget |

---

## 8. Ссылки

- Reasoning Engine спецификация: [`docs/reasoning/SPEC.md`](reasoning/SPEC.md)
- Математическое ядро POLER: [`docs/poler_math/POLER_SPEC.md`](poler_math/POLER_SPEC.md)
- Дорожная карта интеграции: [`docs/poler_math/INTEGRATION_ROADMAP.md`](poler_math/INTEGRATION_ROADMAP.md)
- **Пісочниця LLM (математика замкненого контуру)**: [`docs/llm-sandbox.md`](llm-sandbox.md)
- **Навчальна серія з математики**: [`docs/education/README.md`](education/README.md)
- NLP-кандидаты: anno (Rust), CoReNer, MAVEN-ERE dataset
- Библиотеки: petgraph, z3 0.20.2, tokenizers, candle/ONNX
