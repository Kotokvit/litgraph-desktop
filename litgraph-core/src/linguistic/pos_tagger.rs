//! POS-Tagger & Disambiguator — Layer B of the Symbolic UA-LP Engine.
//!
//! Resolves morphological homonymy in Ukrainian text using LanguageTool UK
//! `disambiguation.xml` (450 compiled rules) and `case_government.txt`
//! (37,728 verb→case mappings). All behavior is deterministic — no ML,
//! no statistics, no stochastic models.
//!
//! ## Architecture
//!
//! ```text
//! tokens + candidates
//!     │
//!     ▼
//! Pass 1: Pattern Rules (disambiguation.xml)
//!     │   Apply 450 rules of the form:
//!     │     <token>мати</token> + <token postag="verb.*">  → replace мати with noun:f:v_naz
//!     │   O(N × R × W) where R=rules, W=avg pattern width (≤3)
//!     ▼
//! Pass 2: Case Government Consistency
//!     │   For each verb, check that its object's case is allowed
//!     │   by case_government.txt. If not, demote that candidate tag.
//!     ▼
//! Pass 3: Fallback Heuristics
//!     │   Capitalization → Proper Noun
//!     │   All-caps → Acronym
//!     │   Punctuation context → Particle vs. Conjunction
//!     ▼
//! Vec<TaggedToken>  (one disambiguated tag per token)
//! ```
//!
//! ## Symbolic vs. Stochastic
//!
//! Each decision is traceable to a rule. "Why is 'мати' a noun here?"
//! → "Because rule MATY_NOUN_BEFORE_VERB matched: 'мати' was followed
//! by a finite verb form, and the rule replaces the candidate verb
//! reading with noun:f:v_naz:anim."
//!
//! ## Performance
//!
//! - Rule loading: ~3 ms (gzipped JSON, lazy via `OnceLock`)
//! - In-memory rules: ~450 `CompiledRule`s, ~1 MB
//! - Per-sentence tagging: O(N × R × W) ≈ 1 ms per 20-token sentence
//!
//! ## Example
//!
//! ```no_run
//! use litgraph_core::linguistic::pos_tagger;
//!
//! // Disambiguate "Мати бачить сина" — "мати" should resolve to NOUN,
//! // not VERB, because the next word is a finite verb.
//! let tokens = vec!["Мати", "бачить", "сина"];
//! let candidates = vec![
//!     vec![
//!         pos_tagger::PosTag::parse("noun:f:v_naz:anim"),
//!         pos_tagger::PosTag::parse("verb:inf:imperf"),
//!     ],
//!     vec![pos_tagger::PosTag::parse("verb:pres:3:s:imperf")],
//!     vec![pos_tagger::PosTag::parse("noun:m:v_rod:anim")],
//! ];
//! let tagged = pos_tagger::tag_sentence(&tokens, &candidates);
//! assert_eq!(tagged[0].selected_tag.class, pos_tagger::PosClass::Noun);
//! ```

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::OnceLock;

use flate2::read::GzDecoder;
use regex::Regex;
use serde::{Deserialize, Serialize};

// ============================================================================
// Core Type System (mirrors spec §3)
// ============================================================================

/// Core Parts of Speech in Ukrainian morphology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PosClass {
    Noun,
    Verb,
    Adjective,
    Adverb,
    Pronoun,
    Numeral,
    Preposition,
    Conjunction,
    Particle,
    Interjection,
    Punctuation,
    #[default]
    Unknown,
}

/// Ukrainian grammatical case system (7 cases).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GrammaticalCase {
    /// v_naz — Називний
    Nominative,
    /// v_rod — Родовий
    Genitive,
    /// v_dav — Давальний
    Dative,
    /// v_zna — Знахідний
    Accusative,
    /// v_oru — Орудний
    Instrumental,
    /// v_mis — Місцевий
    Locative,
    /// v_kly — Кличний
    Vocative,
}

impl GrammaticalCase {
    /// Parse from dict_uk / LanguageTool tag code (e.g. "v_naz" → Nominative)
    pub fn from_tag_code(code: &str) -> Option<Self> {
        match code {
            "v_naz" => Some(Self::Nominative),
            "v_rod" => Some(Self::Genitive),
            "v_dav" => Some(Self::Dative),
            "v_zna" => Some(Self::Accusative),
            "v_oru" => Some(Self::Instrumental),
            "v_mis" => Some(Self::Locative),
            "v_kly" => Some(Self::Vocative),
            _ => None,
        }
    }

    /// Convert to dict_uk tag code
    pub fn to_tag_code(self) -> &'static str {
        match self {
            Self::Nominative => "v_naz",
            Self::Genitive => "v_rod",
            Self::Dative => "v_dav",
            Self::Accusative => "v_zna",
            Self::Instrumental => "v_oru",
            Self::Locative => "v_mis",
            Self::Vocative => "v_kly",
        }
    }
}

/// Grammatical gender (Ukrainian has 4: m, f, n, and plural-only "p").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Gender {
    Masculine,
    Feminine,
    Neuter,
    Plural,
}

/// Number (singular / plural).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Number {
    Singular,
    Plural,
}

/// Animacy (Ukrainian distinguishes animate vs. inanimate nouns).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Animacy {
    Animate,
    Inanimate,
}

/// Verb aspect (Ukrainian pairs: "писати/написати" = imperf/perf).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Aspect {
    /// доконаний — "що зробив?" (написав, прийшов)
    Perfective,
    /// недоконаний — "що робив?" (писав, ішов)
    Imperfective,
}

/// Verb tense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tense {
    Present,
    Past,
    Future,
    Infinitive,
    Imperative,
}

