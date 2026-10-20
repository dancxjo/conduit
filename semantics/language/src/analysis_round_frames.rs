//! Preparation-only canonical selection/composition from the admitted owner.
//! No reinterpretation of raw candidate bytes as refined Native values.
use super::*;
use conduit_core::{
    validate_canonical_structured_value, PreparedStructuredComposer, StructuredCanonicalSelection,
    StructuredInfoType, StructuredSelector,
};
use conduit_plot::rust_binding::NativeRustBinding;
fn field(input: &[u8], ty: &StructuredInfoType, name: &str) -> Result<Vec<u8>, PreludeRefusal> {
    let selector =
        StructuredSelector::field(ty.clone(), name).map_err(|_| PreludeRefusal::Preparation)?;
    let input_type = selector
        .input_type()
        .canonical_bytes()
        .map_err(|_| PreludeRefusal::Preparation)?;
    let output_type = selector
        .output_type()
        .canonical_bytes()
        .map_err(|_| PreludeRefusal::Preparation)?;
    let maximum = conduit_plot::maximum_prepared_canonical_value_bytes(selector.output_type())
        .map_err(|_| PreludeRefusal::Preparation)? as usize;
    let mut output = Vec::with_capacity(maximum);
    if selector
        .select_canonical_into(input, &input_type, &output_type, &mut output)
        .map_err(|_| PreludeRefusal::Preparation)?
        != StructuredCanonicalSelection::Matched
    {
        return Err(PreludeRefusal::Preparation);
    }
    Ok(output)
}
pub(super) fn prepare(owner: &AnalysisEpochOwner<'_>) -> Result<Vec<RequestFrame>, PreludeRefusal> {
    let raw = super::super::raw_beam(owner.beam()).map_err(|_| PreludeRefusal::Preparation)?;
    let raw_bytes = raw.encode().map_err(|_| PreludeRefusal::Preparation)?;
    let ty =
        LanguageAnalysisInitialRawBeam::semantic_type().map_err(|_| PreludeRefusal::Preparation)?;
    let children = ["candidate0", "candidate1", "candidate2", "candidate3"]
        .map(|name| field(&raw_bytes, &ty, name));
    let [a, b, c, d] = children;
    let (a, b, c, d) = (a?, b?, c?, d?);
    let mut composer = PreparedStructuredComposer::new(&ty, raw_bytes.len())
        .map_err(|_| PreludeRefusal::Preparation)?;
    let seed = composer
        .record(&[
            validate_canonical_structured_value(&a).map_err(|_| PreludeRefusal::Preparation)?,
            validate_canonical_structured_value(&b).map_err(|_| PreludeRefusal::Preparation)?,
            validate_canonical_structured_value(&c).map_err(|_| PreludeRefusal::Preparation)?,
            validate_canonical_structured_value(&d).map_err(|_| PreludeRefusal::Preparation)?,
        ])
        .map_err(|_| PreludeRefusal::Preparation)?;
    if seed != raw_bytes {
        return Err(PreludeRefusal::Preparation);
    }
    let mut frames = Vec::with_capacity(2);
    frames.push(RequestFrame {
        operation: PreludeOperation::EmptyRound,
        input: seed.to_vec(),
    });
    if let Some((index, prior)) = super::super::slots(owner.beam())
        .into_iter()
        .enumerate()
        .find(|(_, prior)| *prior.parser().active())
    {
        // The selected Type is the already admitted original ParserState.
        // Its canonical bytes remain distinct from the raw empty-round output.
        let beam = owner
            .beam()
            .clone()
            .encode()
            .map_err(|_| PreludeRefusal::Preparation)?;
        let beam_type =
            LanguageParserScorerBeam::semantic_type().map_err(|_| PreludeRefusal::Preparation)?;
        let selector =
            StructuredSelector::field(beam_type.clone(), alloc::format!("candidate{index}"))
                .map_err(|_| PreludeRefusal::Preparation)?;
        let candidate = field(&beam, &beam_type, &alloc::format!("candidate{index}"))?;
        let parser_selector = StructuredSelector::field(selector.output_type().clone(), "parser")
            .map_err(|_| PreludeRefusal::Preparation)?;
        let parser = field(&candidate, selector.output_type(), "parser")?;
        let state = field(&parser, parser_selector.output_type(), "state")?;
        if state
            != prior
                .parser()
                .state()
                .clone()
                .encode()
                .map_err(|_| PreludeRefusal::Preparation)?
        {
            return Err(PreludeRefusal::Preparation);
        }
        frames.push(RequestFrame {
            operation: PreludeOperation::DecodeComplete { survivor: index },
            input: state,
        });
    }
    Ok(frames)
}
