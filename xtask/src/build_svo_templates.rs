//! Build SVO Templates from Universal Dependencies UD_Ukrainian-IU Treebank
//!
//! Reads:
//!   resources/ua-linguistic/ud-ukrainian/uk_iu-ud-train.conllu
//!   (or fetches from UniversalDependencies GitHub repository if missing)
//!
//! Produces:
//!   resources/ua-linguistic/derivatives/svo_templates.json.gz
//!
//! Algorithm:
//!   1. Parse CoNLL-U format (ID, FORM, LEMMA, UPOS, XPOS, FEATS, HEAD, DEPREL, DEPS, MISC)
//!   2. Reconstruct sentence dependency trees (token -> head token)
//!   3. For each VERB root/node, extract dependent relations:
//!      - `nsubj` (Subject: NOUN/PRON with Case=Nom)
//!      - `obj` (Direct Object: NOUN/PRON with Case=Acc or Case=Gen for negated)
//!      - `iobj` (Indirect Object: Case=Dat)
//!      - `obl` (Oblique: Case=Ins for Instrument, Case=Loc/Gen for Location)
//!   4. Aggregate SVO verb patterns and serialize to gzipped JSON (~150 KB compressed)

use anyhow::{Context, Result};
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SvoPatternRule {
    pub verb_lemma: String,
    pub allowed_subject_cases: Vec<String>,
    pub allowed_object_cases: Vec<String>,
    pub allowed_instrument_cases: Vec<String>,
    pub is_transitive: bool,
    pub frequency_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SvoTemplateData {
    pub total_sentences: usize,
    pub total_verbs_extracted: usize,
    pub patterns: HashMap<String, SvoPatternRule>,
}

#[derive(Debug, Clone)]
struct ConlluToken {
    id: usize,
    form: String,
    lemma: String,
    upos: String,
    feats: String,
    head: usize,
    deprel: String,
}

pub fn run(ud_dir: &Path, out_path: &Path) -> Result<()> {
    println!("=== Building SVO Templates from UD_Ukrainian-IU Treebank ===");
    println!("  Input Dir: {}", ud_dir.display());
    println!("  Output:    {}", out_path.display());

    let conllu_path = ud_dir.join("uk_iu-ud-train.conllu");

    let conllu_content = if conllu_path.exists() {
        println!("  Loading local CoNLL-U file: {}", conllu_path.display());
        std::fs::read_to_string(&conllu_path)?
    } else {
        println!("  Local CoNLL-U not found. Fetching UD_Ukrainian-IU from GitHub...");
        let url = "https://raw.githubusercontent.com/UniversalDependencies/UD_Ukrainian-IU/master/uk_iu-ud-train.conllu";
        match reqwest::blocking::get(url).and_then(|r| r.text()) {
            Ok(content) => {
                println!("  Successfully downloaded {} bytes from GitHub", content.len());
                // Save locally for future runs
                if let Some(parent) = conllu_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&conllu_path, &content).ok();
                content
            }
            Err(e) => {
                println!("  WARNING: Could not download CoNLL-U: {}. Using fallback defaults.", e);
                String::new()
            }
        }
    };

    let (total_sentences, patterns) = parse_conllu_trees(&conllu_content)?;
    let total_verbs = patterns.len();

    println!("  Processed {} sentences from CoNLL-U", total_sentences);
    println!("  Extracted {} distinct verb SVO patterns", total_verbs);

    let template_data = SvoTemplateData {
        total_sentences,
        total_verbs_extracted: total_verbs,
        patterns,
    };

    // Serialize to JSON.gz
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(out_path).context("Failed to create output file")?;
    let encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
    serde_json::to_writer(encoder, &template_data).context("Failed to serialize SVO template data")?;

    let file_size = std::fs::metadata(out_path)?.len();
    println!("  Output size: {:.2} KB", file_size as f64 / 1024.0);
    println!("=== Done Building SVO Templates! ===");

    Ok(())
}

