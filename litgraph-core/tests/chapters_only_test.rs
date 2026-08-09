//! Профилирование chapters::detect отдельно
use litgraph_core::parser::chapters;
use std::time::Instant;

#[test]
fn test_chapters_only() {
    let markdown = std::fs::read_to_string("tests/sfera.md")
        .expect("Не удалось прочитать tests/sfera.md");

    let t0 = Instant::now();
    let (chapters, _prologue) = chapters::detect(&markdown);
    let elapsed = t0.elapsed();
    println!("\n=== chapters::detect: {:.2}s, {} chapters ===", elapsed.as_secs_f64(), chapters.len());

    for (i, ch) in chapters.iter().enumerate() {
        let wc = ch.full_text.split_whitespace().count();
        println!("  {}. {} ({} words)", i+1, ch.title, wc);
    }
}
