//! Профилирование: какая часть парсера самая медленная?
use litgraph_core::parser;
use std::time::Instant;

#[test]
fn test_profile_sfera() {
    let markdown = std::fs::read_to_string("tests/sfera.md")
        .expect("Не удалось прочитать tests/sfera.md");

    // Profile each step individually
    // We can't call internal functions directly, but we can measure total
    // and compare with smaller texts.

    let t0 = Instant::now();
    let result = parser::build_graph(&markdown, "1-Сфера Предела", "Автор")
        .expect("Парсинг упал");
    let total = t0.elapsed();
    println!("\n=== Total: {:.2}s ===", total.as_secs_f64());

    println!("Chapters: {}", result.nodes.iter().filter(|n| n.node_type == "chapter").count());
    println!("Characters: {}", result.nodes.iter().filter(|n| n.node_type == "character").count());
    println!("Concepts: {}", result.nodes.iter().filter(|n| n.node_type == "concept").count());
    println!("Organizations: {}", result.nodes.iter().filter(|n| n.node_type == "organization").count());
    println!("Locations: {}", result.nodes.iter().filter(|n| n.node_type == "location").count());

    // Show top 5 chapters by word count
    let mut chs: Vec<_> = result.nodes.iter()
        .filter(|n| n.node_type == "chapter")
        .map(|n| {
            let wc = n.data.meta.as_ref().and_then(|m| m.get("wordCount")).and_then(|v| v.as_u64()).unwrap_or(0);
            (n.data.title.clone(), wc)
        })
        .collect();
    chs.sort_by(|a, b| b.1.cmp(&a.1));
    println!("\nTop 5 chapters by word count:");
    for (t, wc) in chs.iter().take(5) {
        println!("  {}: {} words", t, wc);
    }
}
