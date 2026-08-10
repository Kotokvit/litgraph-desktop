//! # SVO (Subject-Verb-Object) Parser for Ukrainian
//!
//! Rule-based SVO dependency triplet extraction engine using:
//! - Layer A (`lemmatizer.rs`): Dictionary base forms
//! - Layer B (`pos_tagger.rs`): Disambiguated POS tags & Ukrainian case system
//! - Layer C (`svo_templates.json.gz`): UD_Ukrainian-IU treebank dependency patterns
//!
//! # Features
//! - Subject (Actor) extraction from Nominative (`v_naz`) nouns/pronouns
//! - Action Verb identification with negation detection (`не` / `ні`)
//! - Direct Object (Target) extraction from Accusative (`v_zna`) or Genitive of Negation (`v_rod`)
//! - Indirect Instrument (`v_oru`) and Location (`v_mis` with preposition) extraction
//! - Deterministic confidence scoring in `[0.0, 1.0]`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

use super::lemmatizer;
use super::pos_tagger::{self, GrammaticalCase, PosClass, PosTag, TaggedToken};

/// An extracted SVO Triplet representing a semantic literary event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvoTriplet {
    /// Subject / Actor performing the action ("Марта", "Веня", "він")
    pub actor: String,
    /// Canonical lemma of the action verb ("вбити", "бачити", "йти")
    pub verb: String,
    /// Direct object / Target receiving the action ("ворога", "лист", "країну")
    pub target: Option<String>,
    /// Instrument used ("ножем", "пістолетом")
    pub instrument: Option<String>,
    /// Spatial location ("у кімнаті", "на стації")
    pub location: Option<String>,
    /// Polarity: `true` for affirmative, `false` for negated ("не вбив")
    pub polarity: bool,
    /// Deterministic confidence score in `[0.0, 1.0]`
    pub confidence: f64,
}

/// SVO pattern rule loaded from `svo_templates.json.gz`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SvoPatternRule {
    pub verb_lemma: String,
    pub allowed_subject_cases: Vec<String>,
    pub allowed_object_cases: Vec<String>,
    pub allowed_instrument_cases: Vec<String>,
    pub is_transitive: bool,
    pub frequency_count: usize,
}

/// Container for serialized SVO template data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvoTemplateData {
    pub total_sentences: usize,
    pub total_verbs_extracted: usize,
    pub patterns: HashMap<String, SvoPatternRule>,
}

static SVO_TEMPLATES: OnceLock<Option<SvoTemplateData>> = OnceLock::new();

fn locate_svo_templates_file() -> Option<std::path::PathBuf> {
    let candidates = vec![
        std::path::PathBuf::from("resources/ua-linguistic/derivatives/svo_templates.json.gz"),
        std::path::PathBuf::from("../resources/ua-linguistic/derivatives/svo_templates.json.gz"),
    ];
    for p in candidates {
        if p.exists() {
            return Some(p);
        }
    }
    if let Some(data_dir) = dirs::data_dir() {
        let user_path = data_dir.join("litgraph").join("svo_templates.json.gz");
        if user_path.exists() {
            return Some(user_path);
        }
    }
    let sys_path = std::path::PathBuf::from("/usr/local/share/litgraph/svo_templates.json.gz");
    if sys_path.exists() {
        return Some(sys_path);
    }
    None
}

fn load_svo_templates() -> Result<SvoTemplateData, String> {
    let path = locate_svo_templates_file()
        .ok_or_else(|| "svo_templates.json.gz not found".to_string())?;

    let file = std::fs::File::open(&path)
        .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;

    let mut decoder = flate2::read::GzDecoder::new(file);
    let mut json_bytes = Vec::new();
    std::io::Read::read_to_end(&mut decoder, &mut json_bytes)
        .map_err(|e| format!("Failed to decompress {}: {}", path.display(), e))?;

    let data: SvoTemplateData = serde_json::from_slice(&json_bytes)
        .map_err(|e| format!("Failed to parse SVO JSON: {}", e))?;

    Ok(data)
}

fn svo_templates() -> Option<&'static SvoTemplateData> {
    SVO_TEMPLATES.get_or_init(|| match load_svo_templates() {
        Ok(data) => Some(data),
        Err(err) => {
            eprintln!("[svo_parser] WARNING: failed to load SVO templates: {}", err);
            None
        }
    }).as_ref()
}

/// SVO Parser Engine instance
#[derive(Debug, Clone, Default)]
pub struct SvoParser;

impl SvoParser {
    pub fn new() -> Self {
        Self
    }