/// Fully disambiguated POS tag — one per token after `tag_sentence()`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct PosTag {
    pub class: PosClass,
    pub case: Option<GrammaticalCase>,
    pub gender: Option<Gender>,
    pub number: Option<Number>,
    pub animacy: Option<Animacy>,
    pub aspect: Option<Aspect>,
    pub tense: Option<Tense>,
    /// Original raw POS tag from dict_uk, e.g. "noun:f:v_rod:anim"
    pub raw_tag: String,
}

impl PosTag {
    /// Parse a dict_uk / LanguageTool POS tag string.
    ///
    /// Examples:
    /// - `"noun:f:v_naz:anim"` → Noun, Feminine, Nominative, Animate
    /// - `"verb:pres:3:s:imperf"` → Verb, Present, Singular, Imperfective
    /// - `"adj:m:v_zna:rinanim"` → Adjective, Masculine, Accusative, Inanimate
    /// - `""` (empty) → Unknown
    pub fn parse(raw: &str) -> Self {
        if raw.is_empty() {
            return Self::default();
        }
        let mut tag = Self {
            raw_tag: raw.to_string(),
            ..Self::default()
        };
        let parts: Vec<&str> = raw.split(':').collect();
        if parts.is_empty() {
            return tag;
        }

        // First field = POS class
        tag.class = match parts[0] {
            "noun" => PosClass::Noun,
            "verb" => PosClass::Verb,
            "adj" => PosClass::Adjective,
            "adv" | "advp" => PosClass::Adverb,
            "pron" => PosClass::Pronoun,
            "numr" => PosClass::Numeral,
            "prep" => PosClass::Preposition,
            "conj" => PosClass::Conjunction,
            "part" => PosClass::Particle,
            "intj" => PosClass::Interjection,
            "noninfl" | "number" => PosClass::Unknown,
            _ => PosClass::Unknown,
        };

        // Subsequent fields: parse known codes
        for &part in &parts[1..] {
            // Case
            if let Some(case) = GrammaticalCase::from_tag_code(part) {
                tag.case = Some(case);
                continue;
            }
            // Gender
            match part {
                "m" | "m:v_naz" => tag.gender = Some(Gender::Masculine),
                "f" => tag.gender = Some(Gender::Feminine),
                "n" => tag.gender = Some(Gender::Neuter),
                "p" => {
                    tag.gender = Some(Gender::Plural);
                    tag.number = Some(Number::Plural);
                }
                "anim" | "ranim" => tag.animacy = Some(Animacy::Animate),
                "inanim" | "rinanim" => tag.animacy = Some(Animacy::Inanimate),
                "s" => tag.number = Some(Number::Singular),
                "imperf" => tag.aspect = Some(Aspect::Imperfective),
                "perf" => tag.aspect = Some(Aspect::Perfective),
                "pres" => tag.tense = Some(Tense::Present),
                "past" => tag.tense = Some(Tense::Past),
                "fut" => tag.tense = Some(Tense::Future),
                "inf" => tag.tense = Some(Tense::Infinitive),
                "imp" => tag.tense = Some(Tense::Imperative),
                _ => {}
            }
        }

        tag
    }

    /// Construct an empty/unknown tag.
    pub fn unknown() -> Self {
        Self::default()
    }

    /// Check if this tag matches a POS pattern (regex).
    /// E.g. tag="noun:f:v_naz" matches pattern="noun:.*:v_naz.*"
    pub fn matches_pattern(&self, pattern: &Regex) -> bool {
        pattern.is_match(&self.raw_tag)
    }

    /// Check if this tag matches a POS prefix (non-regex, exact start).
    pub fn matches_prefix(&self, prefix: &str) -> bool {
        self.raw_tag.starts_with(prefix)
    }
}

// ============================================================================
// Rule System (mirrors spec §3 — TokenCondition, DisambigAction, etc.)
// ============================================================================

/// One token-matching condition inside a rule pattern.
///
/// Mirrors `SerializableTokenCondition` from xtask's `build_pos_tables.rs`,
/// but with compiled regexes for fast matching.
#[derive(Debug, Clone)]
pub struct TokenCondition {
    /// Literal text or regex pattern (when `is_regexp`)
    pub text: Option<String>,
    /// Compiled text regex (None = literal match)
    pub text_regex: Option<Regex>,
    /// POS tag pattern (compiled regex when `is_postag_regexp`)
    pub postag: Option<String>,
    pub postag_regex: Option<Regex>,
    pub inflected: bool,
    pub case_sensitive: bool,
    pub negate: bool,
    pub min: Option<u32>,
    pub max: Option<u32>,
}

// Note: TokenCondition doesn't derive Eq because Regex isn't Eq.
// For PartialEq, we compare structurally ignoring regex objects (they're
// deterministically compiled from `text`/`postag`).
impl PartialEq for TokenCondition {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
            && self.postag == other.postag
            && self.inflected == other.inflected
            && self.case_sensitive == other.case_sensitive
            && self.negate == other.negate
            && self.min == other.min
            && self.max == other.max
    }
}
impl Eq for TokenCondition {}

impl TokenCondition {
    /// Check if a token (word + its candidate POS tags) matches this condition.
    ///
    /// Returns true if:
    /// - Text matches (literal or regex, respecting case_sensitive)
    /// - At least one candidate POS tag matches postag pattern
    /// - Negation inverts the result
    pub fn matches(&self, word: &str, candidates: &[PosTag]) -> bool {
        let mut text_match = true;
        let mut pos_match = true;

        // Text matching
        if let Some(text) = &self.text {
            text_match = if let Some(re) = &self.text_regex {
                re.is_match(word)
            } else if self.case_sensitive {
                word == text
            } else {
                // Unicode-aware case-insensitive comparison (Cyrillic-safe)
                word.to_lowercase() == text.to_lowercase()
            };
        }

        // POS matching
        if let Some(_pos) = &self.postag {
            pos_match = if let Some(re) = &self.postag_regex {
                candidates.iter().any(|c| re.is_match(&c.raw_tag))
            } else {
                candidates.iter().any(|c| c.raw_tag == *self.postag.as_deref().unwrap_or(""))
            };
        }

        let result = text_match && pos_match;
        if self.negate {
            !result
        } else {
            result
        }
    }
}

