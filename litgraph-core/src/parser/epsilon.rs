//! Epsilon-алгоритм важности фрагмента текста (POLER v6)
//! ε = (κ × kw_intensity × d_sq + emotion) / √(unique_words)

use std::collections::{HashMap, HashSet};

/// Эмоциональные маркеры (вес = 1.5)
const EMOTIONAL_MARKERS: &[&str] = &[
    "хаос","сила","свідомість","реальність","істина","тінь","світло","темрява",
    "безодня","вічність","тиша","пам'ять","страх","надія","любов","зрада",
    "прощення","самотність","доля","свобода","вибір","правда","війна","смерть",
    "життя","кров","вогонь","біль","гнів","час","мить",
    "хаос","сила","сознание","реальность","истина","тень","свет","тьма",
    "бездна","вечность","тишина","память","страх","надежда","любовь","предательство",
    "chaos","power","consciousness","reality","truth","shadow","light","darkness",
    "abyss","eternity","silence","memory","fear","hope","love","betrayal",
    "forgiveness","loneliness","fate","freedom","choice","war","death","life",
    "blood","fire","pain","anger","time","moment",
];

/// Стоп-слово
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

#[derive(Debug, Clone)]
pub struct EpsilonResult {
    pub epsilon: f64,
    pub normalized: f64,
    pub word_count: usize,
    pub unique_words: usize,
    pub emotion_count: usize,
}

fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(&word)
}

fn is_emotional(word: &str) -> bool {
    EMOTIONAL_MARKERS.contains(&word)
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '\u{2019}')
        .filter(|t| t.len() > 2 && !is_stop_word(t))
        .map(|t| t.to_string())
        .collect()
}

fn word_rarity(word: &str, total_words: usize, counts: &HashMap<String, usize>) -> f64 {
    let count = *counts.get(word).unwrap_or(&1) as f64;
    let p = count / total_words.max(1) as f64;
    -((p.max(1e-10)).ln())
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
    let tokens = tokenize(chapter_text);
    let cleaned_lower = chapter_text.to_lowercase();

    let unique: HashSet<&str> = tokens.iter().map(|s| s.as_str()).collect();
    let d_sq: f64 = unique.iter()
        .map(|w| word_rarity(w, total_words, global_counts).powi(2))
        .sum();
    let len_norm = (unique.len() as f64).sqrt().max(1.0);

    let mut kw_count = 0;
    let mut kw_intensity = 1.0;
    if let Some(kw) = keyword {
        let kw_lower = kw.to_lowercase();
        kw_count = cleaned_lower.matches(&kw_lower).count();
        kw_intensity = 1.0 + (1.0 + kw_count as f64).ln();
    }

    let emotion_count = tokens.iter().filter(|t| is_emotional(t)).count();
    let emotion = emotion_count as f64 * 1.5;

    let epsilon = (kappa * kw_intensity * d_sq + emotion) / len_norm;

    EpsilonResult {
        epsilon,
        normalized: 0.0,
        word_count: tokens.len(),
        unique_words: unique.len(),
        emotion_count,
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
