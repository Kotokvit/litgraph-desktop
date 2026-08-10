# LitGraph — Пісочниця LLM

> **Цей документ — канонічна формалізація того, як LLM живе всередині LitGraph.**
> Доповнює [`docs/architecture.md`](architecture.md) — там загальна архітектура,
> тут — математика замкненого контуру LLM.
>
> Версія: 1.0.0 · Дата: 2026-08-10 · Статус: **canonical**.

---

## 0. Принцип пісочниці

> **LLM живе всередині програми і документів. Без документів він сліпий. Без
> коробки програми він безполезен. Програма — це пісочниця для LLM: замкнений
> світ, де він може сприймати лише те, що йому дає ContextBuilder через
> проекцію нодів World Model, і діяти лише через candidate text, який
> проходить валідацію перед коммітом.**

Це не рекомендація — це контракт. Будь-яка реалізація, що дозволяє LLM:
- бачити щось поза ContextBuilder'ом,
- писати напряму в WorldState,
- зберігати стан між викликами,
- мати доступ до інтернету, файлової системи поза $D$, або інших моделей —

вважається дефектом архітектури.

---

## 1. LLM — функція без стану

LLM — детермінована функція від (ваги $W$, промпт $P$, seed $s$). У
стохастичному режимі — розподіл:

$$\text{LLM}: \mathcal{P}_{\text{prompt}} \times \mathcal{S}_{\text{seed}} \to \Sigma^*$$

**Ключове припущення (statelessness)**: між викликами LLM не зберігає стану.
Вся "пам'ять" міститься в $P$. Це не опція, це контракт:

- якщо LLM має прихований стан, ми не можемо гарантувати детермінованість,
- не можемо відтворювати тести,
- не можемо ітеративно перевіряти budget,
- не можемо побудувати explainability chain.

---

## 2. Середовище

Оточення LLM — трійка:

$$E = (P_{\text{prog}}, D, W_g)$$

| Компонент | Сенс |
|-----------|------|
| $P_{\text{prog}}$ | Внутрішній стан програми (UI, налаштування, активний viewport) |
| $D \subseteq \Sigma^*$ | Завантажені документи (тексти) |
| $W_g$ | Світ, побудований з $D$ до generation $g$ |

**Інваріант**: $D = \emptyset \Rightarrow W_g = \emptyset$. Без документів світ порожній.

---

## 3. Prompt як функція від середовища

Промпт будується ContextBuilder'ом з TaskSpec і Environment:

$$\text{Prompt} = \text{Template}(\text{ContextBuilder}(\text{Task}, E))$$

ContextBuilder — **єдиний шлях** інформації від World Model до LLM. Ніяких
"підказок збоку", ніякого інтернету, ніяких системних повідомлень, які не
пройшли через ContextBuilder.

$$\text{Context} = (\text{Slice}, \text{Neighbor}, \text{Style}, \text{Goal})$$

де:

| Поле | Функція | Сенс |
|------|---------|------|
| Slice | $\pi(W_g, \delta) \subseteq W_g$ | Проекція світу на поточну задачу |
| Neighbor | $\text{textWindow}(D, \delta) \subseteq D$ | Сусідній текст (стиль, голос, лексика) |
| Style | $\sigma(D, \delta)$ | Профіль стилю (авторський голос, мова, тон) |
| Goal | $\gamma(\delta)$ | Формальна ціль правки |

---

## 4. Node → LLM: проекція нодів у промпт

Кожна нода World Model має **проекцію** в природну мову:

$$\pi: \text{WorldState} \times \text{Task} \to \text{Text}$$

Приклади проекцій:

