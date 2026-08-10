//! Epsilon-алгоритм важливості фрагмента тексту (POLER v6.5 Canonical)
//! ε = (κ × kw_intensity × Σ rarity(w) + E + C_canon + A_svo) / √(unique_words + δ_bias)

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

/// Канонічні якорі світу Етерії
pub const CANON_ANCHORS: &[&str] = &[
    "етерія", "буфер", "сектор", "хмара", "геліос", "теневра", "фосфор", 
    "кассіопея", "яр", "ущелина", "аніма", "руна", "вузол", "код", "матриця",
    "інквесторат", "триада", "рада", "пропуск", "чип", "пластик", "стійбище",
    "архів", "проект", "алгоритм", "система", "редакція", "сигнал", "ток",
    "χ-оружие", "хи-оружие", "док", "причал", "буферу", "етерії", "геліоса",
];

/// SVO-активні дієслова високої каузальності
pub const ACTION_VERBS: &[&str] = &[
    "вбити", "убити", "умерти", "померти", "загинути", "застрелити", "отруїти",
    "підірвати", "зрадити", "врятувати", "визволити", "схопити", "ув'язнити",
    "поранити", "ударити", "знівечити", "підпалити", "воскреснути",
    "наказати", "примусити", "пообіцяти", "присягти", "проникнути", "зламати",
    "убить", "умереть", "погибнуть", "застрелить", "отравить", "казнить",
    "взорвать", "предать", "спасти", "освободить", "схватить", "пленить",
    "ранить", "ударить", "воскреснуть", "приказать", "заставить", "пообещать",
];

/// Емоційні маркери (вага = 1.5)
pub const EMOTIONAL_MARKERS: &[&str] = &[
    "хаос","сила","свідомість","реальність","істина","тінь","світло","темрява",
    "безодня","вічність","тиша","пам'ять","страх","надія","любов","зрада",
    "прощення","самотність","доля","свобода","вибір","правда","війна","смерть",
    "життя","кров","вогонь","біль","гнів","час","мить",
    "крик", "кричати", "жах", "плач", "плакати", "сльози", "лють", "паніка", "ненависть",
    "розчарування", "розруха", "агонія", "кривавий", "відчай",
    "ужас", "боль", "слезы", "ярость", "гнев", "паника", "ненависть", "любовь", "отчаяние",
];

/// Стоп-слова
pub const STOP_WORDS: &[&str] = &[
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormulaVariant {
    Canonical,
    Climax,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpsilonResult {
    pub epsilon: f64,
    pub normalized: f64,
    pub word_count: usize,
    pub unique_words: usize,
    pub emotion_count: usize,
    pub is_noise: bool,
    pub is_climax: bool,
    pub canon_count: usize,
    pub action_count: usize,
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

fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

fn is_emotional(word: &str) -> bool {
    EMOTIONAL_MARKERS.contains(&word)
}

/// Токенізація з нормалізацією та фільтрацією
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '\u{2019}')
        .filter(|t| t.len() > 2 && !is_stop_word(t))
        .map(|t| t.to_string())
        .collect()
}

/// Розрахунок рідкості слова rarity(w) = -log10(p_w) з гібридною обрізкою
pub fn word_rarity(word: &str, total_words: usize, counts: &HashMap<String, usize>) -> f64 {
    let clean = word.trim().to_lowercase();
    if clean.len() <= 2 {
        return 0.0;
    }
    
    let is_canon = CANON_ANCHORS.contains(&clean.as_str());
    let is_action = ACTION_VERBS.contains(&clean.as_str());
    let is_emotion = EMOTIONAL_MARKERS.contains(&clean.as_str());

    let count = *counts.get(&clean).unwrap_or(&1) as f64;
    let local_p = count / total_words.max(1) as f64;

    let global_p = if is_canon {
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

    let p_w = 0.7 * global_p + 0.3 * local_p;
    let rarity = -p_w.log10();
    rarity.min(4.5).max(0.1)
}

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
    let config = EpsilonConfig { kappa, ..Default::default() };
    let tokens = tokenize(chapter_text);
    let cleaned_lower = chapter_text.to_lowercase();

    let unique: HashSet<&str> = tokens.iter().map(|s| s.as_str()).collect();
    let u_len = unique.len();

    if u_len == 0 {
        return EpsilonResult {
            epsilon: 0.0,
            normalized: 0.0,
            word_count: 0,
            unique_words: 0,
            emotion_count: 0,
            is_noise: true,
            is_climax: false,
            canon_count: 0,
            action_count: 0,
        };
    }

    let mut kw_intensity = 1.0;
    if let Some(kw) = keyword {
        let kw_lower = kw.to_lowercase();
        let kw_count = cleaned_lower.matches(&kw_lower).count();
        kw_intensity = 1.0 + (1.0 + kw_count as f64).ln();
    }

    let mut emotion_count = 0;
    let mut canon_count = 0;
    let mut action_count = 0;
    let mut d_sum = 0.0;

    for w in &unique {
        let r = word_rarity(w, total_words, global_counts);
        d_sum += r;

        if is_emotional(w) {
            emotion_count += 1;
        }
        if CANON_ANCHORS.contains(w) {
            canon_count += 1;
        }
        if ACTION_VERBS.contains(w) {
            action_count += 1;
        }
    }

    let e_val = 1.5 * (emotion_count as f64);
    let c_canon = 3.0 * (canon_count as f64);
    let a_svo = 2.0 * (action_count as f64);

    let len_norm = ((u_len as f64) + config.delta_bias).sqrt();
    let epsilon = (config.kappa * kw_intensity * d_sum + e_val + c_canon + a_svo) / len_norm;

    let theta_rel = config.effective_theta_rel();
    let is_noise = epsilon < theta_rel;
    let is_climax = epsilon >= config.theta_climax;

    EpsilonResult {
        epsilon,
        normalized: 0.0,
        word_count: tokens.len(),
        unique_words: u_len,
        emotion_count,
        is_noise,
        is_climax,
        canon_count,
        action_count,
    }
}

pub fn normalize_epsilons(results: &mut [EpsilonResult]) {
    if results.is_empty() { return; }
    let max_eps = results.iter().map(|r| r.epsilon).fold(0.0_f64, f64::max);
    if max_eps <= 0.0 { return; }
    for r in results.iter_mut() {
        r.normalized = (r.epsilon / max_eps) * 100.0;
    }
}