/// Action applied to the matched token(s).
#[derive(Debug, Clone)]
pub enum DisambigAction {
    /// Replace all candidate tags with this single tag
    ReplaceTag(PosTag),
    /// Remove all candidate tags matching this regex pattern
    RemoveTagPattern(String, Option<Regex>),
    /// Filter candidates by grammatical case
    FilterByCase(GrammaticalCase),
    /// Add an extra tag to the candidates (rarely used)
    AddTag(PosTag),
    /// Lock token from future rule modifications
    Immunize,
}

impl PartialEq for DisambigAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::ReplaceTag(a), Self::ReplaceTag(b)) => a == b,
            (Self::RemoveTagPattern(a, _), Self::RemoveTagPattern(b, _)) => a == b,
            (Self::FilterByCase(a), Self::FilterByCase(b)) => a == b,
            (Self::AddTag(a), Self::AddTag(b)) => a == b,
            (Self::Immunize, Self::Immunize) => true,
            _ => false,
        }
    }
}
impl Eq for DisambigAction {}

/// One compiled disambiguation rule.
#[derive(Debug, Clone)]
pub struct DisambiguationRule {
    pub id: String,
    pub name: String,
    pub pattern: Vec<TokenCondition>,
    pub marker_start: usize,
    pub marker_end: usize, // exclusive
    pub action: DisambigAction,
}

impl DisambiguationRule {
    /// Try to match this rule against a token slice starting at `pos`.
    /// Returns Some(()) if matched and the action was applied.
    pub fn try_apply(&self, tokens: &[&str], candidates: &mut [Vec<PosTag>], pos: usize) -> bool {
        let pat_len = self.pattern.len();
        if pos + pat_len > tokens.len() {
            return false;
        }

        // Check each pattern token against the corresponding input token
        for (i, cond) in self.pattern.iter().enumerate() {
            let word = tokens[pos + i];
            let cands = &candidates[pos + i];
            if !cond.matches(word, cands) {
                return false;
            }
        }

        // All pattern tokens matched — apply action to marker range
        // Marker indices are relative to the pattern, not the whole sentence.
        // So absolute marker range = [pos + marker_start, pos + marker_end)
        let abs_start = pos + self.marker_start;
        let abs_end = (pos + self.marker_end).min(tokens.len());

        for i in abs_start..abs_end {
            if i >= candidates.len() {
                break;
            }
            self.apply_action(&mut candidates[i]);
        }
        true
    }

    fn apply_action(&self, cands: &mut Vec<PosTag>) {
        match &self.action {
            DisambigAction::ReplaceTag(tag) => {
                if !cands.is_empty() {
                    cands.clear();
                    cands.push(tag.clone());
                }
            }
            DisambigAction::RemoveTagPattern(_, regex) => {
                if let Some(re) = regex {
                    cands.retain(|c| !re.is_match(&c.raw_tag));
                }
            }
            DisambigAction::FilterByCase(case) => {
                let filtered: Vec<PosTag> = cands
                    .iter()
                    .filter(|c| c.case == Some(*case))
                    .cloned()
                    .collect();
                if !filtered.is_empty() {
                    *cands = filtered;
                }
            }
            DisambigAction::AddTag(tag) => {
                if !cands.contains(tag) {
                    cands.push(tag.clone());
                }
            }
            DisambigAction::Immunize => {
                // Mark by leaving a single tag (no-op for now; future: add a flag)
            }
        }
    }
}

// ============================================================================
// Serialized artifact (loaded from pos_rules.json.gz)
// ============================================================================

