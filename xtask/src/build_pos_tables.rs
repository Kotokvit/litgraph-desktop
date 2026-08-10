//! xtask: build_pos_tables
//!
//! Parses LanguageTool UK `disambiguation.xml` (486 rules, 6 action types)
//! and `case_government.txt` (~38,100 verb→case mappings) into a compact
//! gzipped JSON artifact: `derivatives/pos_rules.json.gz`.
//!
//! ## XML Structure Summary (verified against lt_disambiguation.xml)
//!
//! ```xml
//! <rules lang="uk">
//!   <rule id="X" name="Y">
//!     <pattern>
//!       <token regexp="yes">мати|матері</token>
//!       <token postag="verb:.*:(pres|past)" postag_regexp="yes"/>
//!       <marker>
//!         <token>...</token>
//!       </marker>
//!     </pattern>
//!     <disambig action="replace" postag="noun:f:v_naz:anim"/>
//!     <!-- OR -->
//!     <disambig action="remove" postag="adj.*?:bad"/>
//!     <!-- OR -->
//!     <disambig postag="number:latin:bad"/>  <!-- implicit replace -->
//!     <example type="ambiguous" .../>
//!   </rule>
//! ```
//!
//! ## Action Type Frequencies
//!
//! | Action     | Meaning                                  | Count (approx) |
//! |------------|------------------------------------------|----------------|
//! | (none)     | Replace with `postag` attr (implicit)    | ~180           |
//! | replace    | Replace with `<wd pos="..."/>` content   | ~80            |
//! | remove     | Remove POS tags matching `postag` regex  | ~180           |
//! | add        | Inject extra POS tag                     | ~10            |
//! | filter     | Filter POS list by case                  | ~20            |
//! | filterall  | Apply filter to all tokens               | ~5             |
//! | immunize   | Lock token POS from future rules         | ~10            |
//!
//! ## Token Attributes
//!
//! - `regexp="yes"` → text is a regex
//! - `postag="..."` + `postag_regexp="yes"` → POS pattern (regex)
//! - `inflected="yes"` → match all inflected forms of lemma
//! - `case_sensitive="yes"` → do not lowercase
//! - `min="N"` / `max="N"` → distance window
//! - `negate="yes"` → invert match
//! - `skip="N"` → skip N tokens

use anyhow::{Context, Result};
use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use flate2::write::GzEncoder;
use flate2::Compression;

// ============================================================================
// Serialized Output Types — these are what `pos_tagger.rs` loads at runtime
// ============================================================================

/// One token-matching condition inside a disambiguation rule pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableTokenCondition {
    /// Literal text OR regex pattern (when `is_regexp` is true)
    pub text: Option<String>,
    /// POS tag pattern (regex when `is_postag_regexp`)
    pub postag: Option<String>,
    pub is_regexp: bool,
    pub is_postag_regexp: bool,
    pub inflected: bool,
    pub case_sensitive: bool,
    pub negate: bool,
    /// Distance window (None = exact position)
    pub min: Option<u32>,
    pub max: Option<u32>,
}

/// Action applied to the matched token(s).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableDisambigAction {
    /// "replace", "remove", "add", "filter", "filterall", "immunize"
    pub kind: String,
    /// Target POS tag to assign (for replace/add) or remove (for remove)
    pub postag: Option<String>,
}

/// One disambiguation rule — compiled from XML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableRule {
    pub id: String,
    pub name: String,
    /// Token conditions, in order. The `<marker>` block indicates which
    /// token indices the action applies to; we store marker range as
    /// (start, end) inclusive indices into `tokens`.
    pub tokens: Vec<SerializableTokenCondition>,
    pub marker_start: usize,
    pub marker_end: usize, // exclusive
    pub action: SerializableDisambigAction,
}

/// Top-level serialized artifact — what gets written to pos_rules.json.gz
#[derive(Debug, Serialize, Deserialize)]
pub struct PosRulesArtifact {
    pub version: String,
    pub rule_count: usize,
    pub case_government: HashMap<String, Vec<String>>, // verb -> ["v_zna", "v_oru", ...]
    pub rules: Vec<SerializableRule>,
}