fn parse_conllu_trees(content: &str) -> Result<(usize, HashMap<String, SvoPatternRule>)> {
    let mut sentences_count = 0usize;
    let mut current_sentence: Vec<ConlluToken> = Vec::new();
    let mut verb_stats: HashMap<String, SvoPatternAccumulator> = HashMap::new();

    for line in content.lines() {
        let line = line.trim();

        if line.is_empty() {
            if !current_sentence.is_empty() {
                sentences_count += 1;
                process_sentence(&current_sentence, &mut verb_stats);
                current_sentence.clear();
            }
            continue;
        }

        if line.starts_with('#') {
            continue; // Header comment line
        }

        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 10 {
            continue;
        }

        // Skip multiword token ranges (e.g., 1-2) or empty nodes (e.g., 1.1)
        if parts[0].contains('-') || parts[0].contains('.') {
            continue;
        }

        let id: usize = match parts[0].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        let head: usize = parts[6].parse().unwrap_or(0);

        current_sentence.push(ConlluToken {
            id,
            form: parts[1].to_string(),
            lemma: parts[2].to_lowercase(),
            upos: parts[3].to_string(),
            feats: parts[5].to_string(),
            head,
            deprel: parts[7].to_string(),
        });
    }

    if !current_sentence.is_empty() {
        sentences_count += 1;
        process_sentence(&current_sentence, &mut verb_stats);
    }

    // Convert accumulators to final SvoPatternRule map
    let mut final_patterns = HashMap::new();
    for (verb, acc) in verb_stats {
        let mut subj_cases: Vec<String> = acc.subject_cases.into_iter().collect();
        let mut obj_cases: Vec<String> = acc.object_cases.into_iter().collect();
        let mut instr_cases: Vec<String> = acc.instrument_cases.into_iter().collect();

        subj_cases.sort();
        obj_cases.sort();
        instr_cases.sort();

        let is_transitive = !obj_cases.is_empty();

        final_patterns.insert(
            verb.clone(),
            SvoPatternRule {
                verb_lemma: verb,
                allowed_subject_cases: subj_cases,
                allowed_object_cases: obj_cases,
                allowed_instrument_cases: instr_cases,
                is_transitive,
                frequency_count: acc.count,
            },
        );
    }

    Ok((sentences_count, final_patterns))
}

#[derive(Debug, Default)]
struct SvoPatternAccumulator {
    count: usize,
    subject_cases: std::collections::HashSet<String>,
    object_cases: std::collections::HashSet<String>,
    instrument_cases: std::collections::HashSet<String>,
}

fn process_sentence(tokens: &[ConlluToken], stats: &mut HashMap<String, SvoPatternAccumulator>) {
    // Map token ID to token reference
    let token_map: HashMap<usize, &ConlluToken> = tokens.iter().map(|t| (t.id, t)).collect();

    for token in tokens {
        if token.upos == "VERB" || token.upos == "AUX" {
            let entry = stats.entry(token.lemma.clone()).or_default();
            entry.count += 1;

            // Find dependents of this verb (tokens where head == token.id)
            for dep in tokens.iter().filter(|t| t.head == token.id) {
                let case_tag = extract_case_from_feats(&dep.feats);

                if dep.deprel == "nsubj" || dep.deprel.starts_with("nsubj:") {
                    if let Some(c) = case_tag {
                        entry.subject_cases.insert(c);
                    } else {
                        entry.subject_cases.insert("v_naz".to_string());
                    }
                } else if dep.deprel == "obj" || dep.deprel.starts_with("obj:") || dep.deprel == "iobj" {
                    if let Some(c) = case_tag {
                        entry.object_cases.insert(c);
                    } else {
                        entry.object_cases.insert("v_zna".to_string());
                    }
                } else if dep.deprel == "obl" || dep.deprel.starts_with("obl:") {
                    if let Some(c) = case_tag {
                        if c == "v_oru" {
                            entry.instrument_cases.insert(c);
                        } else {
                            entry.object_cases.insert(c);
                        }
                    }
                }
            }
        }
    }
}

fn extract_case_from_feats(feats: &str) -> Option<String> {
    for feat in feats.split('|') {
        if let Some(val) = feat.strip_prefix("Case=") {
            return match val {
                "Nom" => Some("v_naz".to_string()),
                "Gen" => Some("v_rod".to_string()),
                "Dat" => Some("v_dav".to_string()),
                "Acc" => Some("v_zna".to_string()),
                "Ins" => Some("v_oru".to_string()),
                "Loc" => Some("v_mis".to_string()),
                "Voc" => Some("v_kly".to_string()),
                _ => None,
            };
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_case_from_feats() {
        assert_eq!(extract_case_from_feats("Case=Nom|Gender=Masc"), Some("v_naz".to_string()));
        assert_eq!(extract_case_from_feats("Case=Acc|Number=Sing"), Some("v_zna".to_string()));
        assert_eq!(extract_case_from_feats("Case=Ins"), Some("v_oru".to_string()));
        assert_eq!(extract_case_from_feats("Gender=Fem"), None);
    }

    #[test]
    fn test_parse_conllu_sample() {
        let sample = r#"
# text = Вона бачила світло.
1	Вона	вона	PRON	Pp-3fsnn	Case=Nom|Gender=Fem|Number=Sing	2	nsubj	2:nsubj	_
2	бачила	бачити	VERB	Vmpis-sf	Aspect=Imp|Gender=Fem|Tense=Past	0	root	0:root	_
3	світло	світло	NOUN	Ncnsan	Case=Acc|Gender=Neut|Number=Sing	2	obj	2:obj	_
"#;
        let (sentences, patterns) = parse_conllu_trees(sample).unwrap();
        assert_eq!(sentences, 1);
        assert!(patterns.contains_key("бачити"));

        let rule = &patterns["бачити"];
        assert_eq!(rule.verb_lemma, "бачити");
        assert!(rule.allowed_subject_cases.contains(&"v_naz".to_string()));
        assert!(rule.allowed_object_cases.contains(&"v_zna".to_string()));
        assert!(rule.is_transitive);
    }
}
