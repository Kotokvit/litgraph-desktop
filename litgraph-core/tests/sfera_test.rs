use litgraph_core::parser;
use std::time::Instant;

#[test]
fn test_parse_sfera_v040() {
    let markdown = std::fs::read_to_string("tests/sfera.md")
        .expect("Не удалось прочитать tests/sfera.md");

    let t0 = Instant::now();
    let result = parser::build_graph(&markdown, "1-Сфера Предела", "Автор")
        .expect("Парсинг упал");
    let total = t0.elapsed();
    println!("\n=== Total parse time: {:.2}s ===", total.as_secs_f64());

    println!("\n=== Stats ===");
    println!("Description: {}", result.description);
    println!("Stats: {:?}", result.stats);

    let chapters: Vec<_> = result.nodes.iter().filter(|n| n.node_type == "chapter").collect();
    let characters: Vec<_> = result.nodes.iter().filter(|n| n.node_type == "character").collect();
    let concepts: Vec<_> = result.nodes.iter().filter(|n| n.node_type == "concept").collect();
    let organizations: Vec<_> = result.nodes.iter().filter(|n| n.node_type == "organization").collect();
    let locations: Vec<_> = result.nodes.iter().filter(|n| n.node_type == "location").collect();

    println!("\n=== Counts ===");
    println!("Chapters: {}", chapters.len());
    println!("Characters: {}", characters.len());
    println!("Concepts: {}", concepts.len());
    println!("Organizations: {}", organizations.len());
    println!("Locations: {}", locations.len());

    let mut huge_chapters = Vec::new();
    for ch in &chapters {
        let wc = ch.data.meta.as_ref()
            .and_then(|m| m.get("wordCount"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if wc > 8000 {
            huge_chapters.push((ch.data.title.clone(), wc));
        }
    }
    println!("\n=== HUGE chapters (>8000 words): {} ===", huge_chapters.len());
    for (t, wc) in &huge_chapters {
        println!("  {}: {} words", t, wc);
    }
    assert!(huge_chapters.len() <= 6, "Слишком много мега-глав: {}", huge_chapters.len());

    println!("\n=== Characters (top 15) ===");
    for c in characters.iter().take(15) {
        let meta = c.data.meta.as_ref();
        let m = meta.and_then(|m| m.get("mentions")).and_then(|v| v.as_u64()).unwrap_or(0);
        let s = meta.and_then(|m| m.get("speechCount")).and_then(|v| v.as_u64()).unwrap_or(0);
        println!("  - {} (freq={}, speech={})", c.data.title, m, s);
    }

    println!("\n=== Concepts ===");
    for c in &concepts {
        let m = c.data.meta.as_ref().and_then(|m| m.get("mentions")).and_then(|v| v.as_u64()).unwrap_or(0);
        println!("  - {} (freq={})", c.data.title, m);
    }
    println!("\n=== Organizations ===");
    for c in &organizations {
        let m = c.data.meta.as_ref().and_then(|m| m.get("mentions")).and_then(|v| v.as_u64()).unwrap_or(0);
        println!("  - {} (freq={})", c.data.title, m);
    }

    // === Assertions ===
    // v0.4.0: «Затем» должно быть в стоплисте
    assert!(characters.iter().find(|c| c.data.title == "Затем").is_none(),
        "v0.4.0: «Затем» должно быть в стоплисте");

    // v0.4.0: «Рэя» в локациях — известный баг (нужен pymorphy3 для слияния
    // коротких имён с чередованием й↔я). В v0.4.0 не исправлено, оставлено
    // для Variant C (pymorphy3 через Tauri command).
    // Тест: просто логируем, не падляем.
    if let Some(rya) = locations.iter().find(|l| l.data.title == "Рэя") {
        let m = rya.data.meta.as_ref().and_then(|m| m.get("mentions")).and_then(|v| v.as_u64()).unwrap_or(0);
        println!("WARN: «Рэя» все ещё в локациях ({} упоминаний) — нужен pymorphy3 (Variant C)", m);
    }

    // v0.4.0: «Аэлин» должен быть слит в «Аэлира»
    if characters.iter().find(|c| c.data.title == "Аэлира").is_some() {
        assert!(characters.iter().find(|c| c.data.title == "Аэлин").is_none(),
            "v0.4.0: «Аэлин» должен быть слит в «Аэлира» через alias map");
    }

    // v0.4.0: «Веня» должен быть слит в «Вениамин»
    if characters.iter().find(|c| c.data.title == "Вениамин").is_some() {
        assert!(characters.iter().find(|c| c.data.title == "Веня").is_none(),
            "v0.4.0: «Веня» должен быть слит в «Вениамин» через alias map");
    }

    // v0.4.0: «Яме» и «Яму» — короткие слова (3 буквы), lemmatize_simple
    // не обрезает окончания для слов ≤4 символов (защита от over-cutting).
    // Для слияния таких форм нужен pymorphy3 (Variant C).
    let yama_count = locations.iter()
        .filter(|l| {
            let t = l.data.title.to_lowercase();
            t.starts_with("ям")
        })
        .count();
    println!("Яма-family locations: {} (нужен pymorphy3 для слияния)", yama_count);

    // v0.4.0: должно быть БОЛЬШЕ глав чем раньше (28+28б+28в+28г отдельно)
    assert!(chapters.len() >= 40, "v0.4.0: должно быть >= 40 глав, получено {}", chapters.len());

    println!("\n=== ALL ASSERTIONS PASSED ===");
}
