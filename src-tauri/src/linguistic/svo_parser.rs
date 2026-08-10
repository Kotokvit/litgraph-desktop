//! # SVO (Subject-Verb-Object) Parser for Ukrainian (Tauri Mirror)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

use super::pos_tagger::{self, GrammaticalCase, PosClass, PosTag, TaggedToken};

/// An extracted SVO Triplet representing a semantic literary event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SvoTriplet {
    pub actor: String,
    pub verb: String,
    pub target: Option<String>,
    pub instrument: Option<String>,
    pub location: Option<String>,
    pub polarity: bool,
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

        let candidates: Vec<Vec<PosTag>> = tokens
            .iter()
            .map(|&w| candidates_for_word(w))
            .collect();

        let tagged = pos_tagger::tag_sentence(&tokens, &candidates);
        self.extract_triplets(&tagged)
    }

    /// Extract SVO triplets from pre-tagged tokens
    pub fn extract_triplets(&self, tokens: &[TaggedToken]) -> Vec<SvoTriplet> {
        let mut triplets = Vec::new();

        if tokens.len() < 2 {
            return triplets;
        }

        for (i, token) in tokens.iter().enumerate() {
            if token.selected_tag.class != PosClass::Verb {
                continue;
            }

            let verb_lemma = if token.lemma.is_empty() || token.lemma == token.word.to_lowercase() {
                infer_verb_lemma(&token.word)
            } else {
                token.lemma.clone()
            };

            let is_negated = (i > 0 && is_negation_word(&tokens[i - 1].word))
                || (i > 1 && is_negation_word(&tokens[i - 2].word));

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
                None => "Хтось".to_string(),
            };

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

            let mut conf: f64 = 0.70;
            if actor_idx.is_some() {
                conf += 0.15;
            }
            if target_str.is_some() {
                conf += 0.10;
            }
            if is_negated {
                conf -= 0.05;
            }

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
    let lc = word.to_lowercase();
    let mut cands = Vec::new();

    if lc.ends_with("ла") || lc.ends_with("в") || lc.ends_with("ти") || lc.ends_with("ть") {
        cands.push(PosTag::parse("verb:past:m:s:imperf"));
        cands.push(PosTag::parse("verb:past:f:s:imperf"));
    }

    if lc.ends_with("а") || lc.ends_with("я") {
        cands.push(PosTag::parse("noun:f:v_naz:anim"));
        cands.push(PosTag::parse("noun:m:v_zna:anim"));
        cands.push(PosTag::parse("noun:m:v_rod:anim"));
    }

    if lc.ends_with("у") || lc.ends_with("ю") {
        cands.push(PosTag::parse("noun:f:v_zna:anim"));
        cands.push(PosTag::parse("noun:m:v_dav:anim"));
    }

    if lc.ends_with("ом") || lc.ends_with("ем") || lc.ends_with("ею") || lc.ends_with("ою") {
        cands.push(PosTag::parse("noun:m:v_oru:inanim"));
    }

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
    }
}
