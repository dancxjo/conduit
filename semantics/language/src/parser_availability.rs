//! Exact Source schemas for a finite prefix of complete lexical occurrences.
//! Availability does not grant finality, stability, commitment or playback.
pub fn parser_availability_types(
) -> alloc::vec::Vec<(&'static str, conduit_core::StructuredInfoType)> {
    alloc::vec![
        (
            "LanguageParserAvailableLexical",
            crate::LanguageParserAvailableLexical::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserAvailability",
            crate::LanguageParserAvailability::semantic_type().expect("checked Language Type")
        ),
        (
            "LanguageParserRawAvailability",
            crate::LanguageParserRawAvailability::semantic_type().expect("checked Language Type")
        ),
    ]
}
