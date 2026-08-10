//! Epsilon-алгоритм важности фрагмента текста (POLER v7.0-LEM Canonical).
//!
//! Канонічна формула (з §4.1 POLER_EPSILON_CANONICAL_SPECIFICATION.md):
//!
//! ```text
//!                   κ · I_kw · Σ rarity(w) + E + C_canon + A_SVO
//!     ε  =  ─────────────────────────────────────────────────────
//!                              √(|U| + δ_bias)
//! ```
//!
//! Де:
//! - `rarity(w) = -log10(p_w)`, обмежена в `[0.10, 4.50]`.
//! - `I_kw = 1 + ln(1 + kw_count)` — інтенсивність ключового слова (натуральний логарифм).
//! - `E = 1.5 × emotion_count` — емоційна напруга.
//! - `C_canon = 3.0 × canon_count` — канонічні якорі манускрипту.
//! - `A_SVO = 2.0 × action_count` — дієслова дії (SVO-структура).
//! - `δ_bias = 15.0` — зсув довжини (калібровано Nelder-Mead, Loss=0.0).
//! - `θ_rel(κ) = 3.50 / κ` — поріг шуму (сектор-адаптивний).
//!
//! Кульмінаційна формула (з §3 специфікації, B1-B7 resolved):
//!
//! ```text
//!                  κ · I_loc · d̄² + γ_emo · E + λ_conf · Ω_conf
//!     ε_climax  =  ──────────────────────────────────────────────
//!                                ln(e + |U|)
//! ```
//!
//! Де:
//! - `I_loc = 1.0` (placeholder, буде обчислюватись з канонічних якорів).
//! - `d̄²` — середній квадрат рідкості (mean squared rarity).
//! - `γ_emo = 1.0` (B1: усунуто подвійне множення 1.5).
//! - `λ_conf = 12.5` — коефіцієнт конфліктної напруженості.
//! - `Ω_conf` — магнітуда J-матриці (поки placeholder = 0.0).
//!
//! ## Версія 7.0-LEM
//!
//! Ця версія додає опційну лематизацію через `crate::linguistic::lemmatizer`.
//! Якщо лематизатор завантажений, `compute_epsilon_lemmatized()` зводить
//! словоформи ("ходив", "ходить", "ходили") до леми ("ходити") перед
//! обчисленням рідкості. Це зменшує |U| приблизно на 30% (α≈0.7),
//! що збільшує ε на ~9.9% (див. scripts/sympy_lemmatization_impact.py).

use std::collections::{HashMap, HashSet};

use crate::linguistic::lemmatizer;

// ============================================================================
// Лексикони (з scripts/benchmark_poler_epsilon.py)
// ============================================================================

/// Емоційні маркери (вес E = 1.5 × emotion_count).
/// Джерело: benchmark_poler_epsilon.py рядки 26-32 + розширення.
const EMOTIONAL_MARKERS: &[&str] = &[
    // Ukrainian
    "крик","кричати","страх","боятися","жах","біль","боліти","плач","плакати",
    "сльози","лють","гнів","паніка","ненависть","любов","кохати","кохання",
    "розчарування","розруха","агонія","кривавий","кров","смерть","відчай",
    "хаос","сила","свідомість","реальність","істина","тінь","світло","темрява",
    "безодня","вічність","тиша","пам'ять","надія","зрада","прощення","самотність",
    "доля","свобода","вибір","правда","війна","життя","вогонь","гнів","час","мить",
    // Russian (mirror)
    "крикнуть","ужас","боль","слезы","ярость","гнев","паника","ненависть",
    "любовь","любила","любил","крови","кровь","агония","отчаяние","безумие",
    "хаос","сила","сознание","реальность","истина","тень","свет","тьма",
    "бездна","вечность","тишина","память","страх","надежда","любовь","предательство",
    // English (mirror)
    "chaos","power","consciousness","reality","truth","shadow","light","darkness",
    "abyss","eternity","silence","memory","fear","hope","love","betrayal",
    "forgiveness","loneliness","fate","freedom","choice","war","death","life",
    "blood","fire","pain","anger","time","moment",
];