    /// Extract all SVO triplets from a sentence text string.
    pub fn parse_text(&self, sentence_text: &str) -> Vec<SvoTriplet> {
        let tokens: Vec<&str> = sentence_text
            .split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '\u{2019}')
            .filter(|s| !s.is_empty())
            .collect();

        if tokens.is_empty() {
            return Vec::new();
        }

        // Layer A: Fetch candidate POS tags from lemmatizer (with heuristic fallbacks)
        let candidates: Vec<Vec<PosTag>> = tokens
            .iter()
            .map(|&w| candidates_for_word(w))
            .collect();

        // Layer B: Disambiguate POS tags
        let tagged = pos_tagger::tag_sentence(&tokens, &candidates);
        self.extract_triplets(&tagged)
    }

    /// Extract SVO triplets from pre-tagged tokens
    pub fn extract_triplets(&self, tokens: &[TaggedToken]) -> Vec<SvoTriplet> {
        let mut triplets = Vec::new();

        if tokens.len() < 2 {
            return triplets;
        }

        // Find all verb positions
        for (i, token) in tokens.iter().enumerate() {
            if token.selected_tag.class != PosClass::Verb {
                continue;
            }

            let verb_lemma = if token.lemma.is_empty() || token.lemma == token.word.to_lowercase() {
                infer_verb_lemma(&token.word)
            } else {
                token.lemma.clone()
            };

            // Check negation ("не" or "ні" before verb within 2 tokens)
            let is_negated = (i > 0 && is_negation_word(&tokens[i - 1].word))
                || (i > 1 && is_negation_word(&tokens[i - 2].word));

            // Search for Subject (Actor): Noun/Pronoun in Nominative case before verb
            let mut actor_idx: Option<usize> = None;
            for j in (0..i).rev() {
                let t = &tokens[j];
                if (t.selected_tag.class == PosClass::Noun || t.selected_tag.class == PosClass::Pronoun)
                    && (t.selected_tag.case == Some(GrammaticalCase::Nominative) || t.selected_tag.case.is_none())
                {
                    actor_idx = Some(j);
                    break;
                }
            }

            // Fallback actor search after verb (Ukrainian flexible word order: "вбив його Петро")
            if actor_idx.is_none() {
                for j in (i + 1)..tokens.len() {
                    let t = &tokens[j];
                    if (t.selected_tag.class == PosClass::Noun || t.selected_tag.class == PosClass::Pronoun)
                        && t.selected_tag.case == Some(GrammaticalCase::Nominative)
                    {
                        actor_idx = Some(j);
                        break;
                    }
                }
            }

            let actor_str = match actor_idx {
                Some(idx) => capitalize_name(&tokens[idx].word),
                None => "Хтось".to_string(), // Unknown implicit actor
            };

            // Search for Direct Object (Target): Noun/Pronoun in Accusative or Genitive case after verb
            let mut target_str: Option<String> = None;
            let mut instrument_str: Option<String> = None;
            let mut location_str: Option<String> = None;

            for j in (i + 1)..tokens.len() {
                if Some(j) == actor_idx {
                    continue;
                }
                let t = &tokens[j];

                if t.selected_tag.class == PosClass::Noun || t.selected_tag.class == PosClass::Pronoun {
                    match t.selected_tag.case {
                        Some(GrammaticalCase::Accusative) => {
                            if target_str.is_none() {
                                target_str = Some(t.word.clone());
                            }
                        }
                        Some(GrammaticalCase::Genitive) if is_negated => {
                            // Genitive of Negation
                            if target_str.is_none() {
                                target_str = Some(t.word.clone());
                            }
                        }
                        Some(GrammaticalCase::Instrumental) => {
                            if instrument_str.is_none() {
                                instrument_str = Some(t.word.clone());
                            }
                        }
                        Some(GrammaticalCase::Locative) => {
                            if location_str.is_none() {
                                location_str = Some(t.word.clone());
                            }
                        }
                        _ => {
                            if target_str.is_none() && t.selected_tag.class == PosClass::Noun {
                                target_str = Some(t.word.clone());
                            }
                        }
                    }
                }
            }

            // Confidence calculation
            let mut conf: f64 = 0.70;
            if actor_idx.is_some() {
                conf += 0.15;
            }
            if target_str.is_some() {
                conf += 0.10;
            }
            if is_negated {
                conf -= 0.05; // Negation slight uncertainty
            }

            // Boost confidence if verb is in UD SVO template pattern map
            if let Some(data) = svo_templates() {
                if data.patterns.contains_key(&verb_lemma) {
                    conf += 0.05;
                }
            }

            triplets.push(SvoTriplet {
                actor: actor_str,
                verb: verb_lemma,
                target: target_str,
                instrument: instrument_str,
                location: location_str,
                polarity: !is_negated,
                confidence: conf.clamp(0.10, 1.0),
            });
        }

        triplets
    }
}

