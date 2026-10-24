//! Shared exact Source envelope replay and beam admission for both epoch routes.
use super::*;
use crate::generated::LanguageAnalysisRebasedEnvelopeInput;
use conduit_plot::{rust_binding::NativeRustBinding, PortableExpressionProgram};

pub(super) fn rebase_envelopes(
    slots: [&LanguageParserScorerHypothesis; 4],
    raws: [&LanguageParserNumericState; 4],
) -> Result<(LanguageParserScorerBeam, Vec<Vec<u8>>), AnalysisEpochRefusal> {
    use AnalysisEpochRefusal::*;
    let program = PortableExpressionProgram::from_canonical_hex(include_str!(concat!(
        env!("OUT_DIR"),
        "/analysis_rebased_envelope.hex"
    )))
    .map_err(|_| Replay)?;
    let mut admitted = Vec::new();
    let mut envelopes = Vec::new();
    for (prior, raw) in slots.into_iter().zip(raws) {
        let input = LanguageAnalysisRebasedEnvelopeInput::new(prior.clone(), raw.clone())
            .map_err(Native)?;
        let output = program
            .evaluate(&input.encode().map_err(Native)?)
            .map_err(|_| Replay)?;
        if output.len() > 65536 {
            return Err(Capacity);
        }
        let raw = LanguageParserScorerRawHypothesis::decode(&output).map_err(Native)?;
        admitted.push(crate::analysis_epoch_initialization::admit(&raw).map_err(Initialization)?);
        envelopes.push(output);
    }
    let mut it = admitted.into_iter();
    let beam = LanguageParserScorerBeam::new(
        it.next().ok_or(Replay)?,
        it.next().ok_or(Replay)?,
        it.next().ok_or(Replay)?,
        it.next().ok_or(Replay)?,
    )
    .map_err(Native)?;
    Ok((beam, envelopes))
}
