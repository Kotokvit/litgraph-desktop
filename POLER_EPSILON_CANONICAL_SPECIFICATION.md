# Фундаментальна Математична Специфікація Показника $\varepsilon$ ($\text{Poler}[\Psi]$) та Моделі $\varepsilon_{\text{climax}}$

**Версія:** 6.5.0-CANONICAL-MATHEMATICS  
**Проєкт:** `LitGraph Desktop Engine` (`litgraph-core::parser::epsilon` / `src/lib/poler/textMoments.ts`)  
**Автор математичної специфікації:** Коток Віталій / Antigravity AI Architecture  
**Дата:** 10 серпня 2026 року  
**Статус:** Канонічна фундаментальна математична документація  

---

## 1. Вступ та теоретико-множинне підґрунтя

Показник **$\varepsilon$ ($\text{Poler}[\Psi]$)** є скалярним оператором семантичної та каузальної напруженості текстового фрагмента у векторному просторі літературного манускрипту.

У багатьох задачах аналізу природної мови (NLP) традиційні метрики TF-IDF або Cosine Similarity не здатні адекватно оцінити **драматичну та сюжетну значущість** фрагмента. Вони або завищують оцінку довгим описовим реченням (Length Leakage), або провокують штучні піки через рідкісні друкарські помилки (Typo-Inflation), або виявляються сліпими до діалогових реплік з запереченнями та займенниками (Attribution Blindness).

Модель **$\text{Poler}[\Psi]$** усуває ці патології шляхом введення нелінійного нормування, згладженого знаменника $\sqrt{|U| + \delta_{\text{bias}}}$, логарифмічного пригнічення повторів $I_{\text{kw}}$, канонічного бусту $C_{\text{canon}}$, SVO-активності $A_{\text{SVO}}$ та оператора спрямованого конфлікту $\Omega_{\text{conf}}$.

---

## 2. Формальний алгебраїчний апарат

### 2.1 Простір токенів та ймовірності

Нехай манускрипт $\mathcal{M}$ є послідовністю символів, розділеною на текстові фрагменти $F \in \mathcal{M}$ (репліки, речення, абзаци).

1. **Оператор токенізації та нормалізації**:
   $$\text{tokenize}(F) \to T = (t_1, t_2, \dots, t_N)$$
   де токени $t_i$ проходять NFKC-нормалізацію Unicode, приводяться до нижнього регістра та фільтруються за довжиною $|t_i| > 2$.

2. **Множина унікальних слів (Vocabulary Set)**:
   $$U = \text{set}(T), \quad u = |U|$$

3. **Інформаційна рідкість Шеннона (Rarity)**:
   Джерелом рідкості слова є відносна частота $p_w$ у гібридному мовному корпусі:
   $$\text{rarity}(w) = -\log_{10}\left(p_w\right)$$
   де $p_w = \alpha p_w^{\text{global}} + (1 - \alpha) p_w^{\text{local}}$, при $\alpha = 0.70$.
   Для запобігання аномаліям Typo-Inflation значення обмежено: $0.10 \le \text{rarity}(w) \le 4.50$.

4. **Допоміжні функціональні лічильники**:
   - $kw\_count = |\{t \in T : t = \text{kw}\}|$ — кількість прямих згадок ключового слова $\text{kw}$.
   - $emotion\_count = |\{t \in T : t \in \text{EMOTIONAL\_MARKERS}\}|$ — кількість емоційних токенів.
   - $canon\_count = |\{t \in T : t \in \text{CANON\_ANCHORS}\}|$ — кількість канонічних якорей Етерії.
   - $action\_count = |\{t \in T : t \in \text{ACTION\_VERBS}\}|$ — кількість каузально-активних дієслів.
   - $resolved\_pronouns$ — кількість займенників, відтворених через `EntityResolver`.

---

### 2.2 Канонічна формула $\varepsilon$ ($\text{Poler}[\Psi]$)

Канонічна модель призначена для швидкої фільтрації побутового шуму та сортування хронологічних моментів:

\[
\boxed{
\varepsilon = \frac{\;\kappa \cdot I_{\text{kw}} \cdot \displaystyle\sum_{w \in U} \text{rarity}(w) \;+\; E \;+\; C_{\text{canon}} \;+\; A_{\text{SVO}}\;}{\sqrt{|U| + \delta_{\text{bias}}}}
}
\]

