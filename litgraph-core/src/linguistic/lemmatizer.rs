//! Lemmatizer runtime — word-form → lemma resolution via dict_uk (ВЕСУМ).
//!
//! This module loads the pre-built `lemma_index.json.gz` (produced by
//! `xtask build-lemmatizer`) at first use and exposes a simple lookup API.
//!
//! ## Performance
//!
//! - Index size: ~17 MB compressed JSON, ~30 MB in-memory HashMap.
//! - Load time: ~150 ms on first call (one-shot, then cached in `OnceLock`).
//! - Lookup time: O(1) HashMap get.
//!
//! ## Symbolic, not stochastic
//!
//! All lemmas come from a hand-curated dictionary (dict_uk / ВЕСУМ, LGPL).
//! No ML model, no embeddings, no neural network. The same input word
//! always produces the same lemma — fully deterministic and auditable.
//!
//! ## Example
//!
//! ```no_run
//! use litgraph_core::linguistic::lemmatizer;
//!
//! // Returns all lemma candidates for a word form
//! let entries = lemmatizer::lemmatize("ходив");
//! assert!(entries.iter().any(|e| e.lemma == "ходити"));
//!
//! // Convenience: just the first lemma
//! let lemma = lemmatizer::lemmatize_first("ходив");
//! assert_eq!(lemma.as_deref(), Some("ходити"));
//!
//! // Check if word form is known to the dictionary
//! assert!(lemmatizer::is_known("ходити"));
//! assert!(!lemmatizer::is_known("xyzqwerty"));
//! ```
//!
//! ## Missing index behavior
//!
//! If `lemma_index.json.gz` is not found (e.g. user hasn't run
//! `cargo run --release -- build-lemmatizer` yet), all functions
//! return empty results and log a warning. The rest of LitGraph
//! continues to work — lemmatization is an enhancement, not a hard
//! dependency.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::OnceLock;

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};

/// One lemma candidate for a word form.
///
/// A single word form may have multiple lemma candidates
/// (e.g. "пила" → "пити" verb:past:f or "пило" noun:f:v_naz).
/// The POS tagger (Layer B, not yet implemented) will disambiguate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LemmaEntry {
    /// The lemma (dictionary form): "ходити", "абонувати", "страх", "бути"
    pub lemma: String,
    /// Full POS tag from dict_uk, e.g. "verb:past:m:imperf", "noun:f:v_rod",
    /// "adj:m:v_naz:inanim", or "lemma:base:v1" for the canonical form.
    pub pos: String,
    /// Paradigm class: "v1", "n10", "adj", "vr1", or "exception" for
    /// suppletive forms from exceptions.lst (бути/є/буду/було).
    pub paradigm_class: String,
}

/// Inner type of the loaded index.
type Index = HashMap<String, Vec<LemmaEntry>>;

/// Global cached lemmatizer index.
///
/// Loaded on first call to [`index()`]. If loading fails, stores `None`
/// and all subsequent lookups return empty results.
static INDEX: OnceLock<Option<Index>> = OnceLock::new();

/// Locate `lemma_index.json.gz` on disk.
///
/// Search order:
/// 1. `resources/ua-linguistic/derivatives/lemma_index.json.gz` (relative
///    to the current working directory — works for dev runs from repo root)
/// 2. `$XDG_DATA_HOME/litgraph/lemma_index.json.gz` (user-installed data)
/// 3. `/usr/local/share/litgraph/lemma_index.json.gz` (system-wide install)
///
/// Returns `None` if the file is not found in any location.
fn locate_index_file() -> Option<PathBuf> {
    // 1. Repo-relative path (dev mode)
    let dev_path = PathBuf::from("resources/ua-linguistic/derivatives/lemma_index.json.gz");
    if dev_path.exists() {
        return Some(dev_path);
    }

    // 2. User data dir (~/.local/share/litgraph/...)
    if let Some(data_dir) = dirs::data_dir() {
        let user_path = data_dir.join("litgraph").join("lemma_index.json.gz");
        if user_path.exists() {
            return Some(user_path);
        }
    }

    // 3. System-wide install
    let system_path = PathBuf::from("/usr/local/share/litgraph/lemma_index.json.gz");
    if system_path.exists() {
        return Some(system_path);
    }

    None
}

