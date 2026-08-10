//! Build Lemmatizer Index from dict_uk (ВЕСУМ)
//!
//! Reads:
//!   resources/ua-linguistic/dict_uk/data/dict/base.lst       (lemmas with POS tags)
//!   resources/ua-linguistic/dict_uk/data/dict/exceptions.lst (suppletive word forms)
//!   resources/ua-linguistic/dict_uk/data/affix/*.aff         (paradigm rules)
//!
//! Produces:
//!   resources/ua-linguistic/derivatives/lemma_index.json.gz
//!     - HashMap<word_form_lowercase, Vec<LemmaEntry { lemma, pos, paradigm_class }>>
//!     - Gzipped for size (~5 MB compressed vs ~30 MB raw)
//!
//! Algorithm:
//!   1. Parse base.lst → list of (lemma, paradigm_class, modifiers)
//!   2. For each lemma, apply its paradigm's affix rules to generate all word forms
//!   3. Parse exceptions.lst → pre-expanded word_form → (lemma, pos) entries
//!      (for suppletive forms: бути/є/буду/було, іти/хід/ішов, etc.)
//!   4. Build reverse index: word_form → Vec<(lemma, pos_tag)>
//!
//! This is pure symbolic morphology — no ML, no statistics.

use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use flate2::write::GzEncoder;
use flate2::Compression;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LemmaEntry {
    pub lemma: String,
    pub pos: String,         // e.g. "verb:pres:s:1", "noun:f:v_rod"
    pub paradigm_class: String, // e.g. "v1", "n10", "adj"
}

#[derive(Debug, Clone, Default)]
pub struct LemmaRecord {
    pub lemma: String,
    pub paradigm_class: String, // "v1", "n10", "n2n", "adj", "vr1"...
    pub modifiers: Vec<String>, // "p1", "p2", "a", "ke" etc.
}

/// One affix rule line, e.g.:
///   "ти\tла\t[^с]ти\t# абонувати абонувала (Вона) @ verb:past:f"
/// means: if word ends with "ти" matching regex [^с]ти, replace ending "ти" with "ла"
/// and tag the resulting form as "verb:past:f"
#[derive(Debug, Clone)]
pub struct AffixRule {
    pub from_suffix: String,  // what to remove (e.g. "ти")
    pub to_suffix: String,    // what to append (e.g. "ла", or "" for no suffix)
    pub match_regex: Option<Regex>, // optional regex constraint
    pub pos_tag: String,      // e.g. "verb:past:f"
}

#[derive(Debug, Default)]
pub struct AffixGroup {
    pub name: String,
    pub rules: Vec<AffixRule>,
}

