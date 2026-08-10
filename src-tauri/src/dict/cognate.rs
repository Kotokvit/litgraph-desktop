//! Cognate dictionary and LanguageTool token normalization module.

use phf::Map;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum SourceType {
    Barbarism = 1,
    Spelling = 2,
    Grammar = 3,
    Manual = 4,
}

#[derive(Debug, Clone, Copy)]
pub struct CognateEntry {
    pub target: &'static str,
    pub weight: f32,
    pub source_type: SourceType,
}

pub type CognateMap = Map<&'static str, CognateEntry>;

impl CognateEntry {
    pub const fn new(target: &'static str, weight: f32, source_type: SourceType) -> Self {
        Self {
            target,
            weight,
            source_type,
        }
    }
}

pub fn normalize_token(token: &str) -> Option<(&'static str, f32, SourceType)> {
    let lower = token.trim().to_lowercase();
    // Исключаем местоимения RU/UK из автоматической нормализации (предотвращает ошибочный маппинг «вона» -> «листуватися»).
    if matches!(
        lower.as_str(),
        "он" | "она" | "оно" | "они" | "він" | "вона" | "воно" | "вони" | "я" | "ти" | "мы" | "вы" | "ми" | "ви" | "це" | "ця" | "цю" | "цей" | "цим"
    ) {
        return None;
    }
    super::generated_cognates::COGNATE_MAP
        .get(lower.as_str())
        .map(|e| (e.target, e.weight, e.source_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_manual_cognates() {
        let res = normalize_token("Алексей");
        assert!(res.is_some());
        let (tgt, w, st) = res.unwrap();
        assert_eq!(tgt, "олексій");
        assert_eq!(w, 1.0);
        assert_eq!(st, SourceType::Manual);

        let res_rev = normalize_token("Олексій");
        assert!(res_rev.is_some());
        let (tgt_rev, _, _) = res_rev.unwrap();
        assert_eq!(tgt_rev, "алексей");
    }

    #[test]
    fn test_normalize_barbarism() {
        if let Some((tgt, _w, st)) = normalize_token("авіанальот") {
            assert_eq!(tgt, "авіаналіт");
            assert_eq!(st, SourceType::Barbarism);
        }
    }

    #[test]
    fn test_pronouns_not_normalized() {
        assert!(normalize_token("вона").is_none());
        assert!(normalize_token("он").is_none());
        assert!(normalize_token("она").is_none());
        assert!(normalize_token("він").is_none());
        assert!(normalize_token("цю").is_none());
    }
}

