//! v0.6.1 / Phase 2 Step 4: CLI for Rust fast path.
//!
//! Standalone binary that runs `parser::characters::detect()` + the same
//! dispatch logic from `src-tauri/src/commands/ner.rs::rust_fast_path_entities`
//! (without Tauri dependency).
//!
//! Output: JSON matching `NerResult` schema (subset — only PER entities,
//! since Rust fast path doesn't detect LOC/ORG).
//!
//! Usage:
//!     cargo run --bin rust_ner_cli -- path/to/file.md
//!     cat file.md | cargo run --bin rust_ner_cli -- -
//!
//! Used by `experiments/teaching_loop/ingest_corpus.py` to extract Rust-side
//! candidate nodes for each text in the corpus.

use std::io::{self, Read};
use std::fs;
use std::path::PathBuf;

use litgraph_core::parser::characters::{detect, EntityType, ParsedCharacter};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct EntityMention {
    text: String,
    start: usize,
    end: usize,
    sentence: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entity {
    lemma: String,
    label: String,
    count: usize,
    forms: Vec<String>,
    first_mention: usize,
    mentions: Vec<EntityMention>,
    /// Phase 2 Step 4: 8-feature vector for Burn scorer (extracted at ingest time
    /// so the teaching loop can train without re-running detect()).
    features: Vec<f32>,
    /// Phase 2 Step 4: Rust's hardcoded confidence (0.3/0.7/1.0).
    /// Burn will learn to refine this.
    rust_confidence: f32,
}

#[derive(Debug, Serialize, Deserialize)]
struct NerStats {
    total: usize,
    persons: usize,
    locations: usize,
    organizations: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct NerResult {
    entities: Vec<Entity>,
    stats: NerStats,
    model: String,
    version: String,
    truncated: bool,
    text_length: usize,
    processed_length: usize,
    chunks_processed: Option<usize>,
}

fn extract_sentence_around(text: &str, offset: usize) -> String {
    // Approach A from Step 2b: byte scan + char_boundary alignment.
    if text.is_empty() {
        return String::new();
    }

    let max_len = 200usize;
    let target_start = offset.saturating_sub(max_len / 2);
    let target_end = (offset + max_len / 2).min(text.len());

    // Backward scan to sentence boundary
    let mut start = target_start;
    let bytes = text.as_bytes();
    let mut i = target_start;
    while i > 0 && i < offset {
        let b = bytes[i];
        if b == b'.' || b == b'!' || b == b'?' || b == b'\n' {
            start = i + 1;
            break;
        }
        i -= 1;
    }
    // Align to char boundary
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }

    // Forward scan
    let mut end = target_end;
    let mut j = offset;
    while j < target_end && j < text.len() {
        let b = bytes[j];
        if b == b'.' || b == b'!' || b == b'?' || b == b'\n' {
            end = j;
            break;
        }
        j += 1;
    }
    while end > 0 && end < text.len() && !text.is_char_boundary(end) {
        end -= 1;
    }

    if start >= end {
        return String::new();
    }
    text[start..end].trim().to_string()
}

fn entity_from_parsed(c: &ParsedCharacter, text: &str) -> Entity {
    let mentions: Vec<EntityMention> = c.mention_starts.iter().filter_map(|&start| {
        let lower_text = text.to_lowercase();
        for alias in &c.aliases {
            let alias_lower = alias.to_lowercase();
            if start + alias_lower.len() <= text.len()
                && lower_text[start..start + alias_lower.len()] == alias_lower
            {
                let end = start + alias_lower.len();
                let mention_text = &text[start..end];
                let sentence = extract_sentence_around(text, start);
                return Some(EntityMention {
                    text: mention_text.to_string(),
                    start,
                    end,
                    sentence,
                });
            }
        }
        let name_len = c.name.len();
        if start + name_len <= text.len() {
            let end = start + name_len;
            let sentence = extract_sentence_around(text, start);
            Some(EntityMention {
                text: text[start..end].to_string(),
                start,
                end,
                sentence,
            })
        } else {
            None
        }
    }).collect();

    let first_mention = c.mention_starts.first().copied().unwrap_or(0);
    let features = litgraph_core::scorer::extract_features(c).to_vec();

    Entity {
        lemma: c.name.clone(),
        label: "PER".to_string(),
        count: c.mention_starts.len(),
        forms: c.aliases.clone(),
        first_mention,
        mentions,
        features,
        rust_confidence: c.confidence,
    }
}

fn run(text: &str) -> NerResult {
    let parsed = detect(text);

    // Use ALL detected candidates (Characters, Concepts, Organizations) — teaching loop
    // needs both true positives and false positives (confidence 0.3/0.7/1.0) to train Burn MLP.
    let entities: Vec<Entity> = parsed
        .iter()
        .map(|c| entity_from_parsed(c, text))
        .collect();

    let persons = entities.len();
    let stats = NerStats {
        total: persons,
        persons,
        locations: 0,
        organizations: 0,
    };

    NerResult {
        entities,
        stats,
        model: "rust-fast-path".to_string(),
        version: "2.3-step4".to_string(),
        truncated: false,
        text_length: text.len(),
        processed_length: text.len(),
        chunks_processed: None,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path.md|->", args[0]);
        eprintln!("  Use '-' to read from stdin");
        std::process::exit(1);
    }

    let text = if args[1] == "-" {
        let mut buf = String::new();
        io::stdin().read_to_string(&mut buf).expect("read stdin");
        buf
    } else {
        let path = PathBuf::from(&args[1]);
        fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("Error reading {}: {}", path.display(), e);
            std::process::exit(1);
        })
    };

    let result = run(&text);
    let json = serde_json::to_string_pretty(&result).expect("serialize");
    println!("{}", json);
}