| Нода | Проекція $\pi$ у текст |
|------|------------------------|
| `Fact(dead(Peter, G12), τ=FACT, conf=1.0)` | "Пётр мёртв начиная с главы 12 (достоверный факт)." |
| `Fact(alive(Peter, G15), τ=OBSERVATION, conf=0.9)` | "В главе 15 Пётр наблюдается живым (прямое наблюдение, conf 0.9)." |
| `Event(Kill(Ivan, Peter, G12))` | "Иван убил Петра в главе 12." |
| `CausalEdge(Kill → Death)` | "Смерть Петра вызвана убийством." |
| `Hypothesis(H1: flashback, conf=0.4)` | "ГИПОТЕЗА (не подтверждена): сцена в G15 — флешбэк. conf=0.4." |
| `Contradiction(F1, F2, unsat_core={F1,F2})` | "ПРОТИВОРЕЧИЕ: факты F1 и F2 несовместимы. Минимальное конфликтное множество: {F1, F2}." |

### 4.1. Класифікація проекцій за епістемічним типом

Критично для hallucination prevention:

| $\tau$ | Проекція з маркером |
|--------|---------------------|
| `FACT` | без маркера (твердження) |
| `OBSERVATION` | "спостереження: ..." |
| `HYPOTHESIS` | "гіпотеза (не підтверджена): ..." |
| `ASSUMPTION` | "припущення за замовчуванням: ..." |

Без цих маркерів LLM не відрізнить факт від припущення, і отримаємо
самопідтвердження (assumption → сприймається як fact → посилюється).

---

## 5. Теорема (сліпота без документа)

$$D = \emptyset \;\Rightarrow\; \text{Context} = \emptyset \;\Rightarrow\; \text{LLM сліпий}$$

*Доказ*:

1. $D = \emptyset \Rightarrow$ Reasoning Engine не має вхідних подій
   $\Rightarrow F_g = \emptyset$, $E_g = \emptyset$, $R_g = \emptyset$,
   $C_g = \emptyset$, $\text{TL}_g = \emptyset$ $\Rightarrow W_g = \emptyset$.
2. $W_g = \emptyset \Rightarrow \pi(W_g, \delta) = \emptyset$ для будь-якого
   $\delta$.
3. $D = \emptyset \Rightarrow \text{textWindow}(D, \delta) = \emptyset$,
   $\sigma(D, \delta) = \emptyset$.
4. Reasoning Engine не виявляє жодного gap'у (бо немає фактів, отже немає
   протиріч і missing preconditions) $\Rightarrow$ Planner не генерує TaskSpec
   $\Rightarrow$ LLM не викликається взагалі.
5. Отже, при $D = \emptyset$ LLM не має стимулу: навіть якби її викликали з
   порожнім промптом, її вихід не пройшов би валідацію (немає target для
   `Resolves`). $\square$

**Наслідок**: LLM не може "вигадувати" факти поза документом. Її вихід
перевіряється на відповідність TaskSpec, який породжується з Reasoning над
$W_g$, який порожній без $D$.

---

## 6. Пісочниця — формальні обмеження

LLM живе в пісочниці з **шістьма заборонами**:

| # | Заборона | Формально |
|---|----------|-----------|
| F1 | Немає зовнішнього інтернету | $\text{LLM}$ не має доступу до мережі; єдина інформація — $\text{Context}$ |
| F2 | Немає файлової системи поза $D$ | Усі шляхи до тексту — через $D$ |
| F3 | Stateless між викликами | $\text{LLM}_{i+1}$ не залежить від $\text{LLM}_i$ крім як через зміну $W_g$ |
| F4 | Немає прямого запису в WorldState | $\text{LLM}$ пише $\text{candidate text}$, не $\text{Fact}$ |
| F5 | Немає доступу до коду програми | $\text{LLM}$ не бачить $P_{\text{prog}}$ як код, тільки через ContextBuilder |
| F6 | Немає доступу до інших моделей | Один екземпляр LLM, без call-out |

**Дозволений інтерфейс** (контракт пісочниці):

$$\text{LLM}: \text{Prompt} \to \text{CandidateText}$$

Тільки вхід і вихід. Все інше — чорна скринька з боку програми.

---

## 7. Замкнений контур