/// Load and decompress the lemma index from disk.
///
/// Returns `Ok(index)` on success, or `Err(message)` with a human-readable
/// description of what went wrong (file not found, gzip decode failure,
/// JSON parse failure).
fn load_index() -> Result<Index, String> {
    let path = locate_index_file()
        .ok_or_else(|| "lemma_index.json.gz not found in any search location".to_string())?;

    let file = std::fs::File::open(&path)
        .map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;

    let mut decoder = GzDecoder::new(file);
    let mut json_bytes = Vec::new();
    decoder.read_to_end(&mut json_bytes)
        .map_err(|e| format!("Failed to decompress {}: {}", path.display(), e))?;

    let index: Index = serde_json::from_slice(&json_bytes)
        .map_err(|e| format!("Failed to parse JSON in {}: {}", path.display(), e))?;

    Ok(index)
}

/// Get a reference to the global cached index, loading it on first call.
///
/// Returns `None` if the index could not be loaded (file missing or
/// corrupt). All public lookup functions handle `None` gracefully by
/// returning empty results.
fn index() -> Option<&'static Index> {
    INDEX.get_or_init(|| match load_index() {
        Ok(idx) => {
            eprintln!("[lemmatizer] Loaded {} word forms", idx.len());
            Some(idx)
        }
        Err(err) => {
            eprintln!("[lemmatizer] WARNING: failed to load lemma index: {}", err);
            eprintln!("[lemmatizer] Run `cargo run --release -- build-lemmatizer` to (re)build it.");
            None
        }
    }).as_ref()
}

/// Look up all lemma candidates for a word form.
///
/// The input is matched case-insensitively (lowercased before lookup).
/// Returns an empty `Vec` if:
/// - The word form is not in the dictionary
/// - The lemma index could not be loaded
///
/// # Examples
///
/// ```no_run
/// # use litgraph_core::linguistic::lemmatizer;
/// let entries = lemmatizer::lemmatize("ходив");
/// // entries may contain: LemmaEntry { lemma: "ходити", pos: "verb:past:m:imperf", ... }
/// ```
pub fn lemmatize(word: &str) -> Vec<LemmaEntry> {
    let word_lower = word.to_lowercase();
    match index() {
        Some(idx) => idx.get(&word_lower).cloned().unwrap_or_default(),
        None => Vec::new(),
    }
}

/// Convenience: return only the first lemma string, or `None` if unknown.
///
/// Use this when you don't need the full POS tag and just want the
/// dictionary form. For ambiguous word forms, returns the first
/// candidate — order is determined by dict_uk file ordering.
///
/// # Examples
///
/// ```no_run
/// # use litgraph_core::linguistic::lemmatizer;
/// assert_eq!(lemmatizer::lemmatize_first("ходив").as_deref(), Some("ходити"));
/// assert_eq!(lemmatizer::lemmatize_first("xyzqwerty"), None);
/// ```
pub fn lemmatize_first(word: &str) -> Option<String> {
    lemmatize(word).into_iter().next().map(|e| e.lemma)
}

/// Check if a word form is known to the dictionary.
///
/// Returns `false` if the index is not loaded.
pub fn is_known(word: &str) -> bool {
    let word_lower = word.to_lowercase();
    index().map_or(false, |idx| idx.contains_key(&word_lower))
}

/// Return the total number of unique word forms in the loaded index.
///
/// Returns 0 if the index is not loaded. Useful for diagnostics.
pub fn index_size() -> usize {
    index().map_or(0, |idx| idx.len())
}