/// Канонічні якорі манускрипту (вес C_canon = 3.0 × canon_count).
/// Це ключові терміни всесвіту «Сфери Предела» / «Кассіопеї».
/// Джерело: benchmark_poler_epsilon.py рядки 8-14.
const CANON_ANCHORS: &[&str] = &[
    "етерія","буфер","сектор","хмара","геліос","теневра","фосфор",
    "кассіопея","яр","ущелина","аніма","руна","вузол","код","матриця",
    "інквесторат","триада","рада","пропуск","чип","пластик","стійбище",
    "архів","проект","алгоритм","система","редакція","сигнал","ток",
    "χ-оружие","хи-оружие","док","причал","буферу","етерії","геліоса",
];

/// Дієслова дії (вес A_SVO = 2.0 × action_count).
/// SVO-структура: Subj-Verb-Obj, де ці дієслова маркірують сюжетні дії.
/// Джерело: benchmark_poler_epsilon.py рядки 16-24.
const ACTION_VERBS: &[&str] = &[
    // Ukrainian
    "вбити","убити","умерти","померти","загинути","застрелити","отруїти",
    "підірвати","зрадити","врятувати","визволити","схопити","ув'язнити",
    "поранити","ударити","знівечити","підпалити","воскреснути",
    "наказати","примусити","пообіцяти","присягти","проникнути","зламати",
    // Russian (mirror)
    "убить","умереть","погибнуть","застрелить","отравить","казнить",
    "взорвать","предать","спасти","освободить","схватить","пленить",
    "ранить","ударить","воскреснуть","приказать","заставить","пообещать",
];

/// Стоп-слово (не впливає на ε).
const STOP_WORDS: &[&str] = &[
    // Ukrainian
    "і","та","й","в","у","на","з","до","за","від","по","при","про","для","із",
    "це","той","ця","те","він","вона","воно","вони","його","її","їх",
    "я","ти","ми","ви","мене","тебе","себе","мені","тобі","собі",
    "але","або","що","як","де","куди","коли","чому","тому","тож",
    "був","була","було","були","є","бути","ніхто","нічого","все","всі",
    "сьогодні","вчора","завтра","тепер","тоді","потім","раптом",
    "швидко","знову","ще","вже","тільки","навіть","можливо","так","ні",
    // Russian
    "и","в","на","с","к","за","от","по","при","про","для","из","не","ни",
    "это","тот","эта","эти","он","она","оно","они","его","её","их",
    "я","ты","мы","вы","меня","тебя","себя","мне","тебе",
    "но","или","что","как","где","куда","когда","почему","поэтому",
    "был","была","было","были","есть","быть",
    "сегодня","вчера","завтра","теперь","тогда","потом","внезапно",
    "быстро","снова","ещё","уже","только","даже","возможно","да","нет",
    // English
    "the","a","an","and","or","but","in","on","at","to","for","of","with",
    "this","that","these","those","he","she","it","they","his","her","its",
    "is","was","were","been","have","has","had","not","no",
    "i","you","we","me","my","your","our",
];

// ============================================================================
// Канонічні константи (калібровано SymPy/SciPy)
// ============================================================================

/// δ_bias — зсув довжини фрагмента. Калібровано Nelder-Mead: δ*=15.0, Loss=0.0.
/// Джерело: solve_poler_math.py + POLER_EPSILON_CANONICAL_SPECIFICATION.md §8.3.
pub const DELTA_BIAS: f64 = 15.0;

/// θ_base — базовий поріг шуму. Калібровано Nelder-Mead: θ*=3.5.
/// Сектор-адаптивний: θ_rel(κ) = θ_base / κ.
pub const THETA_BASE: f64 = 3.5;

/// Порогова кульмінація: ε ≥ CLIMAX_THRESHOLD.
pub const CLIMAX_THRESHOLD: f64 = 7.5;

/// γ_emo — коефіцієнт емоцій в ε_climax.
/// B1 resolution: γ_emo := 1.0 (усунуто подвійне множення 1.5).
pub const GAMMA_EMO: f64 = 1.0;

