//! Интеграционный тест: парсер .md → граф на реальном файле Касіопеи.
//! Ожидаемый результат (из TS-прототипа):
//! - 60 нод глав (с прологом)
//! - ~25 персонажей
//! - ~15 локаций
//! - 10 тем
//! - ~730 связей
//! - 103342 слов

use litgraph_core::parser;

#[test]
fn test_parse_kasiopia() {
    let markdown = std::fs::read_to_string("tests/kasiopia.md")
        .expect("Не удалось прочитать tests/kasiopia.md");

    let result = parser::build_graph(&markdown, "Касіопея", "Автор")
        .expect("Парсинг упал");

    println!("Title: {}", result.title);
    println!("Nodes: {}", result.nodes.len());
    println!("Edges: {}", result.edges.len());
    println!("Stats: {:?}", result.stats);

    // Главы (включая пролог)
    let chapters = result.nodes.iter().filter(|n| n.node_type == "chapter").count();
    let characters = result.nodes.iter().filter(|n| n.node_type == "character").count();
    let locations = result.nodes.iter().filter(|n| n.node_type == "location").count();
    let themes = result.nodes.iter().filter(|n| n.node_type == "theme").count();

    println!("Chapters: {}", chapters);
    println!("Characters: {}", characters);
    println!("Locations: {}", locations);
    println!("Themes: {}", themes);

    // Проверки
    assert!(chapters >= 55, "Должно быть минимум 55 глав (с прологом), получено {}", chapters);
    assert!(characters >= 15, "Должно быть минимум 15 персонажей, получено {}", characters);
    assert!(locations >= 5, "Должно быть минимум 5 локаций, получено {}", locations);
    assert!(themes >= 5, "Должно быть минимум 5 тем, получено {}", themes);
    assert!(result.edges.len() >= 100, "Должно быть минимум 100 связей, получено {}", result.edges.len());
    assert!(result.stats.words >= 100000, "Должно быть ~100k слов, получено {}", result.stats.words);

    // Проверим что у всех глав есть full_text
    let chapters_with_fulltext = result.nodes.iter()
        .filter(|n| n.node_type == "chapter" && n.data.full_text.is_some())
        .count();
    println!("Chapters with fullText: {}", chapters_with_fulltext);
    assert!(chapters_with_fulltext >= 50, "Минимум 50 глав с полным текстом, получено {}", chapters_with_fulltext);

    // Покажем топ-5 тем
    let mut themes_list: Vec<_> = result.nodes.iter()
        .filter(|n| n.node_type == "theme")
        .collect();
    themes_list.sort_by_key(|n| {
        n.data.meta.as_ref()
            .and_then(|m| m.get("mentions"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    });
    themes_list.reverse();
    println!("\nТоп-5 тем:");
    for t in themes_list.iter().take(5) {
        let mentions = t.data.meta.as_ref()
            .and_then(|m| m.get("mentions"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        println!("  - {}: {} упоминаний", t.data.title, mentions);
    }
}

#[test]
fn test_simple_text() {
    let markdown = r#"Глава 1: Начало
Это первая глава. Анна пошла в лес. Лес был тёмный.

Глава 2: Продолжение
Анна вернулась. Она встретила Бориса. Борис сказал: "Привет, Анна!"
"#;

    let result = parser::build_graph(markdown, "Тест", "Тест").expect("Парсинг упал");

    println!("Simple test:");
    println!("  Nodes: {}", result.nodes.len());
    println!("  Edges: {}", result.edges.len());
    println!("  Stats: {:?}", result.stats);

    assert!(result.stats.chapters >= 2, "Должно быть минимум 2 главы");
}

#[test]
fn test_empty_text() {
    let result = parser::build_graph("", "Пусто", "");
    assert!(result.is_err());
}