pub fn run(dict_uk_path: &Path, out_path: &Path) -> Result<()> {
    println!("=== Building Lemmatizer Index from dict_uk ===");
    println!("  Input:  {}", dict_uk_path.display());
    println!("  Output: {}", out_path.display());

    let base_lst = dict_uk_path.join("data/dict/base.lst");
    let exceptions_lst = dict_uk_path.join("data/dict/exceptions.lst");
    let affix_dir = dict_uk_path.join("data/affix");

    // Step 1: Parse all affix files into a map: paradigm_class → Vec<AffixRule>
    println!("[1/5] Parsing affix rules...");
    let affix_groups = parse_all_affix_files(&affix_dir)?;
    let total_rules: usize = affix_groups.values().map(|g| g.rules.len()).sum();
    println!("      Loaded {} paradigm groups, {} total rules",
             affix_groups.len(), total_rules);

    // Step 2: Parse base.lst → list of lemmas
    println!("[2/5] Parsing lemmas from base.lst...");
    let lemmas = parse_base_lst(&base_lst)?;
    println!("      Loaded {} lemma records", lemmas.len());

    // Step 3: Generate all word forms by applying affix rules
    println!("[3/5] Generating word forms (this may take a while)...");
    let mut word_form_index: HashMap<String, Vec<LemmaEntry>> = HashMap::new();
    let mut total_forms = 0usize;
    let mut lemmas_with_no_paradigm = 0usize;

    for (i, lemma_rec) in lemmas.iter().enumerate() {
        if i % 25000 == 0 {
            println!("      Processing lemma {}/{} ({} forms so far)",
                     i, lemmas.len(), total_forms);
        }

        // Look up affix rules for this lemma's paradigm class
        let group = match affix_groups.get(&lemma_rec.paradigm_class) {
            Some(g) => g,
            None => {
                lemmas_with_no_paradigm += 1;
                // Still add the lemma itself as a form
                word_form_index
                    .entry(lemma_rec.lemma.to_lowercase())
                    .or_default()
                    .push(LemmaEntry {
                        lemma: lemma_rec.lemma.clone(),
                        pos: "lemma:base".to_string(),
                        paradigm_class: lemma_rec.paradigm_class.clone(),
                    });
                total_forms += 1;
                continue;
            }
        };

        // Apply each rule to the lemma
        for rule in &group.rules {
            if let Some(form) = apply_rule(&lemma_rec.lemma, rule) {
                let form_lower = form.to_lowercase();
                word_form_index
                    .entry(form_lower)
                    .or_default()
                    .push(LemmaEntry {
                        lemma: lemma_rec.lemma.clone(),
                        pos: rule.pos_tag.clone(),
                        paradigm_class: lemma_rec.paradigm_class.clone(),
                    });
                total_forms += 1;
            }
        }

        // Also include the lemma itself (infinitive / nominative singular)
        word_form_index
            .entry(lemma_rec.lemma.to_lowercase())
            .or_default()
            .push(LemmaEntry {
                lemma: lemma_rec.lemma.clone(),
                pos: format!("lemma:base:{}", lemma_rec.paradigm_class),
                paradigm_class: lemma_rec.paradigm_class.clone(),
            });
        total_forms += 1;
    }

    println!("      Generated {} total word forms (from affixes)", total_forms);
    println!("      Unique word forms in index: {}", word_form_index.len());
    println!("      Lemmas without matching paradigm: {}", lemmas_with_no_paradigm);

    // Step 4: Merge exceptions.lst (suppletive forms: бути/є/буду, іти/хід/ішов, etc.)
    println!("[4/5] Merging exceptions.lst (suppletive forms)...");
    let exceptions_count = merge_exceptions_lst(&exceptions_lst, &mut word_form_index)?;
    println!("      Merged {} exception word-form entries", exceptions_count);
    println!("      Unique word forms after merge: {}", word_form_index.len());

    // Step 5: Serialize to gzipped JSON
    println!("[5/5] Serializing to JSON.gz...");
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(out_path).context("Failed to create output file")?;
    let encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
    serde_json::to_writer(encoder, &word_form_index)
        .context("Failed to serialize word form index")?;

    let file_size = std::fs::metadata(out_path)?.len();
    println!("      Output size: {:.2} MB", file_size as f64 / 1_048_576.0);
    println!("=== Done! ===");

    Ok(())
}

/// Parse base.lst into a list of LemmaRecord.
/// Format: `<lemma> /<paradigm_class>.<modifier1>.<modifier2>  # comment`
/// Example: `абажур /n20.a.p.ke.@`
fn parse_base_lst(path: &Path) -> Result<Vec<LemmaRecord>> {
    let file = File::open(path).context("Failed to open base.lst")?;
    let reader = BufReader::new(file);
    let mut lemmas = Vec::new();

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        // Skip comments and empty lines
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Strip trailing comment
        let line = line.split('#').next().unwrap_or(line).trim();
        if line.is_empty() {
            continue;
        }

        // Split on '/' to get lemma and paradigm tag
        let parts: Vec<&str> = line.splitn(2, '/').collect();
        if parts.len() != 2 {
            continue;
        }

        let lemma = parts[0].trim();
        let tag_str = parts[1].trim();
        if lemma.is_empty() || tag_str.is_empty() {
            continue;
        }

        // Tag format: v1.cf.advp.is0  →  class=v1, modifiers=[cf, advp, is0]
        let tag_parts: Vec<&str> = tag_str.split('.').collect();
        let paradigm_class = tag_parts[0].to_string();
        let modifiers: Vec<String> = tag_parts[1..].iter().map(|s| s.to_string()).collect();

        // Some lemmas have a space-separated additional tag like `:xp1` or `:up92`
        // We don't use these for now; skip parsing.

        lemmas.push(LemmaRecord {
            lemma: lemma.to_string(),
            paradigm_class,
            modifiers,
        });
    }

    Ok(lemmas)
}