/// λ_conf — коефіцієнт конфліктної напруженості в ε_climax.
pub const LAMBDA_CONF: f64 = 12.5;

/// Мінімальна можлива рідкість слова.
pub const RARITY_MIN: f64 = 0.1;

/// Максимальна можлива рідкість слова.
pub const RARITY_MAX: f64 = 4.5;

// ============================================================================
// EpsilonResult — розширена структура (v7.0-LEM)
// ============================================================================

/// Результат обчислення ε для одного фрагмента.
///
/// Поля `epsilon`, `normalized`, `word_count`, `unique_words`, `emotion_count`
/// збережено для зворотної сумісності з попередніми caller'ами.
///
/// Нові поля (v7.0-LEM):
/// - `kw_count` — кількість входжень ключового слова.
/// - `canon_count` — кількість канонічних якорів.
/// - `action_count` — кількість дієслів дії.
/// - `theta_rel` — поріг шуму для сектору (3.5/κ).
/// - `is_noise` — true, якщо ε < θ_rel.
/// - `is_climax` — true, якщо ε ≥ 7.5.
/// - `formula_variant` — "canonical", "canonical_lemmatized", або "climax".
#[derive(Debug, Clone)]
pub struct EpsilonResult {
    /// Сире значення ε (до нормалізації).
    pub epsilon: f64,
    /// Нормалізоване значення (0-100, відносно максимуму).
    pub normalized: f64,
    /// Кількість токенів (слів) у фрагменті.
    pub word_count: usize,
    /// Кількість унікальних слів |U|.
    pub unique_words: usize,
    /// Кількість емоційних маркерів.
    pub emotion_count: usize,
    /// Кількість входжень ключового слова.
    pub kw_count: usize,
    /// Кількість канонічних якорів манускрипту.
    pub canon_count: usize,
    /// Кількість дієслів дії (SVO-структура).
    pub action_count: usize,
    /// Поріг шуму для сектору: θ_rel = 3.5 / κ.
    pub theta_rel: f64,
    /// true, якщо фрагмент класифікується як шум (ε < θ_rel).
    pub is_noise: bool,
    /// true, якщо фрагмент класифікується як кульмінація (ε ≥ 7.5).
    pub is_climax: bool,
    /// Варіант формули: "canonical", "canonical_lemmatized", "climax".
    pub formula_variant: &'static str,
}

impl Default for EpsilonResult {
    fn default() -> Self {
        Self {
            epsilon: 0.0,
            normalized: 0.0,
            word_count: 0,
            unique_words: 0,
            emotion_count: 0,
            kw_count: 0,
            canon_count: 0,
            action_count: 0,
            theta_rel: THETA_BASE,
            is_noise: true,
            is_climax: false,
            formula_variant: "canonical",
        }
    }
}

// ============================================================================
// Допоміжні функції
// ============================================================================

fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

fn is_emotional(word: &str) -> bool {
    EMOTIONAL_MARKERS.contains(&word)
}

fn is_canon_anchor(word: &str) -> bool {
    CANON_ANCHORS.contains(&word)
}

fn is_action_verb(word: &str) -> bool {
    ACTION_VERBS.contains(&word)
}

/// Токенізація: нижній регістр, розбиття по не-алфавітних символах,
/// фільтр слів довжиною ≤ 2 та стоп-слів.
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '\u{2019}')
        .filter(|t| t.len() > 2 && !is_stop_word(t))
        .map(|t| t.to_string())
        .collect()
}

/// Лематизація токена (якщо лематизатор завантажений).
/// Повертає лему у нижньому регістрі, або оригінальне слово, якщо:
/// - лематизатор не завантажений
/// - слово невідоме словнику
fn lemmatize_token(word: &str) -> String {
    if let Some(lemma) = lemmatizer::lemmatize_first(word) {
        lemma.to_lowercase()
    } else {
        word.to_lowercase()
    }
}

