//! Carry the actual finite input Fore ceiling into the canonical program.
use super::*;
use conduit_core::{ConnectionTrack, FrontValueLocation, KindSemanticLaw};

pub(super) fn checked_bound(
    source: &StageSource,
    plot: &CheckedCanonicalPlot,
    gears: &[CheckedGear],
) -> Result<Option<u32>, CanonicalExpansionDiagnostic> {
    let maximum = match source {
        StageSource::FaceInput(name, _, _, _, track) => {
            let port = conduit_core::port_id(name);
            let location = if *track == ConnectionTrack::AbnormalTerminal {
                FrontValueLocation::InputAbnormal(port)
            } else {
                FrontValueLocation::Input(port)
            };
            plot.runtime_front
                .value_contract(&location)
                .map(|contract| contract.maximum_bytes)
        }
        StageSource::Internal(endpoint) => gears
            .iter()
            .find(|gear| gear.gear_id == endpoint.gear_id)
            .and_then(|gear| {
                let location = if endpoint.track == ConnectionTrack::AbnormalTerminal {
                    FrontValueLocation::OutputAbnormal(endpoint.port.port_id.clone())
                } else {
                    FrontValueLocation::Output(endpoint.port.port_id.clone())
                };
                conduit_core::CheckedFront::new(
                    Vec::new(),
                    gear.inputs.clone(),
                    gear.outputs.clone(),
                    None,
                )
                .with_value_contracts(gear.semantic_contract.value_contracts().to_vec())
                .value_contract(&location)
                .map(|contract| contract.maximum_bytes)
            }),
    };
    if maximum
        .is_some_and(|maximum| maximum as usize > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES)
    {
        return Err(diagnostic("input Fore exceeds canonical transport ceiling"));
    }
    Ok(maximum)
}

pub(super) fn semantic_laws(
    checked: &crate::CheckedExpression,
) -> Result<Vec<KindSemanticLaw>, CanonicalExpansionDiagnostic> {
    crate::expression_program::input_bound::semantic_laws_for_identity(
        checked
            .input_type
            .value_kind()
            .expect("checked expression has semantic input")
            .clone(),
        checked.input_maximum_bytes,
        checked.input_type == checked.value_type
            && matches!(checked.syntax, crate::ExpressionSyntax::Input(_)),
    )
    .map_err(diagnostic)
}

fn diagnostic(error: impl core::fmt::Debug) -> CanonicalExpansionDiagnostic {
    CanonicalExpansionDiagnostic::new(
        "CND-FRM-046",
        format!("pure expression input bound: {error:?}"),
    )
}
