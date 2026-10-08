//! Closed numerical custody profile seam. A profile binds full generated Native
//! Types and fixed Source entries; implementing it is library-owned acceptance,
//! never caller-selected metadata. Window8 remains unimplemented until readiness.
use crate::{
    LanguageParserV2ChoiceQuery, LanguageParserV2ModelFeatures, LanguageParserV2ModelScores,
    parser_model_selection::PreparedParserModelSelection,
    parser_session_execution::ParserSessionEntry,
};
use conduit_plot::rust_binding::PreparedNativeRustBinding;
mod sealed {
    pub trait Sealed {}
}
pub(crate) trait FixedParserNumericProfile: sealed::Sealed {
    type Query: PreparedNativeRustBinding;
    type Features: PreparedNativeRustBinding;
    type Scores: PreparedNativeRustBinding;
    const FEATURES: ParserSessionEntry;
    const INDICES: ParserSessionEntry;
    const SCORES: ParserSessionEntry;
    const SCORE_CLASSES: usize;
    const LOOKUPS: usize;
    fn admits_selection(selection: &PreparedParserModelSelection) -> bool;
}
pub(crate) struct PinnedFourSlotNumericProfile;
impl sealed::Sealed for PinnedFourSlotNumericProfile {}
impl FixedParserNumericProfile for PinnedFourSlotNumericProfile {
    type Query = LanguageParserV2ChoiceQuery;
    type Features = LanguageParserV2ModelFeatures;
    type Scores = LanguageParserV2ModelScores;
    const FEATURES: ParserSessionEntry = ParserSessionEntry::V2ModelFeatures;
    const INDICES: ParserSessionEntry = ParserSessionEntry::V2FeatureIndices;
    const SCORES: ParserSessionEntry = ParserSessionEntry::V2ScoreObservation;
    const SCORE_CLASSES: usize = 76;
    const LOOKUPS: usize = 25;
    fn admits_selection(selection: &PreparedParserModelSelection) -> bool {
        selection.declaration().is_none()
    }
}