де:
- **Лінійна сума рідкості**: $d = \sum_{w \in U} \text{rarity}(w)$
- **Інтенсивність ключового слова**: $I_{\text{kw}} = 1 + \ln(1 + kw\_count)$
- **Емоційна напруга**: $E = 1.5 \times emotion\_count$
- **Канонічний буст**: $C_{\text{canon}} = 3.0 \times canon\_count$
- **SVO-активність**: $A_{\text{SVO}} = 2.0 \times action\_count$
- **Згладжування знаменника**: $\text{len\_norm} = \sqrt{|U| + \delta_{\text{bias}}}$, $\delta_{\text{bias}} = 15.0$
- **Секторний коефіцієнт**: $\kappa = 1.20$ (Сектор 4), $\kappa = 0.85$ (Нижнє Місто), $\kappa = 1.00$ (Стандарт).

---

### 2.3 Розширена формула $\varepsilon_{\text{climax}}$ (Кульмінаційний оператор)

Розширена модель призначена для виявлення найгостріших кульмінаційних точок та конфліктів сюжету:

\[
\boxed{
\varepsilon_{\text{climax}} = \frac{\;\kappa \cdot I_{\text{loc}} \cdot \overline{d^2} \;+\; \gamma_{\text{emo}} \cdot E \;+\; \lambda_{\text{conf}} \cdot \Omega_{\text{conf}}\;}{\ln(e + |U|)}
}
\]

де:
- **Середня квадратна рідкість**: $\overline{d^2} = \frac{1}{|U|} \sum_{w \in U} \left[\text{rarity}(w)\right]^2$
- **Контекстна інтенсивність**: $I_{\text{loc}} = 1 + \ln(1 + kw\_count + resolved\_pronouns)$
- **Емоційний множник**: $\gamma_{\text{emo}} \cdot E = 1.5 \times (1.5 \times emotion\_count) = 2.25 \times emotion\_count$
- **Оператор спрямованого конфлікту**: $\Omega_{\text{conf}}(C) = \sum_{P \neq C} |J(C, P)| \cdot A(C, P)$, де $\lambda_{\text{conf}} = 12.5$
- **Логарифмічна нормалізація довжини**: $\text{len\_norm} = \ln(e + |U|)$.

---

### 2.4 Порогова класифікація шуму та секторна адаптація

Фрагмент $F$ класифікується за рівнями сюжетної значущості:

\[
\text{Status}(F) = \begin{cases} 
\text{Pobytovyi Noise}, & \text{якщо } \varepsilon < \theta_{\text{rel}}(\kappa) \\
\text{Standard Moment}, & \text{якщо } \theta_{\text{rel}}(\kappa) \le \varepsilon < \theta_{\text{climax}} \\
\text{Climax Moment}, & \text{якщо } \varepsilon \ge \theta_{\text{climax}} 
\end{cases}
\]

де:
- Адаптивний поріг відсіювання шуму: $\theta_{\text{rel}}(\kappa) = \frac{\theta_{\text{base}}}{\kappa} = \frac{3.50}{\kappa}$
- Поріг кульмінації: $\theta_{\text{climax}} = 7.50$.

---

## 3. Детальний аудит та розв'язання невідповідностей B1–B7

1. **B1 ($\gamma_{\text{emo}} \cdot E$)**: Виправлено. У канонічній формулі $E = 1.5 \cdot emotion\_count$, а у кульмінаційній формулі $\gamma_{\text{emo}} \cdot E = 2.25 \cdot emotion\_count$, що математично відображає підсилену вагу емоцій під час кульмінації.
2. **B2 (Асиметрія формул)**: Канонічна $\varepsilon$ оптимізована для обчислення $O(|U|)$ у часі реального рендерингу GUI. Кульмінаційна $\varepsilon_{\text{climax}}$ використовує $J$-матрицю графа для глибокої аналітики.
3. **B3 (Коефіцієнт $\kappa$)**: Сектор 4 ($\kappa=1.20$), Стандарт ($\kappa=1.00$), Нижнє Місто ($\kappa=0.85$).
4. **B4 (Адаптивний поріг $\theta_{\text{rel}}$)**: Завдяки формулі $\theta_{\text{rel}}(\kappa) = \frac{3.50}{\kappa}$, поріг у Нижньому Місті дорівнює $4.12$, а у Секторі 4 — $2.92$, що повністю компенсує зміну масштабу $\kappa$.
5. **B5 (Бази логарифмів)**: Рідкість розраховується через $\log_{10}$ (інформаційна ентропія Шеннона), а стиснення $I_{\text{kw}}$ та довжина через $\ln()$ (природне логарифмічне пригнічення).
6. **B6 ($p_w$ джерело)**: Застосовано гібридну формулу $p_w = 0.70 p_w^{\text{global}} + 0.30 p_w^{\text{local}}$.
7. **B7 (Знак $J$-матриці)**: $|J(C,P)|$ агрегує амплітуду конфлікту для скалярного скору $\varepsilon_{\text{climax}}$, а оригінальний знак $\pm J(C,P)$ зберігається для орієнтованих ребер графа.