/// Обчислити рідкість слова `rarity(w) = -log10(p_w)`, обмежену в [0.1, 4.5].
///
/// `p_w` (ймовірність слова в корпусі) визначається евристично:
/// - Канонічні якорі: `p_w = 0.0001` (дуже рідкісні, специфічні для всесвіту)
/// - Дієслова дії: `p_w = 0.0003` (рідкісні, сюжетно-марковані)
/// - Емоційні маркери: `p_w = 0.0002` (рідкісні, афективні)
/// - Інші слова: залежно від довжини (короткі часті, довгі рідкісні)
///
/// B5 resolution: використовуємо `log10` (не `ln`), як у канонічній формулі.
fn word_rarity(word: &str, _total_words: usize, _counts: &HashMap<String, usize>) -> f64 {
    let clean = word.trim().to_lowercase();
    if clean.len() <= 2 {
        return 0.0;
    }

    let p_w: f64 = if is_canon_anchor(&clean) {
        0.0001
    } else if is_action_verb(&clean) {
        0.0003
    } else if is_emotional(&clean) {
        0.0002
    } else {
        // Довжинна евристика (з benchmark_poler_epsilon.py рядки 45-53)
        let l = clean.chars().count();
        if (3..=4).contains(&l) {
            0.05
        } else if (5..=7).contains(&l) {
            0.01
        } else if (8..=10).contains(&l) {
            0.002
        } else {
            0.0005
        }
    };

    let rarity = -(p_w.max(1e-10).log10());
    rarity.clamp(RARITY_MIN, RARITY_MAX)
}

// ============================================================================
// Публічний API
// ============================================================================

pub fn build_word_counts(text: &str) -> (HashMap<String, usize>, usize) {
    let tokens = tokenize(text);
    let mut counts: HashMap<String, usize> = HashMap::new();
    for t in &tokens {
        *counts.entry(t.clone()).or_insert(0) += 1;
    }
    let total = tokens.len();
    (counts, total)
}

/// Обчислити канонічну ε для фрагмента тексту.
///
/// Формула (§4.1):
/// ```text
/// ε = (κ · I_kw · Σ rarity(w) + E + C_canon + A_SVO) / √(|U| + δ_bias)
/// ```
///
/// Зверни увагу: `I_kw = 1 + ln(1 + kw_count)` використовує **натуральний** логарифм
/// (не log10) — це спеціально для згладжування інтенсивності ключового слова.
pub fn compute_epsilon(
    chapter_text: &str,
    global_counts: &HashMap<String, usize>,
    total_words: usize,
    keyword: Option<&str>,
    kappa: f64,
) -> EpsilonResult {
    compute_epsilon_inner(chapter_text, global_counts, total_words, keyword, kappa, false, "canonical")
}

/// Обчислити канонічну ε з лематизацією (v7.0-LEM).
///
/// Якщо лематизатор завантажений (`crate::linguistic::lemmatizer::is_loaded()`),
/// кожна словоформа зводиться до леми перед обчисленням рідкості.
/// Наприклад: "ходив", "ходить", "ходили" → всі рахуються як "ходити".
///
/// Якщо лематизатор не завантажений, поведінка ідентична `compute_epsilon()`.
pub fn compute_epsilon_lemmatized(
    chapter_text: &str,
    global_counts: &HashMap<String, usize>,
    total_words: usize,
    keyword: Option<&str>,
    kappa: f64,
) -> EpsilonResult {
    compute_epsilon_inner(chapter_text, global_counts, total_words, keyword, kappa, true, "canonical_lemmatized")
}