// ============================================================================
// XML Parser (quick-xml event-based)
// ============================================================================

/// Mutable state for the streaming XML parser.
#[derive(Default)]
struct ParserState {
    rules: Vec<SerializableRule>,
    current_rule_id: String,
    current_rule_name: String,
    current_tokens: Vec<SerializableTokenCondition>,
    current_token: Option<SerializableTokenCondition>,
    current_action: Option<SerializableDisambigAction>,
    marker_start: usize,
    marker_end: usize,
    marker_was_set: bool,
    in_marker: bool,
    in_pattern: bool,
    in_disambig: bool,
    current_text: String,
    /// For `<disambig action="replace"><wd pos="X"/></disambig>`
    in_wd: bool,
    wd_pos: Option<String>,
}

/// Parse `disambiguation.xml` → list of compiled rules.
pub fn parse_disambiguation_xml(xml: &str) -> Result<Vec<SerializableRule>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut state = ParserState::default();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                state.handle_start(e.name().as_ref(), &e);
            }
            Ok(Event::Empty(e)) => {
                state.handle_start(e.name().as_ref(), &e);
                state.handle_end(e.name().as_ref());
            }
            Ok(Event::End(e)) => {
                state.handle_end(e.name().as_ref());
            }
            Ok(Event::Text(t)) => {
                // Capture text inside <token>...</token> or inside <wd>...</wd>
                if state.current_token.is_some() || state.in_wd {
                    if let Ok(text) = t.unescape() {
                        state.current_text.push_str(&text);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow::anyhow!("XML parse error: {}", e)),
            _ => (),
        }
        buf.clear();
    }

    Ok(state.rules)
}

