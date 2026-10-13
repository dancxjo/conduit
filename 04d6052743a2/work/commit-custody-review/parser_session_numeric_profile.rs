//! Closed numerical custody profile seam. A profile binds full generated Native
//! Types and fixed Source entries; implementing it is library-owned acceptance,
//! never caller-selected metadata. Window8 remains unimplemented until readiness.
use crate::{
    parser_model_selection::PreparedParserModelSelection,
    parser_session_execution::ParserSessionEntry, LanguageParserV2ChoiceQuery,
    LanguageParserV2ModelFeatures, LanguageParserV2ModelScores,
};
use conduit_plot::rust_binding::PreparedNativeRustBinding;
pub(crate) mod sealed {
    pub trait Sealed {}
}
pub(crate) trait FixedParserNumericProfile: sealed::Sealed {
    type Selection;
    type Query: PreparedNativeRustBinding;
    type Features: PreparedNativeRustBinding;
    type Scores: PreparedNativeRustBinding;
    const FEATURES: ParserSessionEntry;
    const INDICES: ParserSessionEntry;
    const SCORES: ParserSessionEntry;
    const SCORE_CLASSES: usize;
    const LOOKUPS: usize;
    fn feature_guard_descriptor(
    ) -> Option<&'static conduit_plot::rust_binding::NativeFamilyTypeDescriptor> {
        None
    }
    fn admit_feature_guard(
        _family: &mut conduit_plot::rust_binding::PreparedNativeFamily,
        _raw: &[u8],
        guard: &[u8],
    ) -> Result<(), conduit_plot::rust_binding::NativeBindingRefusal> {
        if guard.is_empty() {
            Ok(())
        } else {
            Err(
                conduit_plot::rust_binding::NativeBindingRefusal::InvalidValue(
                    conduit_core::StructuredInfoRefusal::WrongType,
                ),
            )
        }
    }
    fn admits_selection(selection: &Self::Selection) -> bool;
    fn selected_model_owner(selection: &Self::Selection) -> &alloc::sync::Arc<conduit_ai::integer_categorical_step::PreparedCategoricalStep>;
    fn selected_model(
        selection: &Self::Selection,
    ) -> &conduit_ai::integer_categorical_step::PreparedCategoricalStep;
}
pub(crate) struct PinnedFourSlotNumericProfile;
impl sealed::Sealed for PinnedFourSlotNumericProfile {}
impl FixedParserNumericProfile for PinnedFourSlotNumericProfile {
    type Selection = PreparedParserModelSelection;
    type Query = LanguageParserV2ChoiceQuery;
    type Features = LanguageParserV2ModelFeatures;
    type Scores = LanguageParserV2ModelScores;
    const FEATURES: ParserSessionEntry = ParserSessionEntry::V2ModelFeatures;
    const INDICES: ParserSessionEntry = ParserSessionEntry::V2FeatureIndices;
    const SCORES: ParserSessionEntry = ParserSessionEntry::V2ScoreObservation;
    const SCORE_CLASSES: usize = 76;
    const LOOKUPS: usize = 25;
    fn admits_selection(selection: &Self::Selection) -> bool {
        selection.declaration().is_none()
    }
    fn selected_model_owner(selection: &Self::Selection) -> &alloc::sync::Arc<conduit_ai::integer_categorical_step::PreparedCategoricalStep> {
        selection.prepared_categorical()
    }
    fn selected_model(
        selection: &Self::Selection,
    ) -> &conduit_ai::integer_categorical_step::PreparedCategoricalStep {
        selection.prepared_categorical().as_ref()
    }
}