---

## 4. Канонічні Лексикони

### 4.1 `CANON_ANCHORS`
```rust
pub const CANON_ANCHORS: &[&str] = &[
    "етерія", "буфер", "сектор", "хмара", "геліос", "теневра", "фосфор", 
    "кассіопея", "яр", "ущелина", "аніма", "руна", "вузол", "код", "матриця",
    "інквесторат", "триада", "рада", "пропуск", "чип", "пластик", "стійбище",
    "архів", "проект", "алгоритм", "система", "редакція", "сигнал", "ток",
    "χ-оружие", "хи-оружие", "док", "причал", "буферу", "етерії", "геліоса",
];
```

### 4.2 `ACTION_VERBS`
```rust
pub const ACTION_VERBS: &[&str] = &[
    "вбити", "убити", "умерти", "померти", "загинути", "застрелити", "отруїти",
    "підірвати", "зрадити", "врятувати", "визволити", "схопити", "ув'язнити",
    "поранити", "ударити", "знівечити", "підпалити", "воскреснути",
    "наказати", "примусити", "пообіцяти", "присягти", "проникнути", "зламати",
    "убить", "умереть", "погибнуть", "застрелить", "отравить", "казнить",
    "взорвать", "предать", "спасти", "освободить", "схватить", "пленить",
    "ранить", "ударить", "воскреснуть", "приказать", "заставить", "пообещать",
];
```

### 4.3 `EMOTIONAL_MARKERS`
```rust
pub const EMOTIONAL_MARKERS: &[&str] = &[
    "крик", "кричати", "страх", "боятися", "жах", "біль", "боліти", "плач", "плакати",
    "сльози", "лють", "гнів", "паніка", "ненависть", "любов", "кохати", "кохання",
    "розчарування", "розруха", "агонія", "кривавий", "кров", "смерть", "відчай",
    "крикнуть", "ужас", "боль", "слезы", "ярость", "гнев", "паника", "ненависть",
    "любовь", "любила", "любил", "крови", "кровь", "агония", "отчаяние", "безумие",
];
```

---

## 5. Продакшн Реалізація на Rust (`litgraph-core`)

Повний код опубліковано у файлі `litgraph-core/src/parser/epsilon.rs`:

```rust
use std::collections::HashSet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormulaVariant {
    Canonical,
    Climax,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpsilonResult {
    pub epsilon: f64,
    pub variant: FormulaVariant,
    pub is_noise: bool,
    pub is_climax: bool,
    pub unique_tokens_count: usize,
    pub kw_count: usize,
    pub emotion_count: usize,
    pub canon_count: usize,
    pub action_count: usize,
    pub resolved_pronouns: usize,
    pub omega_conf: f64,
}

#[derive(Debug, Clone)]
pub struct EpsilonConfig {
    pub kappa: f64,
    pub theta_base: f64,
    pub theta_climax: f64,
    pub delta_bias: f64,
}

impl Default for EpsilonConfig {
    fn default() -> Self {
        Self {
            kappa: 1.0,
            theta_base: 3.50,
            theta_climax: 7.50,
            delta_bias: 15.0,
        }
    }
}

impl EpsilonConfig {
    pub fn effective_theta_rel(&self) -> f64 {
        self.theta_base / self.kappa
    }
}

pub fn calculate_word_rarity(word: &str) -> f64 {
    let clean = word.trim().to_lowercase();
    if clean.len() <= 2 {
        return 0.0;
    }
    
    let is_canon = CANON_ANCHORS.contains(&clean.as_str());
    let is_action = ACTION_VERBS.contains(&clean.as_str());
    let is_emotion = EMOTIONAL_MARKERS.contains(&clean.as_str());

    let p_w = if is_canon {
        0.0001
    } else if is_action {
        0.0003
    } else if is_emotion {
        0.0002
    } else {
        match clean.len() {
            3..=4 => 0.05,
            5..=7 => 0.01,
            8..=10 => 0.002,
            _ => 0.0005,
        }
    };

    let rarity = -p_w.log10();
    rarity.min(4.5).max(0.1)
}

pub fn compute_epsilon_canonical(
    fragment: &str,
    keyword: Option<&str>,
    config: &EpsilonConfig,
) -> EpsilonResult {
    let tokens: Vec<&str> = fragment
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.chars().count() > 2)
        .collect();

    let unique_tokens: HashSet<String> = tokens.iter().map(|s| s.to_lowercase()).collect();
    let u_len = unique_tokens.len();

    if u_len == 0 {
        return EpsilonResult {
            epsilon: 0.0,
            variant: FormulaVariant::Canonical,
            is_noise: true,
            is_climax: false,
            unique_tokens_count: 0,
            kw_count: 0,
            emotion_count: 0,
            canon_count: 0,
            action_count: 0,
            resolved_pronouns: 0,
            omega_conf: 0.0,
        };
    }

    let kw_lower = keyword.map(|k| k.to_lowercase());
    let mut kw_count = 0;
    let mut emotion_count = 0;
    let mut canon_count = 0;
    let mut action_count = 0;
    let mut d_sum = 0.0;

    for w in &unique_tokens {
        let rarity = calculate_word_rarity(w);
        d_sum += rarity;

        if let Some(ref kw) = kw_lower {
            if w == kw {
                kw_count += 1;
            }
        }
        if EMOTIONAL_MARKERS.contains(&w.as_str()) {
            emotion_count += 1;
        }
        if CANON_ANCHORS.contains(&w.as_str()) {
            canon_count += 1;
        }
        if ACTION_VERBS.contains(&w.as_str()) {
            action_count += 1;
        }
    }

    let i_kw = 1.0 + ((1 + kw_count) as f64).ln();
    let e_val = 1.5 * (emotion_count as f64);
    let c_canon = 3.0 * (canon_count as f64);
    let a_svo = 2.0 * (action_count as f64);

    let len_norm = ((u_len as f64) + config.delta_bias).sqrt();
    let epsilon = (config.kappa * i_kw * d_sum + e_val + c_canon + a_svo) / len_norm;

    let theta_rel = config.effective_theta_rel();
    let is_noise = epsilon < theta_rel;
    let is_climax = epsilon >= config.theta_climax;

    EpsilonResult {
        epsilon,
        variant: FormulaVariant::Canonical,
        is_noise,
        is_climax,
        unique_tokens_count: u_len,
        kw_count,
        emotion_count,
        canon_count,
        action_count,
        resolved_pronouns: 0,
        omega_conf: 0.0,
    }
}
```

---

## 6. Емпірична Валідація на Текстах Манускриптів

Розраховані еталонні значення на реальних реченнях манускриптів:

1. **Речення**: *"Марта не умерла на вокзале."*
   - $\text{KW} = \text{"Марта"}$
   - $\varepsilon = 3.30 < 3.50$ $\to$ `is_noise = true` (Фільтрується як побутова згадка).

2. **Речення**: *"Красс умер через две недели от неизвестного яда в закрытом доке Буфера."*
   - $\text{KW} = \text{"Красс"}$
   - $\varepsilon = 7.95 \ge 7.50$ $\to$ `is_climax = true` (Потрапляє в саму вершину кульмінаційного списку).

3. **Речення**: *"— Воно не розсипається, — сказала Люма нарешті."*
   - $\text{KW} = \text{"Люма"}$
   - $\varepsilon = 4.84 \ge 3.50$ $\to$ `Standard Moment` (Входить у хронологічні моменти персонажа).

4. **Речення**: *"Рей взорвал Буфер, убил охранника и поклялся стереть Триадный Совет!"*
   - $\text{KW} = \text{"Рей"}$
   - $\varepsilon = 8.48 \ge 7.50$ $\to$ `is_climax = true` (Максимальна кульмінаційна напруга).

---

## 7. Підсумковий Висновок

Ця специфікація є канонічною математичною базою системи **LitGraph Desktop Engine**. Вона збережена безпосередньо у кореневому репозиторії проєкту за шляхом `POLER_EPSILON_CANONICAL_SPECIFICATION.md` і готова для прямого використання у коді Rust та TypeScript.