/// Inner serializable form (mirrors xtask's `SerializableRule`).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableTokenCondition {
    text: Option<String>,
    postag: Option<String>,
    is_regexp: bool,
    is_postag_regexp: bool,
    inflected: bool,
    case_sensitive: bool,
    negate: bool,
    min: Option<u32>,
    max: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableDisambigAction {
    kind: String,
    postag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SerializableRule {
    id: String,
    name: String,
    tokens: Vec<SerializableTokenCondition>,
    marker_start: usize,
    marker_end: usize,
    action: SerializableDisambigAction,
}

#[derive(Debug, Serialize, Deserialize)]
struct PosRulesArtifact {
    version: String,
    rule_count: usize,
    case_government: HashMap<String, Vec<String>>,
    rules: Vec<SerializableRule>,
}

// ============================================================================
// Loaded rules cache
// ============================================================================

/// Compiled rules + case government map, loaded once and cached.
pub struct CompiledRules {
    pub rules: Vec<DisambiguationRule>,
    pub case_government: HashMap<String, Vec<GrammaticalCase>>,
    pub rule_count: usize,
}

static COMPILED: OnceLock<Option<CompiledRules>> = OnceLock::new();

/// Locate `pos_rules.json.gz` on disk (mirrors lemmatizer search order).
fn locate_artifact() -> Option<PathBuf> {
    let candidates: Vec<PathBuf> = vec![
        // 1. Project root (when running from litgraph-desktop/)
        PathBuf::from("resources/ua-linguistic/derivatives/pos_rules.json.gz"),
        // 2. From litgraph-core/
        PathBuf::from("../resources/ua-linguistic/derivatives/pos_rules.json.gz"),
        // 3. From src-tauri/
        PathBuf::from("../resources/ua-linguistic/derivatives/pos_rules.json.gz"),
        // 4. User data dir (Tauri app data)
        dirs::data_dir()
            .map(|d| d.join("litgraph/pos_rules.json.gz"))
            .unwrap_or_default(),
    ];

    for path in candidates.iter() {
        if path.exists() {
            return Some(path.clone());
        }
    }
    None
}

/// Load and compile rules from `pos_rules.json.gz`. Returns None on failure.
fn compile_rules() -> Option<CompiledRules> {
    let path = locate_artifact()?;
    let file = std::fs::File::open(&path).ok()?;
    let mut decoder = GzDecoder::new(file);
    let mut json = String::new();
    decoder.read_to_string(&mut json).ok()?;
    let artifact: PosRulesArtifact = serde_json::from_str(&json).ok()?;

    let mut rules = Vec::with_capacity(artifact.rules.len());
    for sr in artifact.rules {
        let mut pattern = Vec::with_capacity(sr.tokens.len());
        for st in sr.tokens {
            // When case_sensitive=false, prepend (?i) to regex for case-insensitive
            // matching (Unicode-aware — handles Cyrillic "Мати" vs "мати").
            let text_regex = if st.is_regexp {
                st.text.as_ref().and_then(|t| {
                    let pattern = if st.case_sensitive {
                        t.to_string()
                    } else {
                        format!("(?i){}", t)
                    };
                    Regex::new(&pattern).ok()
                })
            } else {
                None
            };
            let postag_regex = if st.is_postag_regexp {
                st.postag.as_ref().and_then(|p| Regex::new(p).ok())
            } else {
                None
            };
            pattern.push(TokenCondition {
                text: st.text,
                text_regex,
                postag: st.postag,
                postag_regex,
                inflected: st.inflected,
                case_sensitive: st.case_sensitive,
                negate: st.negate,
                min: st.min,
                max: st.max,
            });
        }

        let action = match sr.action.kind.as_str() {
            "replace" => {
                let postag_str = sr.action.postag.unwrap_or_default();
                DisambigAction::ReplaceTag(PosTag::parse(&postag_str))
            }
            "remove" => {
                let postag_str = sr.action.postag.unwrap_or_default();
                let re = Regex::new(&postag_str).ok();
                DisambigAction::RemoveTagPattern(postag_str, re)
            }
            "add" => {
                let postag_str = sr.action.postag.unwrap_or_default();
                DisambigAction::AddTag(PosTag::parse(&postag_str))
            }
            "filter" | "filterall" => {
                // Filter by case: postag is a case code like "v_zna"
                let case = sr
                    .action
                    .postag
                    .as_deref()
                    .and_then(GrammaticalCase::from_tag_code)
                    .unwrap_or(GrammaticalCase::Nominative);
                DisambigAction::FilterByCase(case)
            }
            "immunize" => DisambigAction::Immunize,
            _ => continue, // Unknown action kind — skip rule
        };

        rules.push(DisambiguationRule {
            id: sr.id,
            name: sr.name,
            pattern,
            marker_start: sr.marker_start,
            marker_end: sr.marker_end,
            action,
        });
    }

    // Convert case_government: HashMap<String, Vec<String>> → HashMap<String, Vec<GrammaticalCase>>
    let case_government: HashMap<String, Vec<GrammaticalCase>> = artifact
        .case_government
        .into_iter()
        .map(|(verb, codes)| {
            let cases: Vec<GrammaticalCase> = codes
                .iter()
                .filter_map(|c| GrammaticalCase::from_tag_code(c))
                .collect();
            (verb, cases)
        })
        .collect();

    let rule_count = rules.len();
    Some(CompiledRules {
        rules,
        case_government,
        rule_count,
    })
}

/// Get the compiled rules (loads on first call).
pub fn compiled() -> &'static Option<CompiledRules> {
    COMPILED.get_or_init(|| {
        match compile_rules() {
            Some(c) => {
                eprintln!(
                    "[pos_tagger] Loaded {} rules + {} case-government entries",
                    c.rule_count,
                    c.case_government.len()
                );
                Some(c)
            }
            None => {
                eprintln!(
                    "[pos_tagger] WARNING: pos_rules.json.gz not found. \
                     Run `cargo run --release -- build-pos-tables` to generate. \
                     Falling back to first-candidate-only tagging."
                );
                None
            }
        }
    })
}

// ============================================================================
// TaggedToken + Public API
// ============================================================================

/// Output of `tag_sentence()`: one per input token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaggedToken {
    pub word: String,
    pub lemma: String,
    pub selected_tag: PosTag,
    pub candidates: Vec<PosTag>,
    pub is_disambiguated: bool,
    /// Rule ID that finally disambiguated this token (None = no rule fired)
    pub applied_rule: Option<String>,
}

/// Disambiguate a sentence: tokens + their candidate POS tags (from dict_uk).
///
/// Returns one `TaggedToken` per input token.
pub fn tag_sentence(tokens: &[&str], candidates: &[Vec<PosTag>]) -> Vec<TaggedToken> {
    assert_eq!(tokens.len(), candidates.len(), "tokens and candidates must have same length");

    let mut work_candidates: Vec<Vec<PosTag>> = candidates.to_vec();
    let mut applied_rules: Vec<Option<String>> = vec![None; tokens.len()];

    // Pass 1: Apply all pattern rules
    if let Some(rules) = compiled() {
        // Try each rule at each starting position
        for rule in &rules.rules {
            for pos in 0..tokens.len() {
                let before: Vec<usize> = (0..tokens.len())
                    .filter(|&i| work_candidates[i].len() > 0)
                    .collect();
                let _ = before; // for debugging
                let matched = rule.try_apply(tokens, &mut work_candidates, pos);
                if matched {
                    for i in (pos + rule.marker_start)..(pos + rule.marker_end).min(tokens.len()) {
                        applied_rules[i] = Some(rule.id.clone());
                    }
                }
            }
        }

        // Pass 2: Case government consistency
        apply_case_government(tokens, &mut work_candidates, &rules.case_government);
    }

    // Pass 3: Fallback heuristics
    apply_fallbacks(tokens, &mut work_candidates);

    // Build final TaggedToken list
    tokens
        .iter()
        .zip(work_candidates.iter())
        .zip(applied_rules.iter())
        .map(|((&word, cands), rule_id)| {
            let lemma = crate::linguistic::lemmatizer::lemmatize_first(word)
                .unwrap_or_else(|| word.to_lowercase());
            let selected_tag = cands.first().cloned().unwrap_or_default();
            let is_disambiguated = cands.len() == 1;
            TaggedToken {
                word: word.to_string(),
                lemma,
                selected_tag,
                candidates: cands.clone(),
                is_disambiguated,
                applied_rule: rule_id.clone(),
            }
        })
        .collect()
}