$$\boxed{\text{Nodes} \xrightarrow{\pi} \text{Context} \xrightarrow{\text{Template}} \text{Prompt} \xrightarrow{\text{LLM}} T_{\text{cand}} \xrightarrow{\text{NLP/IR/Event}} \Delta W \xrightarrow{\text{Validator}} \text{Nodes}'}$$

- Якщо $\text{Validator}(\Delta W, W_g, \text{Task}) = \text{ACCEPT}$:
  $W_{g+1} = W_g \uplus \Delta W$. Ноди змінилися, наступний $\text{Context}$
  буде іншим.
- Якщо $\text{REJECT}$: $W_{g+1} = W_g$ (ноди не змінилися), але
  $\text{diagnostics}$ додається до наступного ContextBuilder'а: LLM бачить,
  чому її відхилили, і може спробувати інакше.

---

## 8. Активація нодів (relevance filtering)

$W_g$ може бути великим (тисячі фактів). Не можна кидати все в prompt — LLM
загубиться, і це дорого. Тому:

$$\text{Slice} = \pi(W_g, \delta) = \{f \in F_g : \text{relevant}(f, \delta) \geq \theta_{\text{rel}}\}$$

де $\text{relevant}: F_g \times \Delta \to [0, 1]$ — функція релевантності, що
об'єднує:

| Компонента | Формула | Сенс |
|------------|---------|------|
| Cause distance | $d_c = \frac{1}{1 + \text{shortestPath}(f, \delta.\text{anchor}, C_g)}$ | Близькість по причинному графу |
| Temporal proximity | $d_t = \frac{1}{1 + |t(f) - t(\delta)|}$ | Близькість у часі |
| Entity overlap | $d_e = \frac{|\text{args}(f) \cap \text{args}(\delta)|}{|\text{args}(\delta)|}$ | Спільні сутності |
| Contradiction membership | $d_u = 1 \text{ if } f \in \text{unsat\_core}(\delta) \text{ else } 0$ | Пряма участь у конфлікті |

$$\text{relevant}(f, \delta) = w_c d_c + w_t d_t + w_e d_e + w_u d_u$$

$w_i$ — глобальні константи, $\sum w_i = 1$, калибруються в Python Lab.

**Наслідок**: приріст активних нодів у Context обмежений. Якщо
$|\text{Slice}| > B_{\text{ctx}}$ (budget context) — беремо топ-$B_{\text{ctx}}$
за $\text{relevant}$.

---

## 9. Властивості пісочниці

| Властивість | Формально | Гарантування |
|-------------|-----------|--------------|
| **Statelessness** | $\text{LLM}_{i+1}(P) = \text{LLM}_i(P)$ при фіксованих $W, s$ | Відтворюваність тестів |
| **Document-bounded** | $D = \emptyset \Rightarrow$ LLM не викликається | Сліпота без документа |
| **Closure** | $\text{LLM}$ бачить тільки $\text{Context}$ | Немає витоку інформації |
| **Validator-gated** | $W_{g+1} \neq W_g \Rightarrow \text{Valid}(\Delta W)$ | Монотонність committed facts |
| **Budget-bounded** | $\sum_i \mathbb{1}[\text{LLM}_i \text{ called}] \leq B_{\text{total}}$ | Термінація |
| **Deterministic under seed** | $\text{LLM}(P, s) = \text{LLM}(P, s)$ | Тестовиф властивість |
| **Hypothesis-isolated** | $H \in \text{HYPOTHESIS}$ впливає на Reasoning тільки у своїй гілці $\text{TL}$ | Немає самопідтвердження |

---

## 10. Метрика "живості" LLM у пісочниці

LLM "жива" у тому сенсі, що має стимул. Формалізуємо міру інформованості:

$$\mathcal{I}(g) = \underbrace{|\text{Slice}(g)|}_{\text{active facts}} \cdot \underbrace{\log(1 + |D|)}_{\text{document mass}} \cdot \underbrace{\mathbb{1}[\text{pending gap}]}_{\text{has work}}$$

- $\mathcal{I} = 0$ якщо $D = \emptyset$ (сліпий)
- $\mathcal{I} = 0$ якщо немає gap'у (нема роботи — LLM "спить")
- $\mathcal{I} > 0$ якщо є документ і є gap — LLM "жива": Context непорожній,
  Validator приймає або відхиляє її вихід

Це не метрика якості LLM, а метрика **наявності стимулу**. Якщо
$\mathcal{I} = 0$, виклик LLM — марнотратство токенів.

---

## 11. Anti-hallucination через projection discipline

Головна загроза: LLM "вигадує" факт поза Context. Математика дає два захисти:

### 11.1. Validators на проекцію

Будь-який факт у $\Delta W$, який не виводиться з $T_{\text{cand}}$ через
$\text{NLP} \to \text{IR} \to \text{Event}$ pipeline, відкидається.

$$\forall f \in \Delta W.\text{added}: \exists \text{ derivation } T_{\text{cand}} \xrightarrow{\text{NLP}} \text{IR} \xrightarrow{\nu} \text{Event} \xrightarrow{\text{L2-L3}} f$$

### 11.2. Closed-world assumption для Context

LLM не може посилатися на факти, яких немає в $\text{Slice}$. Якщо пише "як ми
знаємо з глави 27, що..." — але глави 27 немає в $\text{Neighbor}$ або
відповідного факту немає в $\text{Slice}$ — Validator відкидає.

$$\text{ProvenanceOK}(\Delta W) \iff \forall f \in \Delta W.\text{added}: \text{prov}(f).\text{derived\_from} \neq \emptyset \;\land\; \text{chain leads to } T_{\text{cand}} \text{ or } D$$

Пісочниця + projection discipline = математична гарантія від hallucination.

---

## 12. Контракт Rust-типів

```rust
/// Єдиний інтерфейс LLM з програмою.
/// Все, що LLM бачить — Prompt. Все, що пише — CandidateText.
pub trait LlmSandbox {
    fn generate(&self, prompt: Prompt, seed: Option<u64>) -> CandidateText;
}

/// Prompt будується ТІЛЬКИ через ContextBuilder.
/// Прямий доступ до LLM з будь-якого місця коду заборонений.
pub struct Prompt {
    pub context: Context,
    pub task: TaskSpec,
    pub template_id: TemplateId,
}

pub struct Context {
    pub slice: Vec<ProjectedNode>,    // π(W_g, δ)
    pub neighbor: Vec<TextWindow>,    // textWindow(D, δ)
    pub style: StyleProfile,          // σ(D, δ)
    pub goal: Goal,                   // γ(δ)
    pub diagnostics: Vec<Diagnostic>, // попередні REJECT-причини
}

pub struct ProjectedNode {
    pub node_id: NodeId,
    pub epistemic_marker: EpistemicType,  // FACT/OBS/HYP/ASSUM — для тексту
    pub text: String,                     // проекція в природну мову
    pub provenance: Provenance,           // для перевірки closed-world
}

pub struct CandidateText {
    pub text: String,
    pub seed: u64,                         // для відтворюваності
}

/// Validator — єдиний шлях від CandidateText до WorldStateDelta.
/// Жоден інший код не має права створювати Fact напряму.
pub trait Validator {
    fn validate(
        &self,
        candidate: CandidateText,
        base: &WorldState,
        task: &TaskSpec,
    ) -> ValidationResult;
}

pub enum ValidationResult {
    Accept(WorldStateDelta),
    Reject {
        diagnostics: Vec<Diagnostic>,
        remaining_budget: usize,
    },
    Escalate {
        reason: EscalationReason,
        gap_id: GapId,
    },
}
```

---

## 13. Посилання

- Загальна архітектура: [`docs/architecture.md`](architecture.md) §3.8 (LLM Loop),
  §3.7 (Hypotheses lifecycle), §3.5 (World Model).
- Reasoning Engine: [`docs/reasoning/SPEC.md`](reasoning/SPEC.md).
- 数学 core POLER: [`docs/poler_math/POLER_SPEC.md`](poler_math/POLER_SPEC.md).