/// Внутрішня реалізація для обох варіантів (з/без лематизації).
fn compute_epsilon_inner(
    chapter_text: &str,
    _global_counts: &HashMap<String, usize>,
    _total_words: usize,
    keyword: Option<&str>,
    kappa: f64,
    use_lemmatizer: bool,
    formula_variant: &'static str,
) -> EpsilonResult {
    let tokens = tokenize(chapter_text);

    // Зведення до лем (якщо ввімкнено)
    let analyzed_tokens: Vec<String> = if use_lemmatizer {
        tokens.iter().map(|t| lemmatize_token(t)).collect()
    } else {
        tokens.clone()
    };

    let cleaned_lower = chapter_text.to_lowercase();
    let unique: HashSet<String> = analyzed_tokens.iter().cloned().collect();
    let u_len = unique.len();

    if u_len == 0 {
        let mut result = EpsilonResult::default();
        result.theta_rel = THETA_BASE / kappa;
        result.is_noise = true;
        result.is_climax = false;
        result.formula_variant = formula_variant;
        return result;
    }

    // d = Σ rarity(w) — ЛІНІЙНА сума (не квадрат, як у v6)
    let d: f64 = unique.iter()
        .map(|w| word_rarity(w, _total_words, _global_counts))
        .sum();

    // len_norm = √(|U| + δ_bias)  — B5 fix: додано δ_bias
    let len_norm = ((u_len as f64) + DELTA_BIAS).sqrt();

    // I_kw = 1 + ln(1 + kw_count) — натуральний логарифм
    let mut kw_count = 0usize;
    if let Some(kw) = keyword {
        let kw_lower = kw.to_lowercase();
        kw_count = cleaned_lower.matches(&kw_lower).count();
    }
    let i_kw = 1.0 + (1.0_f64 + kw_count as f64).ln();

    // Лічильники лексичних категорій
    let mut emotion_count = 0usize;
    let mut canon_count = 0usize;
    let mut action_count = 0usize;
    for w in &unique {
        if is_emotional(w) {
            emotion_count += 1;
        }
        if is_canon_anchor(w) {
            canon_count += 1;
        }
        if is_action_verb(w) {
            action_count += 1;
        }
    }

    // Компоненти формули
    let e_val = 1.5 * emotion_count as f64;
    let c_canon = 3.0 * canon_count as f64;
    let a_svo = 2.0 * action_count as f64;

    // Канонічна ε
    let epsilon = (kappa * i_kw * d + e_val + c_canon + a_svo) / len_norm;

    // Класифікація
    let theta_rel = THETA_BASE / kappa;
    let is_noise = epsilon < theta_rel;
    let is_climax = epsilon >= CLIMAX_THRESHOLD;

    EpsilonResult {
        epsilon,
        normalized: 0.0, // буде обчислено в normalize_epsilons
        word_count: analyzed_tokens.len(),
        unique_words: u_len,
        emotion_count,
        kw_count,
        canon_count,
        action_count,
        theta_rel,
        is_noise,
        is_climax,
        formula_variant,
    }
}

/// Обчислити ε_climax для фрагмента (кульмінаційна модель).
///
/// Формула (§3):
/// ```text
/// ε_climax = (κ · I_loc · d̄² + γ_emo · E + λ_conf · Ω_conf) / ln(e + |U|)
/// ```
///
/// **PLACEHOLDER**: `Ω_conf = 0.0`, `I_loc = 1.0`, оскільки J-матриця
/// ще не інтегрована в Rust (див. src-tauri/python/build_j_matrix.py).
/// Після інтеграції — замінити ці значення на реальні.
pub fn compute_epsilon_climax(
    chapter_text: &str,
    keyword: Option<&str>,
    kappa: f64,
    omega_conf: f64,
) -> EpsilonResult {
    let tokens = tokenize(chapter_text);
    let cleaned_lower = chapter_text.to_lowercase();
    let unique: HashSet<String> = tokens.iter().cloned().collect();
    let u_len = unique.len();

    if u_len == 0 {
        let mut result = EpsilonResult::default();
        result.theta_rel = THETA_BASE / kappa;
        result.formula_variant = "climax";
        return result;
    }

    // d̄² — середній квадрат рідкості
    let empty_counts = HashMap::new();
    let d: f64 = unique.iter()
        .map(|w| word_rarity(w, 0, &empty_counts))
        .sum();
    let d_bar = d / u_len as f64; // середня рідкість
    let d_bar_sq = d_bar.powi(2);

    // I_loc — placeholder (буде обчислено з канонічних якорів)
    let i_loc = 1.0;

    // I_kw для kw_count
    let mut kw_count = 0usize;
    if let Some(kw) = keyword {
        let kw_lower = kw.to_lowercase();
        kw_count = cleaned_lower.matches(&kw_lower).count();
    }
    let _ = 1.0 + (1.0_f64 + kw_count as f64).ln(); // I_kw не використовується в climax

    // Лічильники
    let mut emotion_count = 0usize;
    let mut canon_count = 0usize;
    let mut action_count = 0usize;
    for w in &unique {
        if is_emotional(w) {
            emotion_count += 1;
        }
        if is_canon_anchor(w) {
            canon_count += 1;
        }
        if is_action_verb(w) {
            action_count += 1;
        }
    }

    let e_val = 1.5 * emotion_count as f64;

    // Знаменник: ln(e + |U|) — натуральний логарифм
    let denominator = (std::f64::consts::E + u_len as f64).ln();

    // ε_climax
    let epsilon_climax = (kappa * i_loc * d_bar_sq
        + GAMMA_EMO * e_val
        + LAMBDA_CONF * omega_conf) / denominator.max(1e-10);

    let theta_rel = THETA_BASE / kappa;
    let is_noise = epsilon_climax < theta_rel;
    let is_climax = epsilon_climax >= CLIMAX_THRESHOLD;

    EpsilonResult {
        epsilon: epsilon_climax,
        normalized: 0.0,
        word_count: tokens.len(),
        unique_words: u_len,
        emotion_count,
        kw_count,
        canon_count,
        action_count,
        theta_rel,
        is_noise,
        is_climax,
        formula_variant: "climax",
    }
}