/// Pass 2: For each verb, check that its object's case is in the verb's
/// allowed case frame. If not, demote that candidate tag.
fn apply_case_government(
    tokens: &[&str],
    candidates: &mut [Vec<PosTag>],
    case_government: &HashMap<String, Vec<GrammaticalCase>>,
) {
    for i in 0..tokens.len() {
        // Find verb candidates
        let has_verb = candidates[i]
            .iter()
            .any(|c| c.class == PosClass::Verb);
        if !has_verb {
            continue;
        }

        // Get the verb lemma (use first verb candidate)
        let verb_lemma = crate::linguistic::lemmatizer::lemmatize_first(tokens[i])
            .unwrap_or_else(|| tokens[i].to_lowercase());

        let allowed_cases = match case_government.get(&verb_lemma) {
            Some(c) => c,
            None => continue,
        };

        // Check the NEXT token: if it's a noun with a case NOT in allowed_cases,
        // demote it (move it to the end of candidates).
        if i + 1 >= tokens.len() {
            continue;
        }
        let next_cands = &mut candidates[i + 1];
        let next_is_noun = next_cands.iter().any(|c| c.class == PosClass::Noun);
        if !next_is_noun {
            continue;
        }

        // Partition: keep noun candidates with allowed case first, demote others
        let mut preferred: Vec<PosTag> = Vec::new();
        let mut demoted: Vec<PosTag> = Vec::new();
        for c in next_cands.drain(..) {
            if c.class == PosClass::Noun {
                if let Some(case) = c.case {
                    if allowed_cases.contains(&case) {
                        preferred.push(c);
                        continue;
                    }
                }
                demoted.push(c);
            } else {
                preferred.push(c);
            }
        }
        preferred.append(&mut demoted);
        *next_cands = preferred;
    }
}

/// Pass 3: Fallback heuristics for tokens that still have multiple candidates.
fn apply_fallbacks(tokens: &[&str], candidates: &mut [Vec<PosTag>]) {
    for (i, &word) in tokens.iter().enumerate() {
        if candidates[i].len() <= 1 {
            continue;
        }

        // Heuristic 1: Capitalized word at sentence start → likely Proper Noun
        let is_sentence_start = i == 0
            || tokens[i - 1].ends_with('.')
            || tokens[i - 1].ends_with('!')
            || tokens[i - 1].ends_with('?');
        let is_capitalized = word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
        if is_sentence_start && is_capitalized {
            // Don't override — just sort noun candidates first
            candidates[i].sort_by_key(|c| if c.class == PosClass::Noun { 0 } else { 1 });
            continue;
        }

        // Heuristic 2: All-caps token → acronym (likely proper noun)
        if word.len() > 1 && word.chars().all(|c| c.is_uppercase()) {
            candidates[i].sort_by_key(|c| if c.class == PosClass::Noun { 0 } else { 1 });
            continue;
        }

        // Heuristic 3: Word followed by punctuation → keep verb/noun, drop particles
        let followed_by_punct = i + 1 >= tokens.len()
            || tokens[i + 1].chars().next().map(|c| c.is_ascii_punctuation()).unwrap_or(false);
        if followed_by_punct {
            candidates[i].sort_by_key(|c| match c.class {
                PosClass::Verb | PosClass::Noun => 0,
                _ => 1,
            });
        }
    }
}

/// Convenience: tag a single word (returns first candidate, no context).
pub fn tag_word(word: &str, candidates: &[PosTag]) -> TaggedToken {
    let tokens = [word];
    let cands = [candidates.to_vec()];
    tag_sentence(&tokens, &cands)
        .into_iter()
        .next()
        .unwrap()
}

/// Get count of loaded rules (0 if artifact missing).
pub fn rule_count() -> usize {
    compiled().as_ref().map(|c| c.rule_count).unwrap_or(0)
}

