// Distinct proposer feature ABI. These are arithmetic representations, not
// lexical truth, dependency admission, model selection or commitment authority.
fn proposal_source_entries() -> [(&'static str, &'static str); 29] {
    let original = source_entries();
    let additional = [
        (
            "proposal_window8_origins",
            include_str!(concat!(env!("OUT_DIR"), "/proposal_window8_origins.hex")),
        ),
        (
            "proposal_window8_feature_context",
            include_str!(concat!(
                env!("OUT_DIR"),
                "/proposal_window8_feature_context.hex"
            )),
        ),
        (
            "proposal_window8_feature_values",
            include_str!(concat!(
                env!("OUT_DIR"),
                "/proposal_window8_feature_values.hex"
            )),
        ),
    ];
    core::array::from_fn(|index| {
        if index < original.len() {
            original[index]
        } else {
            additional[index - original.len()]
        }
    })
}
fn prepare_proposal_native_family(
    limits: PreparedNativeFamilyLimits,
) -> Result<PreparedNativeFamily, PreparedNativeFamilyRefusal> {
    prepare_proposal_family(
        limits,
        LanguageParserProposalWindow8Features::PREPARED_DESCRIPTOR,
    )
}
fn prepare_proposal_v2_native_family(
    limits: PreparedNativeFamilyLimits,
) -> Result<PreparedNativeFamily, PreparedNativeFamilyRefusal> {
    prepare_proposal_family(
        limits,
        LanguageParserProposalWindow8V2Features::PREPARED_DESCRIPTOR,
    )
}
fn prepare_proposal_family(
    limits: PreparedNativeFamilyLimits,
    features: &'static conduit_plot::rust_binding::NativeFamilyTypeDescriptor,
) -> Result<PreparedNativeFamily, PreparedNativeFamilyRefusal> {
    PreparedNativeFamily::prepare(
        &[
            LanguageParserWindow8StableLexicalFact::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawState::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawWalk::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RootCount::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawClassIndex::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawClassRelations::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawClass::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawBeam::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawContext::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawResult::PREPARED_DESCRIPTOR,
            LanguageParserWindow8Completion::PREPARED_DESCRIPTOR,
            LanguageParserWindow8Selected::PREPARED_DESCRIPTOR,
            // RawHypothesis is already an exact child of RawBeam.
            LanguageParserWindow8RawFeatureContext::PREPARED_DESCRIPTOR,
            LanguageParserWindow8RawModelFeatures::PREPARED_DESCRIPTOR,
            LanguageParserProposalWindow8FeatureContext::PREPARED_DESCRIPTOR,
            features,
        ],
        limits,
    )
}
impl Window8ProgramBank {
    /// Prepares the separate 27-feature Source ABI and its complete Native output
    /// closure. Does not select a model or admit an unknown-origin lexical fact.
    /// Ordinary Native input construction/encoding remains outside the receipt.
    pub fn prepare_proposal_native_evaluator(
        native_limits: PreparedNativeFamilyLimits,
        limits: Window8SourcePreparationLimits,
    ) -> Result<Self, Window8PreparedBankRefusal> {
        Self::prepare_evaluator_bank(
            native_limits,
            limits,
            prepare_proposal_native_family,
            proposal_source_entries(),
        )
    }
    /// Projects explicit origins from the complete correlated proposal/tape query.
    /// The caller retains the original opaque revision and Source witness.
    pub fn proposal_origins(
        &self,
        query: LanguageParserProposalWindow8OriginQuery,
    ) -> Result<LanguageParserProposalWindow8RawOrigins, Window8Refusal> {
        self.run("proposal_window8_origins", query)
    }
    /// Exact Source arithmetic only. The complete guarded Features wrapper and
    /// independently pinned model selection remain separate admission steps.
    pub fn proposal_features(
        &self,
        query: LanguageParserProposalWindow8FeatureQuery,
    ) -> Result<LanguageParserProposalWindow8RawFeatures, Window8Refusal> {
        let context: LanguageParserProposalWindow8FeatureContext =
            self.run("proposal_window8_feature_context", query)?;
        self.run("proposal_window8_feature_values", context)
    }
    /// Bounded 411-category successor; preserves exact unigrams/history/origins.
    /// The old 446-category Source and preparation APIs remain distinct.
    pub fn prepare_proposal_v2_native_evaluator(
        native_limits: PreparedNativeFamilyLimits,
        limits: Window8SourcePreparationLimits,
    ) -> Result<Self, Window8PreparedBankRefusal> {
        let mut entries = proposal_source_entries();
        entries[28] = (
            "proposal_window8_v2_feature_values",
            include_str!(concat!(
                env!("OUT_DIR"),
                "/proposal_window8_v2_feature_values.hex"
            )),
        );
        Self::prepare_evaluator_bank(
            native_limits,
            limits,
            prepare_proposal_v2_native_family,
            entries,
        )
    }
    /// Source arithmetic representation only; full guarded feature/model admission
    /// and original opaque revision custody remain separate caller duties.
    pub fn proposal_v2_features(
        &self,
        query: LanguageParserProposalWindow8FeatureQuery,
    ) -> Result<LanguageParserProposalWindow8V2RawFeatures, Window8Refusal> {
        let context: LanguageParserProposalWindow8FeatureContext =
            self.run("proposal_window8_feature_context", query)?;
        self.run("proposal_window8_v2_feature_values", context)
    }
}
