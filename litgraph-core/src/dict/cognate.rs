//! Cognate dictionary and LanguageTool token normalization module.

use phf::Map;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    super::generated_cognates::COGNATE_MAP
        .get(token.trim().to_lowercase().as_str())
        .map(|e| (e.target, e.weight, e.source_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_manual_cognates() {
        let res = normalize_token("Олексій");
        assert!(res.is_some());
        let (tgt, w, st) = res.unwrap();
        assert_eq!(tgt, "олексій");
        assert_eq!(w, 1.0);
        assert_eq!(st, SourceType::Manual);
    }

    #[test]
    fn test_normalize_barbarism() {
        if let Some((tgt, _w, st)) = normalize_token("авіанальот") {
            assert_eq!(tgt, "авіаналіт");
            assert_eq!(st, SourceType::Barbarism);
        }
    }
}