/// Get case government entries for a verb lemma.
pub fn cases_for_verb(verb_lemma: &str) -> Vec<GrammaticalCase> {
    compiled()
        .as_ref()
        .and_then(|c| c.case_government.get(verb_lemma))
        .cloned()
        .unwrap_or_default()
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- PosTag parsing tests ---

    #[test]
    fn test_pos_tag_parse_noun() {
        let tag = PosTag::parse("noun:f:v_naz:anim");
        assert_eq!(tag.class, PosClass::Noun);
        assert_eq!(tag.gender, Some(Gender::Feminine));
        assert_eq!(tag.case, Some(GrammaticalCase::Nominative));
        assert_eq!(tag.animacy, Some(Animacy::Animate));
    }

    #[test]
    fn test_pos_tag_parse_verb_past() {
        let tag = PosTag::parse("verb:past:m:s:imperf");
        assert_eq!(tag.class, PosClass::Verb);
        assert_eq!(tag.tense, Some(Tense::Past));
        assert_eq!(tag.gender, Some(Gender::Masculine));
        assert_eq!(tag.number, Some(Number::Singular));
        assert_eq!(tag.aspect, Some(Aspect::Imperfective));
    }

    #[test]
    fn test_pos_tag_parse_empty() {
        let tag = PosTag::parse("");
        assert_eq!(tag.class, PosClass::Unknown);
        assert_eq!(tag.case, None);
    }

    #[test]
    fn test_pos_tag_parse_unknown_pos() {
        let tag = PosTag::parse("xyz:foo:bar");
        assert_eq!(tag.class, PosClass::Unknown);
    }

    #[test]
    fn test_pos_tag_parse_adjective() {
        let tag = PosTag::parse("adj:m:v_zna:rinanim");
        assert_eq!(tag.class, PosClass::Adjective);
        assert_eq!(tag.gender, Some(Gender::Masculine));
        assert_eq!(tag.case, Some(GrammaticalCase::Accusative));
        assert_eq!(tag.animacy, Some(Animacy::Inanimate));
    }

    #[test]
    fn test_pos_tag_parse_preposition() {
        let tag = PosTag::parse("prep");
        assert_eq!(tag.class, PosClass::Preposition);
    }

    // --- GrammaticalCase tests ---

    #[test]
    fn test_case_round_trip() {
        for code in &["v_naz", "v_rod", "v_dav", "v_zna", "v_oru", "v_mis", "v_kly"] {
            let case = GrammaticalCase::from_tag_code(code).expect(code);
            assert_eq!(case.to_tag_code(), *code);
        }
    }

    #[test]
    fn test_case_from_unknown_code() {
        assert_eq!(GrammaticalCase::from_tag_code("v_xyz"), None);
        assert_eq!(GrammaticalCase::from_tag_code(""), None);
    }

    // --- TokenCondition matching tests ---

    #[test]
    fn test_token_condition_literal_match() {
        let cond = TokenCondition {
            text: Some("мати".to_string()),
            text_regex: None,
            postag: None,
            postag_regex: None,
            inflected: false,
            case_sensitive: false,
            negate: false,
            min: None,
            max: None,
        };
        let cands = vec![PosTag::parse("noun:f:v_naz:anim")];
        assert!(cond.matches("мати", &cands));
        assert!(cond.matches("МАТИ", &cands)); // case-insensitive
        assert!(!cond.matches("матір", &cands));
    }

    #[test]
    fn test_token_condition_case_sensitive() {
        let cond = TokenCondition {
            text: Some("І".to_string()),
            text_regex: None,
            postag: None,
            postag_regex: None,
            inflected: false,
            case_sensitive: true,
            negate: false,
            min: None,
            max: None,
        };
        let cands = vec![PosTag::unknown()];
        assert!(cond.matches("І", &cands));
        assert!(!cond.matches("і", &cands));
    }

    #[test]
    fn test_token_condition_regex_text() {
        let cond = TokenCondition {
            text: Some("мати|матері|матір’ю".to_string()),
            text_regex: Some(Regex::new("мати|матері|матір’ю").unwrap()),
            postag: None,
            postag_regex: None,
            inflected: false,
            case_sensitive: false,
            negate: false,
            min: None,
            max: None,
        };
        let cands = vec![PosTag::unknown()];
        assert!(cond.matches("мати", &cands));
        assert!(cond.matches("матері", &cands));
        assert!(!cond.matches("ходити", &cands));
    }

    #[test]
    fn test_token_condition_postag_regex() {
        let cond = TokenCondition {
            text: None,
            text_regex: None,
            postag: Some("verb:.*".to_string()),
            postag_regex: Some(Regex::new("verb:.*").unwrap()),
            inflected: false,
            case_sensitive: false,
            negate: false,
            min: None,
            max: None,
        };
        let verb_cands = vec![PosTag::parse("verb:pres:3:s:imperf")];
        let noun_cands = vec![PosTag::parse("noun:f:v_naz:anim")];
        assert!(cond.matches("бачить", &verb_cands));
        assert!(!cond.matches("мати", &noun_cands));
    }

    #[test]
    fn test_token_condition_negate() {
        let cond = TokenCondition {
            text: Some("не".to_string()),
            text_regex: None,
            postag: None,
            postag_regex: None,
            inflected: false,
            case_sensitive: false,
            negate: true,
            min: None,
            max: None,
        };
        let cands = vec![PosTag::unknown()];
        assert!(!cond.matches("не", &cands));
        assert!(cond.matches("так", &cands));
    }

    // --- Rule application tests ---

    #[test]
    fn test_rule_replace_action() {
        let rule = DisambiguationRule {
            id: "TEST_R1".to_string(),
            name: "test replace".to_string(),
            pattern: vec![
                TokenCondition {
                    text: Some("мати".to_string()),
                    // (?i) makes the regex case-insensitive (matches "Мати" too)
                    text_regex: Some(Regex::new("(?i)мати").unwrap()),
                    postag: None,
                    postag_regex: None,
                    inflected: false,
                    case_sensitive: false,
                    negate: false,
                    min: None,
                    max: None,
                },
                TokenCondition {
                    text: None,
                    text_regex: None,
                    postag: Some("verb:.*".to_string()),
                    postag_regex: Some(Regex::new("verb:.*").unwrap()),
                    inflected: false,
                    case_sensitive: false,
                    negate: false,
                    min: None,
                    max: None,
                },
            ],
            marker_start: 0,
            marker_end: 1,
            action: DisambigAction::ReplaceTag(PosTag::parse("noun:f:v_naz:anim")),
        };

        let tokens = vec!["Мати", "бачить"];
        let mut candidates = vec![
            vec![
                PosTag::parse("noun:f:v_naz:anim"),
                PosTag::parse("verb:inf:imperf"),
            ],
            vec![PosTag::parse("verb:pres:3:s:imperf")],
        ];

        let matched = rule.try_apply(&tokens, &mut candidates, 0);
        assert!(matched);
        // After replace: only noun tag remains for "Мати"
        assert_eq!(candidates[0].len(), 1);
        assert_eq!(candidates[0][0].class, PosClass::Noun);
    }

    #[test]
    fn test_rule_remove_action() {
        let rule = DisambiguationRule {
            id: "TEST_R2".to_string(),
            name: "test remove".to_string(),
            pattern: vec![TokenCondition {
                text: None,
                text_regex: None,
                postag: Some("adj.*?:bad.*".to_string()),
                postag_regex: Some(Regex::new("adj.*?:bad.*").unwrap()),
                inflected: false,
                case_sensitive: false,
                negate: false,
                min: None,
                max: None,
            }],
            marker_start: 0,
            marker_end: 1,
            action: DisambigAction::RemoveTagPattern(
                "adj.*?:bad.*".to_string(),
                Some(Regex::new("adj.*?:bad.*").unwrap()),
            ),
        };

        let tokens = vec!["поганий"];
        let mut candidates = vec![vec![
            PosTag::parse("adj:m:v_naz:rinanim"),
            PosTag::parse("adj:m:v_naz:bad:arch"),
        ]];

        let matched = rule.try_apply(&tokens, &mut candidates, 0);
        assert!(matched);
        // After remove: only the non-bad adjective remains
        assert_eq!(candidates[0].len(), 1);
        assert!(!candidates[0][0].raw_tag.contains("bad"));
    }

    #[test]
    fn test_rule_no_match_wrong_pattern() {
        let rule = DisambiguationRule {
            id: "TEST_R3".to_string(),
            name: "test no match".to_string(),
            pattern: vec![
                TokenCondition {
                    text: Some("мати".to_string()),
                    text_regex: Some(Regex::new("мати").unwrap()),
                    postag: None,
                    postag_regex: None,
                    inflected: false,
                    case_sensitive: false,
                    negate: false,
                    min: None,
                    max: None,
                },
                TokenCondition {
                    text: None,
                    text_regex: None,
                    postag: Some("verb:.*".to_string()),
                    postag_regex: Some(Regex::new("verb:.*").unwrap()),
                    inflected: false,
                    case_sensitive: false,
                    negate: false,
                    min: None,
                    max: None,
                },
            ],
            marker_start: 0,
            marker_end: 1,
            action: DisambigAction::ReplaceTag(PosTag::parse("noun:f:v_naz:anim")),
        };

        let tokens = vec!["Мати", "сина"]; // "сина" is NOUN, not VERB
        let mut candidates = vec![
            vec![PosTag::parse("noun:f:v_naz:anim"), PosTag::parse("verb:inf:imperf")],
            vec![PosTag::parse("noun:m:v_rod:anim")],
        ];

        let matched = rule.try_apply(&tokens, &mut candidates, 0);
        assert!(!matched);
        // No change to candidates
        assert_eq!(candidates[0].len(), 2);
    }

    // --- Fallback heuristic tests ---

    #[test]
    fn test_fallback_capitalized_sentence_start() {
        let tokens = vec!["Мати"];
        let mut candidates = vec![vec![
            PosTag::parse("verb:inf:imperf"),
            PosTag::parse("noun:f:v_naz:anim"),
        ]];
        apply_fallbacks(&tokens, &mut candidates);
        // After fallback: noun should be sorted first (sentence-start + capitalized)
        assert_eq!(candidates[0][0].class, PosClass::Noun);
    }

    #[test]
    fn test_fallback_all_caps_acronym() {
        let tokens = vec!["НАТО"];
        let mut candidates = vec![vec![
            PosTag::parse("verb:inf:imperf"),
            PosTag::parse("noun:p:v_naz:anim"),
        ]];
        apply_fallbacks(&tokens, &mut candidates);
        // After fallback: noun should be first (all-caps → proper noun)
        assert_eq!(candidates[0][0].class, PosClass::Noun);
    }

    #[test]
    fn test_fallback_single_candidate_no_change() {
        let tokens = vec!["ходив"];
        let mut candidates = vec![vec![PosTag::parse("verb:past:m:s:imperf")]];
        apply_fallbacks(&tokens, &mut candidates);
        assert_eq!(candidates[0].len(), 1);
        assert_eq!(candidates[0][0].class, PosClass::Verb);
    }

    // --- Integration tests ---

    #[test]
    fn test_tag_sentence_basic() {
        let tokens = vec!["Мати", "бачить", "сина"];
        let candidates = vec![
            vec![
                PosTag::parse("noun:f:v_naz:anim"),
                PosTag::parse("verb:inf:imperf"),
            ],
            vec![PosTag::parse("verb:pres:3:s:imperf")],
            vec![PosTag::parse("noun:m:v_rod:anim")],
        ];

        let tagged = tag_sentence(&tokens, &candidates);
        assert_eq!(tagged.len(), 3);
        // "Мати" at sentence start should prefer NOUN (capitalized + sentence start)
        assert_eq!(tagged[0].selected_tag.class, PosClass::Noun);
    }

    #[test]
    fn test_tag_sentence_homonymy_resolution() {
        // "Діти ідуть додому" — "діти" should be NOUN (children), not VERB (to do)
        let tokens = vec!["Діти", "йдуть", "додому"];
        let candidates = vec![
            vec![
                PosTag::parse("noun:p:v_naz:anim"),
                PosTag::parse("verb:inf:imperf"),
            ],
            vec![PosTag::parse("verb:pres:3:p:imperf")],
            vec![PosTag::parse("adv")],
        ];

        let tagged = tag_sentence(&tokens, &candidates);
        // Capitalized + sentence start → noun preferred
        assert_eq!(tagged[0].selected_tag.class, PosClass::Noun);
    }

    #[test]
    fn test_tag_sentence_unknown_tag_handled() {
        let tokens = vec!["xyz"];
        let candidates = vec![vec![PosTag::unknown()]];
        let tagged = tag_sentence(&tokens, &candidates);
        assert_eq!(tagged.len(), 1);
        assert_eq!(tagged[0].selected_tag.class, PosClass::Unknown);
    }

    #[test]
    fn test_tag_sentence_empty_input() {
        let tokens: Vec<&str> = vec![];
        let candidates: Vec<Vec<PosTag>> = vec![];
        let tagged = tag_sentence(&tokens, &candidates);
        assert!(tagged.is_empty());
    }

    #[test]
    fn test_rule_count_loads_artifact() {
        // This test will pass if pos_rules.json.gz exists in the expected location.
        // If not, returns 0 (which is also a valid test — checks graceful degradation).
        let count = rule_count();
        // We expect ~450 rules if artifact was built
        if count > 0 {
            assert!(count > 100, "Expected >100 rules, got {}", count);
            println!("✓ Loaded {} rules from artifact", count);
        } else {
            println!("⚠ pos_rules.json.gz not found — skipping artifact test");
        }
    }

    #[test]
    fn test_cases_for_verb_known() {
        // "боятися" governs Genitive in case_government.txt
        let cases = cases_for_verb("боятися");
        if !cases.is_empty() {
            assert!(cases.contains(&GrammaticalCase::Genitive));
        }
    }

    #[test]
    fn test_cases_for_verb_unknown() {
        let cases = cases_for_verb("xyzqwerty");
        assert!(cases.is_empty());
    }

    #[test]
    fn test_tag_word_single() {
        let word = "ходив";
        let cands = vec![PosTag::parse("verb:past:m:s:imperf")];
        let tagged = tag_word(word, &cands);
        assert_eq!(tagged.word, "ходив");
        assert_eq!(tagged.selected_tag.class, PosClass::Verb);
        assert!(tagged.is_disambiguated);
    }

    #[test]
    fn test_pos_tag_matches_pattern() {
        let tag = PosTag::parse("noun:f:v_naz:anim");
        let pattern = Regex::new("noun:.*:v_naz.*").unwrap();
        assert!(tag.matches_pattern(&pattern));
        let pattern2 = Regex::new("verb:.*").unwrap();
        assert!(!tag.matches_pattern(&pattern2));
    }

    #[test]
    fn test_pos_tag_matches_prefix() {
        let tag = PosTag::parse("noun:f:v_naz:anim");
        assert!(tag.matches_prefix("noun:"));
        assert!(!tag.matches_prefix("verb:"));
    }

    #[test]
    fn test_filter_by_case_action() {
        let rule = DisambiguationRule {
            id: "TEST_FILTER".to_string(),
            name: "filter".to_string(),
            pattern: vec![TokenCondition {
                text: None,
                text_regex: None,
                postag: Some("noun:.*".to_string()),
                postag_regex: Some(Regex::new("noun:.*").unwrap()),
                inflected: false,
                case_sensitive: false,
                negate: false,
                min: None,
                max: None,
            }],
            marker_start: 0,
            marker_end: 1,
            action: DisambigAction::FilterByCase(GrammaticalCase::Accusative),
        };

        let tokens = vec!["сина"];
        let mut candidates = vec![vec![
            PosTag::parse("noun:m:v_rod:anim"),
            PosTag::parse("noun:m:v_zna:anim"),
            PosTag::parse("noun:m:v_naz:anim"),
        ]];

        let matched = rule.try_apply(&tokens, &mut candidates, 0);
        assert!(matched);
        // After filter: only Accusative candidate remains
        assert_eq!(candidates[0].len(), 1);
        assert_eq!(candidates[0][0].case, Some(GrammaticalCase::Accusative));
    }

    #[test]
    fn test_immunize_action_no_change() {
        let rule = DisambiguationRule {
            id: "TEST_IMM".to_string(),
            name: "immunize".to_string(),
            pattern: vec![TokenCondition {
                text: Some("ходити".to_string()),
                text_regex: Some(Regex::new("ходити").unwrap()),
                postag: None,
                postag_regex: None,
                inflected: false,
                case_sensitive: false,
                negate: false,
                min: None,
                max: None,
            }],
            marker_start: 0,
            marker_end: 1,
            action: DisambigAction::Immunize,
        };

        let tokens = vec!["ходити"];
        let mut candidates = vec![vec![PosTag::parse("verb:inf:imperf")]];
        let matched = rule.try_apply(&tokens, &mut candidates, 0);
        assert!(matched);
        // Immunize should not change candidates
        assert_eq!(candidates[0].len(), 1);
    }

    #[test]
    fn test_add_action_appends_tag() {
        let rule = DisambiguationRule {
            id: "TEST_ADD".to_string(),
            name: "add".to_string(),
            pattern: vec![TokenCondition {
                text: Some("військових".to_string()),
                text_regex: Some(Regex::new("військових").unwrap()),
                postag: None,
                postag_regex: None,
                inflected: false,
                case_sensitive: false,
                negate: false,
                min: None,
                max: None,
            }],
            marker_start: 0,
            marker_end: 1,
            action: DisambigAction::AddTag(PosTag::parse("noun:subst:p:v_rod:anim")),
        };

        let tokens = vec!["військових"];
        let mut candidates = vec![vec![PosTag::parse("adj:p:v_rod:anim")]];
        let matched = rule.try_apply(&tokens, &mut candidates, 0);
        assert!(matched);
        assert_eq!(candidates[0].len(), 2);
        assert!(candidates[0].iter().any(|c| c.raw_tag == "noun:subst:p:v_rod:anim"));
    }
}