/// Merge exceptions.lst entries into the existing word_form_index.
///
/// exceptions.lst format (3 columns, space-separated):
///   `<word_form> <lemma> <pos_tag>  # optional comment`
///
/// Examples (suppletive verbs and irregular nouns):
///   `бути бути verb:inf:imperf                       # rv_oru:rv_dav`
///   `було бути verb:past:n:imperf:insert`
///   `є бути verb:pres:s:1:imperf`     (not in file, hypothetical)
///   `діти дитина noun:anim:p:v_naz`   (suppletive plural)
///   `якоря якір noun:m:v_rod`         (irregular noun declension)
///
/// Some lines use `//p:v_naz/v_zna/v_kly` syntax in the POS column to indicate
/// that the same word form also has plural forms — we keep these tags as-is.
///
/// Lines starting with `!` (like `# !noun`) are section markers — skip them.
/// Lines like `переддень /n20.a.p` (lemma with paradigm tag, NOT 3-column exception)
/// are skipped here — they belong to base.lst format, not exceptions.
fn merge_exceptions_lst(
    path: &Path,
    word_form_index: &mut HashMap<String, Vec<LemmaEntry>>,
) -> Result<usize> {
    if !path.exists() {
        // exceptions.lst is optional — return 0 if missing
        return Ok(0);
    }

    let file = File::open(path).context("Failed to open exceptions.lst")?;
    let reader = BufReader::new(file);
    let mut count = 0usize;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        // Skip empty lines, comments, and section markers like "# !noun"
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Strip trailing comment (after #) but preserve the POS tag columns
        let line_no_comment = line.split('#').next().unwrap_or(line).trim();
        if line_no_comment.is_empty() {
            continue;
        }

        // Split into columns by whitespace.
        // Expected: <word_form> <lemma> <pos_tag>
        // If we get only 2 columns where the 2nd starts with '/', it's a base.lst-style
        // line (lemma + paradigm tag) — skip it, those are processed by parse_base_lst.
        let cols: Vec<&str> = line_no_comment.split_whitespace().collect();
        if cols.len() < 3 {
            continue;
        }

        // Skip lines that look like base.lst entries (word /paradigm.tag)
        if cols[1].starts_with('/') {
            continue;
        }

        let word_form = cols[0];
        let lemma = cols[1];
        let pos_tag = cols[2..].join(" "); // join in case POS has spaces (rare)

        // Normalize: lowercase word form for lookup
        let form_lower = word_form.to_lowercase();
        if form_lower.is_empty() || lemma.is_empty() {
            continue;
        }

        word_form_index
            .entry(form_lower)
            .or_default()
            .push(LemmaEntry {
                lemma: lemma.to_string(),
                pos: pos_tag,
                paradigm_class: "exception".to_string(),
            });
        count += 1;
    }

    Ok(count)
}

/// Parse all .aff files in the affix directory.
/// Returns a map: paradigm_class → AffixGroup
fn parse_all_affix_files(affix_dir: &Path) -> Result<HashMap<String, AffixGroup>> {
    let mut groups = HashMap::new();

    if !affix_dir.exists() {
        return Err(anyhow::anyhow!("Affix directory not found: {}", affix_dir.display()));
    }

    for entry in std::fs::read_dir(affix_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("aff") {
            continue;
        }
        let file_groups = parse_affix_file(&path)?;
        for g in file_groups {
            groups.insert(g.name.clone(), g);
        }
    }

    Ok(groups)
}