/// Return `true` if the lemma index has been successfully loaded.
///
/// Triggers loading on first call. Subsequent calls are O(1).
pub fn is_loaded() -> bool {
    index().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that `locate_index_file` does not panic when no index exists.
    /// In CI/test environments where the index is not built, this should
    /// return `None` gracefully.
    #[test]
    fn test_locate_index_file_does_not_panic() {
        let _ = locate_index_file();
        // We don't assert the result because it depends on the environment.
    }

    /// Test the lowercase normalization of `lemmatize`.
    /// If the index is not loaded, this returns an empty Vec (no panic).
    #[test]
    fn test_lemmatize_lowercases_input() {
        let _ = lemmatize("Ходив");
        let _ = lemmatize("ХОДИВ");
        let _ = lemmatize("ходив");
        // No panic = pass. We can't assert content without a loaded index.
    }

    /// Test that `lemmatize_first` returns None gracefully when index is missing.
    #[test]
    fn test_lemmatize_first_returns_none_if_unloaded() {
        // In environments without the index file, this should be None.
        // In environments with the index, it should be Some("ходити") for "ходив".
        let result = lemmatize_first("ходив");
        if !is_loaded() {
            assert!(result.is_none(), "Without index, lemmatize_first must be None");
        } else {
            // If loaded, "ходив" should lemmatize to "ходити"
            assert_eq!(result.as_deref(), Some("ходити"));
        }
    }

    /// Test that `is_known` returns false for nonsense words.
    #[test]
    fn test_is_known_returns_false_for_nonsense() {
        let result = is_known("zzzqqqxxx");
        if !is_loaded() {
            assert!(!result);
        } else {
            // Even with index loaded, nonsense should not be known
            assert!(!result);
        }
    }

    /// Test that `index_size` returns 0 when unloaded, or a large number when loaded.
    #[test]
    fn test_index_size() {
        let size = index_size();
        if !is_loaded() {
            assert_eq!(size, 0);
        } else {
            // dict_uk produces ~1.7M word forms
            assert!(size > 100_000, "Index should have 100k+ entries, got {}", size);
        }
    }

    /// Test suppletive verb "було" → "бути" (if index is loaded).
    /// This validates that exceptions.lst parsing works end-to-end.
    #[test]
    fn test_suppletive_verb_bulo_to_buty() {
        if !is_loaded() {
            eprintln!("[test] Index not loaded, skipping suppletive verb test");
            return;
        }
        let entries = lemmatize("було");
        assert!(
            entries.iter().any(|e| e.lemma == "бути"),
            "Expected 'було' to lemmatize to 'бути', got: {:?}",
            entries
        );
    }

    /// Test regular verb conjugation: "ходив" → "ходити".
    #[test]
    fn test_regular_verb_khodyv_to_khodyty() {
        if !is_loaded() {
            eprintln!("[test] Index not loaded, skipping regular verb test");
            return;
        }
        let entries = lemmatize("ходив");
        assert!(
            entries.iter().any(|e| e.lemma == "ходити"),
            "Expected 'ходив' to lemmatize to 'ходити', got: {:?}",
            entries
        );
    }

    /// Test noun declension: "страху" → "страх" (genitive).
    #[test]
    fn test_noun_genitive_strakhu_to_strakh() {
        if !is_loaded() {
            eprintln!("[test] Index not loaded, skipping noun declension test");
            return;
        }
        let entries = lemmatize("страху");
        assert!(
            entries.iter().any(|e| e.lemma == "страх"),
            "Expected 'страху' to lemmatize to 'страх', got: {:?}",
            entries
        );
    }

    /// Test that the same word always produces the same lemma (determinism).
    /// This is the core promise of symbolic AI — no stochasticity.
    #[test]
    fn test_determinism() {
        if !is_loaded() {
            return;
        }
        let r1 = lemmatize("ходив");
        let r2 = lemmatize("ходив");
        let r3 = lemmatize("ходив");
        assert_eq!(r1, r2);
        assert_eq!(r2, r3);
    }
}