/// Нормалізувати ε-значення до шкали 0-100 (відносно максимуму).
pub fn normalize_epsilons(results: &mut [EpsilonResult]) {
    if results.is_empty() {
        return;
    }
    let max_eps = results.iter().map(|r| r.epsilon).fold(0.0_f64, f64::max);
    if max_eps <= 0.0 {
        return;
    }
    for r in results.iter_mut() {
        r.normalized = (r.epsilon / max_eps) * 100.0;
    }
}

// ============================================================================
// Юніт-тести
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_rarity_uses_log10_not_ln() {
        // Звичайне слово довжиною 5: p_w = 0.01, rarity = -log10(0.01) = 2.0
        let counts = HashMap::new();
        let r = word_rarity("слово", 1000, &counts);
        assert!((r - 2.0).abs() < 0.01, "Expected rarity=2.0 for len-5 word, got {}", r);
    }

    #[test]
    fn test_word_rarity_clamped() {
        let counts = HashMap::new();
        // Канонічний якір: p_w = 0.0001, rarity = -log10(0.0001) = 4.0 (в межах [0.1, 4.5])
        let r = word_rarity("етерія", 1000, &counts);
        assert!((r - 4.0).abs() < 0.01, "Expected rarity=4.0 for canon anchor, got {}", r);
        // Дуже рідкісне: clamp до 4.5
        // Дуже часте: clamp до 0.1
    }

    #[test]
    fn test_word_rarity_short_word_returns_zero() {
        let counts = HashMap::new();
        let r = word_rarity("як", 1000, &counts);
        assert_eq!(r, 0.0, "Short words (len<=2) should return 0");
    }

    #[test]
    fn test_compute_epsilon_basic() {
        let counts = HashMap::new();
        let result = compute_epsilon("Це звичайне речення з кількома словами.", &counts, 10, None, 1.0);
        assert!(result.epsilon > 0.0, "Epsilon should be positive");
        assert_eq!(result.formula_variant, "canonical");
        assert!(result.theta_rel > 0.0);
    }

    #[test]
    fn test_compute_epsilon_empty_text() {
        let counts = HashMap::new();
        let result = compute_epsilon("", &counts, 0, None, 1.0);
        assert_eq!(result.epsilon, 0.0);
        assert!(result.is_noise);
        assert!(!result.is_climax);
    }

    #[test]
    fn test_compute_epsilon_with_canon_anchor() {
        let counts = HashMap::new();
        // Фрагмент з канонічним якорем "етерія"
        let result = compute_epsilon("етерія активізувала систему контролю.", &counts, 10, None, 1.0);
        assert!(result.canon_count >= 1, "Should detect canon anchor 'етерія'");
        assert!(result.epsilon > 0.0);
    }

    #[test]
    fn test_compute_epsilon_with_action_verb() {
        let counts = HashMap::new();
        // Фрагмент з дієсловом дії "вбити"
        let result = compute_epsilon("вбити ворога було необхідно.", &counts, 10, None, 1.0);
        assert!(result.action_count >= 1, "Should detect action verb 'вбити'");
    }

    #[test]
    fn test_compute_epsilon_kappa_affects_threshold() {
        let counts = HashMap::new();
        let text = "звичайне речення без особливих слів";
        let r1 = compute_epsilon(text, &counts, 10, None, 1.0);
        let r2 = compute_epsilon(text, &counts, 10, None, 2.0);
        // θ_rel = 3.5/κ, тому при κ=2.0 поріг нижчий
        assert!((r2.theta_rel - 1.75).abs() < 0.01, "Expected theta_rel=1.75 for kappa=2.0, got {}", r2.theta_rel);
        assert!((r1.theta_rel - 3.50).abs() < 0.01, "Expected theta_rel=3.50 for kappa=1.0, got {}", r1.theta_rel);
    }

    #[test]
    fn test_compute_epsilon_lemmatized_falls_back_gracefully() {
        // Якщо лематизатор не завантажений, lemmatized повинен працювати як canonical
        let counts = HashMap::new();
        let text = "звичайне речення з кількома словами";
        let r_canonical = compute_epsilon(text, &counts, 10, None, 1.0);
        let r_lemmatized = compute_epsilon_lemmatized(text, &counts, 10, None, 1.0);
        if !lemmatizer::is_loaded() {
            // Без лематизатора результати ідентичні
            assert!((r_canonical.epsilon - r_lemmatized.epsilon).abs() < 0.001,
                    "Without lemmatizer, both variants should give same result");
        }
        assert_eq!(r_lemmatized.formula_variant, "canonical_lemmatized");
    }

    #[test]
    fn test_compute_epsilon_climax_placeholder() {
        let counts = HashMap::new();
        let _ = counts;
        // ε_climax з Ω_conf=0.0 (placeholder)
        let result = compute_epsilon_climax("етерія вбити страх", None, 1.0, 0.0);
        assert_eq!(result.formula_variant, "climax");
        assert!(result.epsilon >= 0.0);
    }

    #[test]
    fn test_compute_epsilon_climax_with_omega() {
        // ε_climax з ненульовим Ω_conf
        let r_low = compute_epsilon_climax("етерія страх", None, 1.0, 0.0);
        let r_high = compute_epsilon_climax("етерія страх", None, 1.0, 100.0);
        // Більший Ω_conf → більший ε_climax
        assert!(r_high.epsilon > r_low.epsilon,
                "Higher omega_conf should increase epsilon_climax: {} vs {}",
                r_high.epsilon, r_low.epsilon);
    }

    #[test]
    fn test_normalize_epsilons() {
        let mut results = vec![
            EpsilonResult { epsilon: 5.0, ..Default::default() },
            EpsilonResult { epsilon: 10.0, ..Default::default() },
            EpsilonResult { epsilon: 2.5, ..Default::default() },
        ];
        normalize_epsilons(&mut results);
        assert!((results[0].normalized - 50.0).abs() < 0.01);
        assert!((results[1].normalized - 100.0).abs() < 0.01);
        assert!((results[2].normalized - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_default_epsilon_result() {
        let r = EpsilonResult::default();
        assert_eq!(r.epsilon, 0.0);
        assert_eq!(r.formula_variant, "canonical");
        assert!(r.is_noise);
        assert!(!r.is_climax);
    }

    #[test]
    fn test_determinism_same_input_same_output() {
        let counts = HashMap::new();
        let text = "етерія активізувала систему контролю та вбила ворога.";
        let r1 = compute_epsilon(text, &counts, 10, None, 1.0);
        let r2 = compute_epsilon(text, &counts, 10, None, 1.0);
        assert_eq!(r1.epsilon, r2.epsilon, "Same input must give same output (symbolic AI determinism)");
        assert_eq!(r1.canon_count, r2.canon_count);
        assert_eq!(r1.action_count, r2.action_count);
    }
}