impl ParserState {
    fn handle_start(&mut self, name: &[u8], e: &quick_xml::events::BytesStart) {
        match name {
            b"rule" => {
                self.current_rule_id.clear();
                self.current_rule_name.clear();
                self.current_tokens.clear();
                self.current_action = None;
                self.marker_start = 0;
                self.marker_end = 0;
                self.marker_was_set = false;
                self.in_marker = false;
                self.in_pattern = false;
                self.in_disambig = false;
                self.in_wd = false;
                self.wd_pos = None;
                self.current_text.clear();
                self.current_token = None;

                for attr in e.attributes().flatten() {
                    match attr.key.as_ref() {
                        b"id" => {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                self.current_rule_id = v.to_string();
                            }
                        }
                        b"name" => {
                            if let Ok(v) = std::str::from_utf8(&attr.value) {
                                self.current_rule_name = v.to_string();
                            }
                        }
                        _ => (),
                    }
                }
            }
            b"pattern" => {
                self.in_pattern = true;
            }
            b"marker" => {
                self.in_marker = true;
                self.marker_was_set = true;
                self.marker_start = self.current_tokens.len();
            }
            b"token" => {
                let mut cond = SerializableTokenCondition {
                    text: None,
                    postag: None,
                    is_regexp: false,
                    is_postag_regexp: false,
                    inflected: false,
                    case_sensitive: false,
                    negate: false,
                    min: None,
                    max: None,
                };

                for attr in e.attributes().flatten() {
                    let key = attr.key.as_ref();
                    let val = std::str::from_utf8(&attr.value).unwrap_or("");
                    let val_bool = val == "yes" || val == "true";
                    match key {
                        b"regexp" => cond.is_regexp = val_bool,
                        b"postag" => cond.postag = Some(val.to_string()),
                        b"postag_regexp" => cond.is_postag_regexp = val_bool,
                        b"inflected" => cond.inflected = val_bool,
                        b"case_sensitive" => cond.case_sensitive = val_bool,
                        b"negate" => cond.negate = val_bool,
                        b"negate_pos" => cond.negate = val_bool,
                        b"min" => cond.min = val.parse().ok(),
                        b"max" => cond.max = val.parse().ok(),
                        _ => (),
                    }
                }

                // Text content (if any) is captured separately via Event::Text
                // For empty <token postag="..."/> there is no text.
                // For <token regexp="yes">мати|матері</token>, text comes next.
                // We handle the text in handle_text by checking if current_token is Some.
                self.current_token = Some(cond);
            }
            b"disambig" => {
                self.in_disambig = true;
                let mut action = SerializableDisambigAction {
                    kind: "replace".to_string(), // default when no action attr
                    postag: None,
                };
                let mut has_action_attr = false;
                for attr in e.attributes().flatten() {
                    let key = attr.key.as_ref();
                    let val = std::str::from_utf8(&attr.value).unwrap_or("");
                    match key {
                        b"action" => {
                            action.kind = val.to_string();
                            has_action_attr = true;
                        }
                        b"postag" => action.postag = Some(val.to_string()),
                        _ => (),
                    }
                }
                // If no action attribute but postag is present, it's an implicit "replace"
                if !has_action_attr && action.postag.is_some() {
                    action.kind = "replace".to_string();
                }
                self.current_action = Some(action);
            }
            b"wd" => {
                // Inside <disambig action="replace"><wd pos="X"/></disambig>
                self.in_wd = true;
                for attr in e.attributes().flatten() {
                    if attr.key.as_ref() == b"pos" {
                        if let Ok(v) = std::str::from_utf8(&attr.value) {
                            self.wd_pos = Some(v.to_string());
                        }
                    }
                }
            }
            _ => (),
        }
    }

    fn handle_end(&mut self, name: &[u8]) {
        match name {
            b"rule" => {
                if let Some(action) = self.current_action.take() {
                    // If <marker> was never set, default applies to LAST token only
                    let (marker_start, marker_end) = if self.marker_was_set {
                        // Marker was set; if marker_end wasn't captured
                        // (e.g. self-closing or rule ended inside), default to current tokens length
                        let me = if self.marker_end == 0 {
                            self.current_tokens.len()
                        } else {
                            self.marker_end
                        };
                        (self.marker_start, me)
                    } else if !self.current_tokens.is_empty() {
                        // No marker: apply to last token
                        let last = self.current_tokens.len() - 1;
                        (last, last + 1)
                    } else {
                        (0, 0)
                    };

                    self.rules.push(SerializableRule {
                        id: std::mem::take(&mut self.current_rule_id),
                        name: std::mem::take(&mut self.current_rule_name),
                        tokens: std::mem::take(&mut self.current_tokens),
                        marker_start,
                        marker_end,
                        action,
                    });
                }
                self.in_pattern = false;
                self.in_marker = false;
                self.marker_was_set = false;
                self.in_disambig = false;
            }
            b"pattern" => {
                self.in_pattern = false;
            }
            b"marker" => {
                self.in_marker = false;
                self.marker_end = self.current_tokens.len();
            }
            b"token" => {
                if let Some(mut tok) = self.current_token.take() {
                    // Capture any pending text
                    if !self.current_text.is_empty() {
                        tok.text = Some(std::mem::take(&mut self.current_text));
                    }
                    self.current_tokens.push(tok);
                }
            }
            b"disambig" => {
                self.in_disambig = false;
                // If we have a wd_pos, set it as the action's postag
                if let Some(ref mut action) = self.current_action {
                    if action.postag.is_none() {
                        if let Some(pos) = self.wd_pos.take() {
                            action.postag = Some(pos);
                        }
                    }
                }
                self.in_wd = false;
            }
            b"wd" => {
                self.in_wd = false;
                // Don't clear wd_pos here — disambig closing will pick it up
            }
            _ => (),
        }
    }
}

// ============================================================================
// case_government.txt parser
// ============================================================================

