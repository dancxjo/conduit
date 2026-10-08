//! Distinct closed Window8 numerical ABI; legacy FourSlot remains unchanged.
use crate::parser_session_execution::ParserSessionEntry;
use crate::parser_session_numeric_profile::{sealed, FixedParserNumericProfile};
use conduit_plot::rust_binding::PreparedNativeRustBinding;
/// Distinct selected411/27/76 proposal profile. The enclosing closed factory
/// additionally readmits the full expected signature and retains exact model,
/// Source/Plan and opaque revision owners before any public Session is created.
pub(crate) struct ProposalWindow8V2NumericProfile;
impl sealed::Sealed for ProposalWindow8V2NumericProfile {}
impl FixedParserNumericProfile for ProposalWindow8V2NumericProfile {
    type Selection = crate::parser_session_window8_model::VerifiedWindow8Model;
    type Query = crate::generated::LanguageParserProposalWindow8FeatureContext;
    type Features = crate::generated::LanguageParserProposalWindow8V2RawFeatures;
    type Scores = crate::generated::LanguageParserProposalWindow8V2ModelScores;
    const FEATURES: ParserSessionEntry = ParserSessionEntry::ProposalWindow8V2Features;
    const INDICES: ParserSessionEntry = ParserSessionEntry::ProposalWindow8V2Indices;
    const SCORES: ParserSessionEntry = ParserSessionEntry::ProposalWindow8V2Scores;
    const SCORE_CLASSES: usize = 76;
    const LOOKUPS: usize = 27;
    fn feature_guard_descriptor(
    ) -> Option<&'static conduit_plot::rust_binding::NativeFamilyTypeDescriptor> {
        Some(<crate::generated::LanguageParserProposalWindow8V2Features as PreparedNativeRustBinding>::PREPARED_DESCRIPTOR)
    }
    fn admit_feature_guard(
        family: &mut conduit_plot::rust_binding::PreparedNativeFamily,
        raw: &[u8],
        guard: &[u8],
    ) -> Result<(), conduit_plot::rust_binding::NativeBindingRefusal> {
        use conduit_plot::rust_binding::NativeBindingRefusal as R;
        let wrong = || R::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType);
        drop(family.decode::<crate::generated::LanguageParserProposalWindow8V2Features>(guard)?);
        let parent =
            conduit_core::validate_canonical_structured_value(raw).map_err(R::InvalidValue)?;
        let wrapper =
            conduit_core::validate_canonical_structured_value(guard).map_err(R::InvalidValue)?;
        let retained = wrapper
            .record_field("raw")
            .map_err(R::InvalidValue)?
            .ok_or_else(wrong)?;
        if retained.type_bytes() != parent.type_bytes()
            || retained.value_node() != parent.value_node()
        {
            return Err(wrong());
        }
        Ok(())
    }
    fn admits_selection(_selection: &Self::Selection) -> bool {
        // This opaque owner is constructed only after complete closed-profile,
        // resource and fresh expected-signature admission. Generic declaration
        // metadata can never enter this seam directly.
        true
    }
    fn selected_model_owner(selection: &Self::Selection) -> &alloc::sync::Arc<conduit_ai::integer_categorical_step::PreparedCategoricalStep> {
        selection.selection().categorical_owner()
    }
    fn selected_model(
        selection: &Self::Selection,
    ) -> &conduit_ai::integer_categorical_step::PreparedCategoricalStep {
        selection.categorical()
    }
}