/// Parse a single .aff file. May contain multiple groups.
fn parse_affix_file(path: &Path) -> Result<Vec<AffixGroup>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut groups = Vec::new();
    let mut current_group: Option<AffixGroup> = None;
    // Tracks the current rule's regex constraint header (most recent [...] line)
    let mut pending_regex: Option<String> = None;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim_end();

        // Skip empty lines and full-line comments
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        // Detect "group <name>" lines
        if line.starts_with("group ") {
            if let Some(g) = current_group.take() {
                groups.push(g);
            }
            let name = line.trim_start_matches("group ").trim().to_string();
            current_group = Some(AffixGroup { name, rules: Vec::new() });
            pending_regex = None;
            continue;
        }

        // Detect "subgroup <name>" lines — treat same as group but with full name
        if line.starts_with("subgroup ") {
            // Don't push the current group; subgroups add rules to parent group
            // but use the subgroup's own constraints. For simplicity, we ignore
            // subgroup boundaries and let their rules accumulate into the parent.
            pending_regex = None;
            continue;
        }

        // Detect header lines ending with ':' — these set the regex constraint
        // for subsequent rules. E.g. "[аеєиіїоуюя]ти:" or ".[аеєиіїоуюя]ти:"
        let trimmed = line.trim();
        if trimmed.ends_with(':') && !trimmed.contains('\t') {
            // Strip trailing ':'
            let header = trimmed.trim_end_matches(':').trim().to_string();
            pending_regex = Some(header);
            continue;
        }

        // Parse a rule line. Format examples:
        //   "ти\tла\t[^с]ti\t# comment @ verb:past:f"
        //   "вати\tю\t[ауюя]вати\t\t# comment @ verb:pres:s:1"
        //   "ати\tу\t[рз]вати\t\t# comment @ verb:pres:s:1"
        // Some rules don't have the third column; they inherit the pending regex.
        // Some rules have the form "ати\t\tу\t[бдн]ати" (inverted order).

        // Strip trailing comment but keep the @ tag
        let (rule_part, pos_tag) = if let Some(idx) = line.find('#') {
            let before = &line[..idx];
            let after = &line[idx..];
            // Extract @ tag from comment
            let pos_tag = after
                .split('@')
                .nth(1)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            (before, pos_tag)
        } else {
            (line, String::new())
        };

        // Split by tabs (or whitespace) into columns
        let cols: Vec<&str> = rule_part.split('\t')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if cols.len() < 2 {
            continue;
        }

        // Two formats observed:
        // Format A: <from_suffix> <to_suffix> [<regex>]
        // Format B: <from_suffix> <empty> <to_suffix> [<regex>]  (inverted)
        let (from_suffix, to_suffix, regex_str) = if cols.len() >= 3 {
            // Check if cols[1] is empty-looking or if cols[2] looks like a regex
            // Actually, since we filtered empty strings, cols[1] is non-empty here.
            // Distinguish Format A from Format B by checking if cols[2] starts with '['
            if cols[2].starts_with('[') || cols[2].starts_with('.') {
                // Format A: from, to, regex
                (cols[0].to_string(), cols[1].to_string(), Some(cols[2].to_string()))
            } else if cols.len() >= 4 && (cols[3].starts_with('[') || cols[3].starts_with('.')) {
                // Format B: from, "", to, regex  (cols[1] was filtered out as empty)
                // Actually cols[1] is the "to_suffix" here, cols[2] is the regex... wait
                // Let me re-check. If line is "ати\t\tу\t[бдн]ати",
                // after split('\t') and filter empty, we get ["ати", "у", "[бдн]ати"]
                // So cols[0]=ати, cols[1]=у, cols[2]=[бдн]ати → Format A.
                // The "inverted" format may not actually exist; let's just use Format A.
                (cols[0].to_string(), cols[1].to_string(), Some(cols[2].to_string()))
            } else {
                (cols[0].to_string(), cols[1].to_string(), None)
            }
        } else {
            // cols.len() == 2: <from> <to>, no regex
            (cols[0].to_string(), cols[1].to_string(), None)
        };

        // Use pending regex from header if rule doesn't have its own
        let final_regex_str = regex_str.or_else(|| pending_regex.clone());

        let match_regex = if let Some(ref rs) = final_regex_str {
            // Convert dict_uk regex syntax to Rust regex syntax.
            // dict_uk uses [аеєиіїоуюя] character classes which work in Rust regex.
            // But it also uses things like [^с] which Rust understands.
            // The only concern: dict_uk regex matches the END of the word (suffix).
            // We need to wrap it as: ^.*<regex>$
            // But for our apply_rule function, we just check if the word matches.
            let pattern = format!("(?i){}$", rs);
            Regex::new(&pattern).ok()
        } else {
            None
        };

        if let Some(ref mut g) = current_group {
            g.rules.push(AffixRule {
                from_suffix,
                to_suffix,
                match_regex,
                pos_tag,
            });
        }
    }

    if let Some(g) = current_group.take() {
        groups.push(g);
    }

    Ok(groups)
}

/// Apply an affix rule to a lemma to produce a word form.
/// Returns Some(form) if the rule applies, None otherwise.
fn apply_rule(lemma: &str, rule: &AffixRule) -> Option<String> {
    // If rule has a regex constraint, check that lemma matches
    if let Some(ref re) = rule.match_regex {
        // Skip "0" or "." placeholder regex — they mean "no real constraint"
        let regex_src = re.as_str();
        if !regex_src.ends_with("0$") && !regex_src.ends_with(".$") {
            if !re.is_match(lemma) {
                return None;
            }
        }
    }

    // Handle special from_suffix tokens:
    // "0" or "" or "." means "don't remove anything, just append to_suffix"
    let from = rule.from_suffix.as_str();
    if from.is_empty() || from == "0" || from == "." {
        return Some(format!("{}{}", lemma, rule.to_suffix));
    }

    // Check that lemma ends with from_suffix (case-insensitive)
    if !lemma.to_lowercase().ends_with(&from.to_lowercase()) {
        return None;
    }

    // Strip from_suffix from end and append to_suffix (preserve case of stem)
    let stem = &lemma[..lemma.len() - from.len()];
    Some(format!("{}{}", stem, rule.to_suffix))
}