/// Parse lines like "абонувати v_zna:v_oru:v_dav" → ("абонувати", ["v_zna", "v_oru", "v_dav"])
pub fn parse_case_government(content: &str) -> HashMap<String, Vec<String>> {
    let mut map = HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((verb, cases_str)) = line.split_once(char::is_whitespace) {
            let verb = verb.trim().to_lowercase();
            let cases: Vec<String> = cases_str
                .trim()
                .split(':')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !verb.is_empty() && !cases.is_empty() {
                map.entry(verb).or_insert(cases);
            }
        }
    }
    map
}

// ============================================================================
// Top-level runner (called from xtask main.rs)
// ============================================================================

/// Entry point: read XML + txt from `resources/ua-linguistic/languagetool/`,
/// write `derivatives/pos_rules.json.gz`.
pub fn run(languagetool_dir: &Path, out_path: &Path) -> Result<()> {
    println!("=== build-pos-tables: compiling LanguageTool UK resources ===");

    let disambig_path = languagetool_dir.join("lt_disambiguation.xml");
    let case_gov_path = languagetool_dir.join("lt_case_government.txt");

    let xml = std::fs::read_to_string(&disambig_path)
        .with_context(|| format!("reading {}", disambig_path.display()))?;
    let case_gov_text = std::fs::read_to_string(&case_gov_path)
        .with_context(|| format!("reading {}", case_gov_path.display()))?;

    println!("Parsing disambiguation.xml ({} bytes)...", xml.len());
    let rules = parse_disambiguation_xml(&xml)?;
    println!("  → {} rules compiled", rules.len());

    println!("Parsing case_government.txt ({} bytes)...", case_gov_text.len());
    let case_government = parse_case_government(&case_gov_text);
    println!("  → {} verb→case entries", case_government.len());

    // Sanity-check: validate that regex patterns compile
    let mut regex_failures = 0;
    for rule in &rules {
        for tok in &rule.tokens {
            if let Some(text) = &tok.text {
                if tok.is_regexp {
                    if Regex::new(text).is_err() {
                        regex_failures += 1;
                    }
                }
            }
            if let Some(pos) = &tok.postag {
                if tok.is_postag_regexp {
                    if Regex::new(pos).is_err() {
                        regex_failures += 1;
                    }
                }
            }
        }
    }
    if regex_failures > 0 {
        println!("  ⚠ {} regex patterns failed to compile (will be skipped at runtime)", regex_failures);
    }

    let artifact = PosRulesArtifact {
        version: "1.0.0".to_string(),
        rule_count: rules.len(),
        case_government,
        rules,
    };

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file = File::create(out_path)
        .with_context(|| format!("creating {}", out_path.display()))?;
    let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
    let json = serde_json::to_string(&artifact)?;
    encoder.write_all(json.as_bytes())?;
    encoder.finish()?;

    let out_size = std::fs::metadata(out_path)?.len();
    println!("✓ written {} ({} KB)", out_path.display(), out_size / 1024);
    println!("=== build-pos-tables: DONE ===");
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_case_government_basic() {
        // Note: lines starting with '#' are comments and skipped (e.g. "#згідно v_rod")
        let text = "# v_oru\n#згідно v_rod\nабонувати v_zna:v_oru:v_dav\nбоятися v_rod\nзгідно v_rod\n";
        let map = parse_case_government(text);
        assert_eq!(map.get("абонувати"), Some(&vec![
            "v_zna".to_string(),
            "v_oru".to_string(),
            "v_dav".to_string(),
        ]));
        assert_eq!(map.get("боятися"), Some(&vec!["v_rod".to_string()]));
        // "згідно" appears on its own line (no # prefix) → should be in map
        assert_eq!(map.get("згідно"), Some(&vec!["v_rod".to_string()]));
        // comments and empty lines skipped
        assert!(!map.contains_key(""));
    }

    #[test]
    fn test_parse_disambiguation_simple_replace() {
        let xml = r#"<?xml version="1.0"?>
<rules lang="uk">
  <rule id="TEST1" name="Test replace">
    <pattern>
      <marker>
        <token regexp="yes">мати|матері</token>
      </marker>
      <token postag="verb:.*" postag_regexp="yes"/>
    </pattern>
    <disambig action="replace" postag="noun:f:v_naz:anim"/>
  </rule>
</rules>"#;
        let rules = parse_disambiguation_xml(xml).unwrap();
        assert_eq!(rules.len(), 1);
        let r = &rules[0];
        assert_eq!(r.id, "TEST1");
        assert_eq!(r.name, "Test replace");
        assert_eq!(r.tokens.len(), 2);
        assert_eq!(r.tokens[0].text.as_deref(), Some("мати|матері"));
        assert!(r.tokens[0].is_regexp);
        assert_eq!(r.tokens[1].postag.as_deref(), Some("verb:.*"));
        assert!(r.tokens[1].is_postag_regexp);
        // marker covers only token 0
        assert_eq!(r.marker_start, 0);
        assert_eq!(r.marker_end, 1);
        assert_eq!(r.action.kind, "replace");
        assert_eq!(r.action.postag.as_deref(), Some("noun:f:v_naz:anim"));
    }

    #[test]
    fn test_parse_disambiguation_implicit_replace() {
        // <disambig postag="X"/> with no action attr → implicit replace
        let xml = r#"<?xml version="1.0"?>
<rules lang="uk">
  <rule id="IMP1" name="Implicit">
    <pattern>
      <marker><token case_sensitive="yes">І</token></marker>
    </pattern>
    <disambig postag="number:latin:bad" />
  </rule>
</rules>"#;
        let rules = parse_disambiguation_xml(xml).unwrap();
        assert_eq!(rules.len(), 1);
        let r = &rules[0];
        assert_eq!(r.action.kind, "replace");
        assert_eq!(r.action.postag.as_deref(), Some("number:latin:bad"));
        assert!(r.tokens[0].case_sensitive);
    }

    #[test]
    fn test_parse_disambiguation_remove_action() {
        let xml = r#"<?xml version="1.0"?>
<rules lang="uk">
  <rule id="RM1" name="Remove bad adj">
    <pattern>
      <marker><token postag="adj.*" postag_regexp="yes"/></marker>
    </pattern>
    <disambig action="remove" postag="adj.*?:bad.*"/>
  </rule>
</rules>"#;
        let rules = parse_disambiguation_xml(xml).unwrap();
        let r = &rules[0];
        assert_eq!(r.action.kind, "remove");
        assert_eq!(r.action.postag.as_deref(), Some("adj.*?:bad.*"));
    }

    #[test]
    fn test_parse_disambiguation_wd_inner() {
        // <disambig action="replace"><wd pos="noninfl"/></disambig>
        let xml = r#"<?xml version="1.0"?>
<rules lang="uk">
  <rule id="WD1" name="Wd replace">
    <pattern>
      <marker><token regexp="yes">[0-9]+-[а-жєґ]</token></marker>
    </pattern>
    <disambig action="replace"><wd pos="noninfl"/></disambig>
  </rule>
</rules>"#;
        let rules = parse_disambiguation_xml(xml).unwrap();
        let r = &rules[0];
        assert_eq!(r.action.kind, "replace");
        assert_eq!(r.action.postag.as_deref(), Some("noninfl"));
    }

    #[test]
    fn test_marker_no_marker_block() {
        // No <marker> → action applies to last token (fallback)
        let xml = r#"<?xml version="1.0"?>
<rules lang="uk">
  <rule id="NM1" name="No marker">
    <pattern>
      <token regexp="yes">номер</token>
      <token regexp="yes">[0-9]+</token>
    </pattern>
    <disambig action="replace" postag="noninfl"/>
  </rule>
</rules>"#;
        let rules = parse_disambiguation_xml(xml).unwrap();
        let r = &rules[0];
        assert_eq!(r.tokens.len(), 2);
        // No marker: marker_start defaults to last token, marker_end to tokens.len()
        assert_eq!(r.marker_start, 1);
        assert_eq!(r.marker_end, 2);
    }
}