fn candidates_for_word(word: &str) -> Vec<PosTag> {
    let entries = lemmatizer::lemmatize(word);
    if !entries.is_empty() {
        return entries.iter().map(|e| PosTag::parse(&e.pos)).collect();
    }

    let lc = word.to_lowercase();
    let mut cands = Vec::new();

    // Verbs: past/infinitive endings
    if lc.ends_with("ла") || lc.ends_with("в") || lc.ends_with("ти") || lc.ends_with("ть") {
        cands.push(PosTag::parse("verb:past:m:s:imperf"));
        cands.push(PosTag::parse("verb:past:f:s:imperf"));
    }

    // Feminine Nominative ("Марта", "країна", "дитина") or Masculine Genitive/Accusative ("сина", "ворога")
    if lc.ends_with("а") || lc.ends_with("я") {
        cands.push(PosTag::parse("noun:f:v_naz:anim"));
        cands.push(PosTag::parse("noun:m:v_zna:anim"));
        cands.push(PosTag::parse("noun:m:v_rod:anim"));
    }

    // Accusative feminine ("Марту", "дитину") / Dative masculine ("сину")
    if lc.ends_with("у") || lc.ends_with("ю") {
        cands.push(PosTag::parse("noun:f:v_zna:anim"));
        cands.push(PosTag::parse("noun:m:v_dav:anim"));
    }

    // Instrumental ("мечем", "ножем", "днем")
    if lc.ends_with("ом") || lc.ends_with("ем") || lc.ends_with("ею") || lc.ends_with("ою") {
        cands.push(PosTag::parse("noun:m:v_oru:inanim"));
    }

    // Capitalized -> Proper Noun Nominative
    if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
        cands.push(PosTag::parse("noun:f:v_naz:anim"));
        cands.push(PosTag::parse("noun:m:v_naz:anim"));
    }

    if cands.is_empty() {
        cands.push(PosTag::unknown());
    }
    cands
}

fn infer_verb_lemma(word: &str) -> String {
    let lc = word.to_lowercase();
    let chars: Vec<char> = lc.chars().collect();
    if chars.len() >= 3 {
        if lc.ends_with("бив") || lc.ends_with("била") {
            let stem: String = chars[..chars.len() - 3].iter().collect();
            return format!("{}бити", stem);
        }
        if lc.ends_with("рив") || lc.ends_with("рила") {
            let stem: String = chars[..chars.len() - 3].iter().collect();
            return format!("{}рити", stem);
        }
        if lc.ends_with("ила") || lc.ends_with("ив") {
            let stem: String = chars[..chars.len() - 3].iter().collect();
            return format!("{}ити", stem);
        }
        if lc.ends_with("ала") || lc.ends_with("ав") {
            let stem: String = chars[..chars.len() - 3].iter().collect();
            return format!("{}ати", stem);
        }
    }
    lc
}

fn is_negation_word(w: &str) -> bool {
    let lc = w.to_lowercase();
    lc == "не" || lc == "ні"
}

fn capitalize_name(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svo_basic_affirmative() {
        let parser = SvoParser::new();
        let triplets = parser.parse_text("Марта бачила сина.");
        assert_eq!(triplets.len(), 1);
        let t = &triplets[0];
        assert_eq!(t.actor, "Марта");
        assert_eq!(t.verb, "бачити");
        assert_eq!(t.target.as_deref(), Some("сина"));
        assert!(t.polarity);
        assert!(t.confidence >= 0.80);
    }

    #[test]
    fn test_svo_negated_kill() {
        let parser = SvoParser::new();
        let triplets = parser.parse_text("Петро не вбив ворога.");
        assert_eq!(triplets.len(), 1);
        let t = &triplets[0];
        assert_eq!(t.actor, "Петро");
        assert_eq!(t.verb, "вбити");
        assert_eq!(t.target.as_deref(), Some("ворога"));
        assert!(!t.polarity); // negated
    }

    #[test]
    fn test_svo_with_instrument() {
        let parser = SvoParser::new();
        let triplets = parser.parse_text("Воїн ударив ворога мечем.");
        assert_eq!(triplets.len(), 1);
        let t = &triplets[0];
        assert_eq!(t.actor, "Воїн");
        assert_eq!(t.verb, "ударити");
        assert_eq!(t.target.as_deref(), Some("ворога"));
        assert_eq!(t.instrument.as_deref(), Some("мечем"));
    }
}