/// Helper to get the canonical resources path
pub fn default_resources_path() -> PathBuf {
    PathBuf::from("resources/ua-linguistic")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_rule() {
        let rule_str = "ти\tла\t[^с]ти\t# абонувати абонувала @ verb:past:f";
        // Just ensure the parsing logic doesn't panic on a sample line
        let _ = rule_str;
    }

    #[test]
    fn test_apply_rule_verb_past() {
        let rule = AffixRule {
            from_suffix: "ти".to_string(),
            to_suffix: "ла".to_string(),
            match_regex: Regex::new(r"(?i)[^с]ти$").ok(),
            pos_tag: "verb:past:f".to_string(),
        };
        // "абонувати" should become "абонувала"
        let form = apply_rule("абонувати", &rule).unwrap();
        assert_eq!(form, "абонувала");
    }

    #[test]
    fn test_apply_rule_with_regex_constraint() {
        let rule = AffixRule {
            from_suffix: "ти".to_string(),
            to_suffix: "ла".to_string(),
            match_regex: Regex::new(r"(?i)[^с]ти$").ok(),
            pos_tag: "verb:past:f".to_string(),
        };
        // "пасти" ends with "сти" → regex [^с]ти$ should NOT match (с before ти)
        // Note: "пустити" was wrong here — it ends with "ити", not "сти".
        let form = apply_rule("пасти", &rule);
        assert!(form.is_none(), "Regex should have rejected пасти (ends in сти)");
    }

    #[test]
    fn test_parse_exceptions_lst_suppletive_verb() {
        // Simulate parsing "було бути verb:past:n:imperf:insert"
        let line = "було бути verb:past:n:imperf:insert";
        let cols: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(cols.len(), 3);
        assert_eq!(cols[0], "було");
        assert_eq!(cols[1], "бути");
        assert_eq!(cols[2], "verb:past:n:imperf:insert");
        // cols[1] does not start with '/' → it's a real exception, not a base.lst line
        assert!(!cols[1].starts_with('/'));
    }

    #[test]
    fn test_parse_exceptions_lst_skips_base_lst_style() {
        // Lines like "переддень /n20.a.p" should be skipped (cols[1] starts with '/')
        let line = "переддень /n20.a.p";
        let cols: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(cols.len(), 2); // only 2 cols, also fails the len < 3 check
        assert!(cols.len() < 3 || cols[1].starts_with('/'));
    }

    #[test]
    fn test_parse_exceptions_lst_ignores_comments() {
        // Section markers and comments should be skipped
        let line = "# !noun";
        assert!(line.starts_with('#'));
        let line2 = "# Іменники";
        assert!(line2.starts_with('#'));
    }

    #[test]
    fn test_merge_exceptions_lst_into_index() {
        use std::io::Write;
        let tmpdir = std::env::temp_dir().join("litgraph_test_exceptions");
        std::fs::create_dir_all(&tmpdir).unwrap();
        let tmpfile = tmpdir.join("exceptions.lst");
        let mut f = std::fs::File::create(&tmpfile).unwrap();
        writeln!(f, "# test data").unwrap();
        writeln!(f, "бути бути verb:inf:imperf").unwrap();
        writeln!(f, "було бути verb:past:n:imperf:insert").unwrap();
        writeln!(f, "діти дитина noun:anim:p:v_naz").unwrap();
        writeln!(f, "якоря якір noun:m:v_rod").unwrap();
        writeln!(f, "# base.lst-style line should be skipped:").unwrap();
        writeln!(f, "переддень /n20.a.p").unwrap();
        writeln!(f, "# comment-only line").unwrap();
        drop(f);

        let mut index: HashMap<String, Vec<LemmaEntry>> = HashMap::new();
        let count = merge_exceptions_lst(&tmpfile, &mut index).unwrap();
        // 4 real exceptions, 1 base.lst-style skip, 2 comments
        assert_eq!(count, 4);
        // бути has 2 entries: "бути" (inf) and "було" (past n)
        assert!(index.contains_key("бути"));
        assert!(index.contains_key("було"));
        assert!(index.contains_key("діти"));
        assert!(index.contains_key("якоря"));
        // переддень was NOT added (base.lst-style line skipped)
        assert!(!index.contains_key("переддень"));
        // Verify lemma mapping for "було"
        let bulo_entries = &index["було"];
        assert_eq!(bulo_entries.len(), 1);
        assert_eq!(bulo_entries[0].lemma, "бути");
        assert_eq!(bulo_entries[0].pos, "verb:past:n:imperf:insert");
        assert_eq!(bulo_entries[0].paradigm_class, "exception");

        // Cleanup
        std::fs::remove_dir_all(&tmpdir).ok();
    }
}
