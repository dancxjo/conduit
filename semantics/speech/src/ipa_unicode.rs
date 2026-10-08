//! Supported Unicode IPA unit syntax, version1. This is a conservative notation
//! subset, not a universal inventory or a phoneme-to-phone mapping. No Unicode
//! normalization or ASCII/provider conversion occurs here. Other combinations
//! require an explicit future profile extension and currently refuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UnitKind {
    Segment,
    PrimaryStress,
    SecondaryStress,
    Length,
    SyllableBoundary,
}

pub(crate) const SUPPORTED_SEGMENTS: &[&str] = &[
    "p", "b", "t", "d", "k", "ɡ", "m", "n", "ŋ", "f", "v", "θ", "ð", "s", "z", "ʃ", "ʒ", "h", "ɹ",
    "r", "ɾ", "l", "j", "w", "i", "ɪ", "e", "ɛ", "æ", "a", "ɑ", "ɒ", "ɔ", "o", "ʊ", "u", "ə", "ʌ",
    "ɜ", "t͡ʃ", "d͡ʒ", "pʰ", "tʰ", "kʰ", "n̩", "ã", "ã",
];

pub(crate) fn supported_unit(spelling: &str, kind: UnitKind) -> bool {
    match kind {
        UnitKind::PrimaryStress => spelling == "ˈ",
        UnitKind::SecondaryStress => spelling == "ˌ",
        UnitKind::Length => spelling == "ː",
        UnitKind::SyllableBoundary => spelling == ".",
        UnitKind::Segment => SUPPORTED_SEGMENTS.contains(&spelling),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declared_unicode_units_survive_without_ascii_reinterpretation() {
        for unit in [
            "ɪ", "ə", "θ", "ð", "ʃ", "ʒ", "ŋ", "ɹ", "j", "ɡ", "t͡ʃ", "d͡ʒ", "pʰ", "tʰ", "kʰ", "ɾ",
            "n̩", "ã", "ã",
        ] {
            assert!(supported_unit(unit, UnitKind::Segment), "{unit}");
        }
        for unit in [
            "ih",
            "ax",
            "th",
            "ch",
            "p_aspirated",
            "g",
            "tʃ",
            "a̩",
            "p̃",
            "n̩ʰ",
            "͡",
            "̃",
        ] {
            assert!(!supported_unit(unit, UnitKind::Segment), "{unit}");
        }
    }
    #[test]
    fn suprasegmentals_have_distinct_kinds() {
        for (unit, kind) in [
            ("ˈ", UnitKind::PrimaryStress),
            ("ˌ", UnitKind::SecondaryStress),
            ("ː", UnitKind::Length),
            (".", UnitKind::SyllableBoundary),
        ] {
            assert!(supported_unit(unit, kind));
            assert!(!supported_unit(unit, UnitKind::Segment));
        }
        assert!(!supported_unit("ˈ", UnitKind::SecondaryStress));
    }
}
