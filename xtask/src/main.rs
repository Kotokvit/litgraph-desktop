use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Debug, Clone)]
struct RawCognate {
    target: String,
    weight: f32,
    source_type: &'static str, // "Barbarism", "Spelling", "Grammar", "Manual"
}

fn main() -> Result<()> {
    println!("=== LitGraph Cognates & LanguageTool Weights Generator ===");

    let mut entries: HashMap<String, RawCognate> = HashMap::new();

    // 1. Hardcoded manual cognates (RU <-> UK names & terms)
    let manual_pairs = vec![
        ("алексей", "олексій", 1.0, "Manual"),
        ("петр", "петро", 1.0, "Manual"),
        ("пётр", "петро", 1.0, "Manual"),
        ("александр", "олександр", 1.0, "Manual"),
        ("михаил", "михайло", 1.0, "Manual"),
        ("евгений", "євген", 1.0, "Manual"),
        ("владимир", "володимир", 1.0, "Manual"),
        ("николай", "микола", 1.0, "Manual"),
        ("дмитрий", "дмитро", 1.0, "Manual"),
        ("иван", "іван", 1.0, "Manual"),
        ("анна", "анна", 1.0, "Manual"),
        ("мария", "марія", 1.0, "Manual"),
        ("елена", "олена", 1.0, "Manual"),
        ("екатерина", "катерина", 1.0, "Manual"),
        ("наталья", "наталія", 1.0, "Manual"),
        ("сфера предела", "сфера предела", 1.0, "Manual"),
    ];

    for (src, tgt, w, st) in manual_pairs {
        entries.insert(
            src.to_string(),
            RawCognate {
                target: tgt.to_string(),
                weight: w,
                source_type: st,
            },
        );
        // Bidirectional mapping for cross-language resolution
        // (RU↔UK cognates — both directions are valid; unlike LT replace.txt
        // where reverse direction would "normalize" correct word into error)
        if src != tgt {
            entries.insert(
                tgt.to_string(),
                RawCognate {
                    target: src.to_string(),
                    weight: w,
                    source_type: st,
                },
            );
        }
    }

    // 2. Fetch LanguageTool UK replace.txt
    println!("Fetching LanguageTool UK replace.txt...");
    if let Ok(uk_replace) = fetch_text("https://raw.githubusercontent.com/languagetool-org/languagetool/master/languagetool-language-modules/uk/src/main/resources/org/languagetool/rules/uk/replace.txt") {
        parse_replace_txt(&uk_replace, 0.95, "Barbarism", &mut entries);
    }

    // 3. Fetch LanguageTool RU replace.txt
    println!("Fetching LanguageTool RU replace.txt...");
    if let Ok(ru_replace) = fetch_text("https://raw.githubusercontent.com/languagetool-org/languagetool/master/languagetool-language-modules/ru/src/main/resources/org/languagetool/rules/ru/replace.txt") {
        parse_replace_txt(&ru_replace, 0.90, "Spelling", &mut entries);
    }

    // 4. Fetch LanguageTool UK grammar-barbarism.xml
    println!("Fetching LanguageTool UK grammar-barbarism.xml...");
    if let Ok(uk_xml) = fetch_text("https://raw.githubusercontent.com/languagetool-org/languagetool/master/languagetool-language-modules/uk/src/main/resources/org/languagetool/rules/uk/grammar-barbarism.xml") {
        parse_lt_xml(&uk_xml, 0.95, "Barbarism", &mut entries);
    }

    println!("Total deduplicated cognate entries collected: {}", entries.len());

    // 5. Generate output files
    let out_paths = vec![
        "src-tauri/src/dict/generated_cognates.rs",
        "litgraph-core/src/dict/generated_cognates.rs",
    ];

    for path in out_paths {
        if let Some(parent) = Path::new(path).parent() {
            std::fs::create_dir_all(parent)?;
        }
        write_phf_map(path, &entries)?;
        println!("Generated {}", path);
    }

    println!("Done!");
    Ok(())
}

fn fetch_text(url: &str) -> Result<String> {
    let resp = reqwest::blocking::get(url)?.text()?;
    Ok(resp)
}

fn parse_replace_txt(content: &str, default_weight: f32, source_type: &'static str, entries: &mut HashMap<String, RawCognate>) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((wrong, correct_list)) = line.split_once('=') {
            let key = wrong.trim().to_lowercase();
            let first_suggestion = correct_list.split('|').next().unwrap_or("").trim();
            // Remove explanatory parens in suggestions e.g. "поглинальний (вбиральний)" -> "поглинальний"
            let clean_target = first_suggestion
                .split('(')
                .next()
                .unwrap_or(first_suggestion)
                .split(';')
                .next()
                .unwrap_or(first_suggestion)
                .trim()
                .to_lowercase();

            if !key.is_empty() && !clean_target.is_empty() && key != clean_target {
                entries.entry(key).or_insert(RawCognate {
                    target: clean_target,
                    weight: default_weight,
                    source_type,
                });
            }
        }
    }
}

fn parse_lt_xml(content: &str, default_weight: f32, source_type: &'static str, entries: &mut HashMap<String, RawCognate>) {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(content);
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                if e.name().as_ref() == b"example" {
                    let mut correction = None;
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"correction" {
                            if let Ok(val) = std::str::from_utf8(&attr.value) {
                                correction = Some(val.to_string());
                            }
                        }
                    }
                    if let Some(corr) = correction {
                        let target = corr.split('|').next().unwrap_or("").trim().to_lowercase();
                        if let Ok(Event::Text(t)) = reader.read_event_into(&mut buf) {
                            if let Ok(text) = t.unescape() {
                                let key = text.trim().to_lowercase();
                                if !key.is_empty() && !target.is_empty() && key != target {
                                    entries.entry(key).or_insert(RawCognate {
                                        target,
                                        weight: default_weight,
                                        source_type,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => (),
        }
        buf.clear();
    }
}

fn write_phf_map(output_path: &str, entries: &HashMap<String, RawCognate>) -> Result<()> {
    let mut file = BufWriter::new(File::create(output_path)?);

    writeln!(file, "// Auto-generated by xtask. DO NOT EDIT MANUALLY.\n")?;
    writeln!(file, "use super::cognate::{{CognateEntry, SourceType}};\n")?;

    let mut map_builder = phf_codegen::Map::new();

    for (key, val) in entries {
        let entry_str = format!(
            "CognateEntry {{ target: {:?}, weight: {:.2}, source_type: SourceType::{} }}",
            val.target, val.weight, val.source_type
        );
        map_builder.entry(key.as_str(), &entry_str);
    }

    writeln!(
        file,
        "pub static COGNATE_MAP: phf::Map<&'static str, CognateEntry> = {};",
        map_builder.build()
    )?;

    Ok(())
}
