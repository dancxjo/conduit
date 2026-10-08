use crate::bank::*;
// Evaluation-only bridge from the actual opaque proposal owner. Ordinary Native
// input/representation allocations are outside the prepared Source bank receipt.
// This grants no lexical fact or dependency commitment.
use conduit_language::{
    lexical_proposer_port::token_producer::revision::AdmittedRevision,
    *,
};
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};
fn failure(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
fn evaluate<I: NativeRustBinding, O: NativeRustBinding>(input: I, hex: &str) -> Result<O, String> {
    let program = PortableExpressionProgram::from_canonical_hex(hex).map_err(failure)?;
    let input = input.encode().map_err(failure)?;
    let output = program.evaluate(&input).map_err(failure)?;
    O::decode(&output).map_err(failure)
}
pub struct ProposedLexical<'a> {
    pub original: &'a AdmittedRevision,
    pub proposed: LanguageLexicalProposedTape,
    pub lexical: LanguageParserWindow8Lexical,
    pub projection: LanguageParserWindow8RawProjection,
    pub origins: LanguageParserProposalWindow8Origins,
}
impl<'a> ProposedLexical<'a> {
    pub fn prepare(
        bank: &Window8ProgramBank,
        original: &'a AdmittedRevision,
    ) -> Result<Self, String> {
        let proposed = LanguageLexicalProposedTape::decode(original.canonical_proposed_tape())
            .map_err(failure)?;
        let available: LanguageParserWindow8Available = evaluate(
            proposed.tape().clone(),
            include_str!(concat!(env!("OUT_DIR"), "/window8_available.hex")),
        )?;
        let lexical =
            LanguageParserWindow8Lexical::new(proposed.tape().clone(), *available.count())
                .map_err(failure)?;
        let mut tokens = Vec::with_capacity(8);
        for ordinal in 0..8 {
            let codes: LanguageParserWindow8RawCodes = if ordinal < *lexical.token_count() {
                evaluate(
                    LanguageParserWindow8CodeQuery::new(
                        ordinal,
                        lexical.tape().tokens()[ordinal as usize].clone(),
                    )
                    .map_err(failure)?,
                    include_str!(concat!(env!("OUT_DIR"), "/window8_token_codes.hex")),
                )?
            } else {
                evaluate(
                    LanguageParserWindow8Ordinal::new(ordinal).map_err(failure)?,
                    include_str!(concat!(env!("OUT_DIR"), "/window8_empty_codes.hex")),
                )?
            };
            tokens.push(codes);
        }
        let projection = LanguageParserWindow8RawProjection::new(
            lexical.tape().profile().identity().clone(),
            *lexical.tape().source().sequence(),
            lexical.tape().source().material().revision().clone(),
            lexical.tape().source().material().identity().clone(),
            *lexical.token_count(),
            tokens.try_into().map_err(failure)?,
        )
        .map_err(failure)?;
        let origins = LanguageParserProposalWindow8Origins::new(
            bank.proposal_origins(
                LanguageParserProposalWindow8OriginQuery::new(lexical.clone(), proposed.clone())
                    .map_err(failure)?,
            )
            .map_err(failure)?,
        )
        .map_err(failure)?;
        Ok(Self {
            original,
            proposed,
            lexical,
            projection,
            origins,
        })
    }
    pub fn features(
        &self,
        bank: &Window8ProgramBank,
        state: &Window8BankState,
        basis: &LanguageParserBasis,
        choices: [u64; 8],
    ) -> Result<
        (
            LanguageParserProposalWindow8FeatureQuery,
            LanguageParserProposalWindow8V2Features,
        ),
        String,
    > {
        let raw = LanguageParserWindow8RawFeatureQuery::new(
            choices,
            basis.clone(),
            self.projection.clone(),
            state.state().clone(),
        )
        .map_err(failure)?;
        let query = LanguageParserProposalWindow8FeatureQuery::new(self.origins.clone(), raw)
            .map_err(failure)?;
        let features = LanguageParserProposalWindow8V2Features::new(
            bank.proposal_v2_features(query.clone()).map_err(failure)?,
        )
        .map_err(failure)?;
        Ok((query, features))
    }
}
