use litgraph_core::parser;

fn main() {
    let markdown = std::fs::read_to_string("/home/z/my-project/upload/sfera-predela.md")
        .expect("Не удалось прочитать файл");
    
    // НЕ обрезаем оглавление в примере — пусть skip_table_of_contents сделает это
    let result = parser::build_graph(&markdown, "Сфера Предела", "Виталий Коток")
        .expect("Парсинг упал");
    
    println!("Nodes: {}", result.nodes.len());
    println!("Stats: {:?}", result.stats);
    
    let chapters: Vec<_> = result.nodes.iter().filter(|n| n.node_type == "chapter").collect();
    println!("\nГлав: {}", chapters.len());
    
    for (i, ch) in chapters.iter().enumerate() {
        let full_text_len = ch.data.full_text.as_ref().map(|t| t.len()).unwrap_or(0);
        let title: String = ch.data.title.chars().take(50).collect();
        let eps = ch.data.meta.as_ref().and_then(|m| m.get("epsilon"));
        println!("  {:2}. {:50} | текст: {:7} симв | ε={:?}", i+1, title, full_text_len, eps);
    }
}
