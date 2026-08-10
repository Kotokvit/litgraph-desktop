#[test]
fn test_vozvrashatsja_debug() {
    let text = "Мы возвращались обратно домой";
    let lc = text.to_lowercase();
    println!("lc = {:?}", lc);
    
    // Test stem matching manually
    let alt = "возвращаться";
    let chars: Vec<char> = alt.chars().collect();
    println!("alt chars = {} chars", chars.len());
    
    let stem2: String = chars[..chars.len() - 2].iter().collect();
    println!("stem (len-2) = {:?}", stem2);
    println!("text contains stem2? {}", lc.contains(&stem2));
    
    let stem3: String = chars[..chars.len() - 3].iter().collect();
    println!("stem (len-3) = {:?}", stem3);
    println!("text contains stem3? {}", lc.contains(&stem3));
}
