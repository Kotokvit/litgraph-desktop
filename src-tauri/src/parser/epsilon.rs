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
//! Цей файл — Tauri-версія. Лематизація (v7.0-LEM) тут не підключена,
//! бо `src-tauri` не залежить від `litgraph-core::linguistic::lemmatizer`.
//! Для тестів з лематизацією використовуйте `litgraph-core`.

use std::collections::{HashMap, HashSet};

// ============================================================================
// Лексикони (з scripts/benchmark_poler_epsilon.py)
// ============================================================================

/// Емоційні маркери (вес E = 1.5 × emotion_count).
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
const CANON_ANCHORS: &[&str] = &[
    "етерія","буфер","сектор","хмара","геліос","теневра","фосфор",
    "кассіопея","яр","ущелина","аніма","руна","вузол","код","матриця",
    "інквесторат","триада","рада","пропуск","чип","пластик","стійбище",
    "архів","проект","алгоритм","система","редакція","сигнал","ток",
    "χ-оружие","хи-оружие","док","причал","буферу","етерії","геліоса",
];

/// Дієслова дії (вес A_SVO = 2.0 × action_count).
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
    "і","та","й","в","у","на","з","до","за","від","по","при","про","для","із",
    "це","той","ця","те","він","вона","воно","вони","його","її","їх",
    "я","ти","ми","ви","мене","тебе","себе","мені","тобі","собі",
    "але","або","що","як","де","куди","коли","чому","тому","тож",
    "був","була","було","були","є","бути","ніхто","нічого","все","всі",
    "сьогодні","вчора","завтра","тепер","тоді","потім","раптом",
    "швидко","знову","ще","вже","тільки","навіть","можливо","так","ні",
    "и","в","на","с","к","за","от","по","при","про","для","из","не","ни",
    "это","тот","эта","эти","он","она","оно","они","его","её","их",
    "я","ты","мы","вы","меня","тебя","себя","мне","тебе",
    "но","или","что","как","где","куда","когда","почему","поэтому",
    "был","была","было","были","есть","быть",
    "сегодня","вчера","завтра","теперь","тогда","потом","внезапно",
    "быстро","снова","ещё","уже","только","даже","возможно","да","нет",
    "the","a","an","and","or","but","in","on","at","to","for","of","with",
    "this","that","these","those","he","she","it","they","his","her","its",
    "is","was","were","been","have","has","had","not","no",
    "i","you","we","me","my","your","our",
];

// ============================================================================
// Канонічні константи (калібровано SymPy/SciPy)
// ============================================================================

pub const DELTA_BIAS: f64 = 15.0;
pub const THETA_BASE: f64 = 3.5;
pub const CLIMAX_THRESHOLD: f64 = 7.5;
pub const GAMMA_EMO: f64 = 1.0;
pub const LAMBDA_CONF: f64 = 12.5;
pub const RARITY_MIN: f64 = 0.1;
pub const RARITY_MAX: f64 = 4.5;

// ============================================================================
// EpsilonResult — розширена структура (v7.0-LEM)
// ============================================================================

#[derive(Debug, Clone)]
pub struct EpsilonResult {
    pub epsilon: f64,
    pub normalized: f64,
    pub word_count: usize,
    pub unique_words: usize,
    pub emotion_count: usize,
    pub kw_count: usize,
    pub canon_count: usize,
    pub action_count: usize,
    pub theta_rel: f64,
    pub is_noise: bool,
    pub is_climax: bool,
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

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '\u{2019}')
        .filter(|t| t.len() > 2 && !is_stop_word(t))
        .map(|t| t.to_string())
        .collect()
}

/// Обчислити рідкість слова `rarity(w) = -log10(p_w)`, обмежену в [0.1, 4.5].
/// B5 resolution: використовуємо log10 (не ln).
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

pub fn compute_epsilon(
    chapter_text: &str,
    global_counts: &HashMap<String, usize>,
    total_words: usize,
    keyword: Option<&str>,
    kappa: f64,
) -> EpsilonResult {
    let tokens = tokenize(chapter_text);
    let cleaned_lower = chapter_text.to_lowercase();

    let unique: HashSet<&str> = tokens.iter().map(|s| s.as_str()).collect();
    let u_len = unique.len();

    if u_len == 0 {
        let mut result = EpsilonResult::default();
        result.theta_rel = THETA_BASE / kappa;
        result.is_noise = true;
        result.is_climax = false;
        return result;
    }

    // d = Σ rarity(w) — ЛІНІЙНА сума (не квадрат, як у v6)
    let d: f64 = unique.iter()
        .map(|w| word_rarity(w, total_words, global_counts))
        .sum();

    // len_norm = √(|U| + δ_bias)  — B5 fix: додано δ_bias
    let len_norm = ((u_len as f64) + DELTA_BIAS).sqrt();

    // I_kw = 1 + ln(1 + kw_count)
    let mut kw_count = 0usize;
    let mut i_kw = 1.0;
    if let Some(kw) = keyword {
        let kw_lower = kw.to_lowercase();
        kw_count = cleaned_lower.matches(&kw_lower).count();
        i_kw = 1.0 + (1.0_f64 + kw_count as f64).ln();
    }

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
    let c_canon = 3.0 * canon_count as f64;
    let a_svo = 2.0 * action_count as f64;

    let epsilon = (kappa * i_kw * d + e_val + c_canon + a_svo) / len_norm;

    let theta_rel = THETA_BASE / kappa;
    let is_noise = epsilon < theta_rel;
    let is_climax = epsilon >= CLIMAX_THRESHOLD;

    EpsilonResult {
        epsilon,
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
        formula_variant: "canonical",
    }
}

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
