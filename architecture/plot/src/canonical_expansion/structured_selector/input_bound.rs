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
    crate::PortableExpressionProgram::from_checked(checked)
        .map_err(diagnostic)?
        .semantic_laws()
        .map_err(diagnostic)
}

fn diagnostic(error: impl core::fmt::Debug) -> CanonicalExpansionDiagnostic {
    CanonicalExpansionDiagnostic::new(
        "CND-FRM-046",
        format!("pure expression input bound: {error:?}"),
    )
}

pub(super) fn retain_contracts(
    checked: &mut crate::CheckedExpression,
    source: &StageSource,
    right: Option<&Stage>,
    plot: &CheckedCanonicalPlot,
    gears: &[CheckedGear],
) -> Result<(), CanonicalExpansionDiagnostic> {
    fn retained(
        contract: Option<&conduit_core::CheckedValueContract>,
    ) -> Option<conduit_core::CheckedValueContract> {
        contract
            .filter(|c| {
                c.constraints.iter().any(|constraint| {
                    matches!(
                        constraint,
                        conduit_core::ValueConstraint::NearQuantity { .. }
                    )
                })
            })
            .cloned()
    }
    checked.input_value_contract = match source {
        StageSource::FaceInput(name, ..) => retained(
            plot.runtime_front
                .value_contract(&FrontValueLocation::Input(conduit_core::port_id(name))),
        ),
        StageSource::Internal(endpoint) => gears
            .iter()
            .find(|g| g.gear_id == endpoint.gear_id)
            .and_then(|g| {
                g.semantic_contract.value_contracts().iter().find(|c| {
                    c.location == FrontValueLocation::Output(endpoint.port.port_id.clone())
                })
            })
            .and_then(|c| retained(Some(&c.contract))),
    };
    checked.output_value_contract = right
        .and_then(|s| s.input.as_ref())
        .and_then(|s| s.first())
        .and_then(|sink| match sink {
            // An authored output ceiling is a runtime obligation, independently
            // of the evaluator's conservative construction scratch bound.
            StageSink::FaceOutput(name, ..) => plot
                .runtime_front
                .value_contract(&FrontValueLocation::Output(conduit_core::port_id(name)))
                .cloned(),
            StageSink::Internal(endpoint) => gears
                .iter()
                .find(|g| g.gear_id == endpoint.gear_id)
                .and_then(|g| {
                    g.semantic_contract.value_contracts().iter().find(|c| {
                        c.location == FrontValueLocation::Input(endpoint.port.port_id.clone())
                    })
                })
                .and_then(|c| retained(Some(&c.contract))),
        });
    if matches!(checked.syntax, crate::ExpressionSyntax::Input(_))
        && super::super::qualified_front::incompatible_physical_sources(
            checked.input_value_contract.as_ref(),
            checked.output_value_contract.as_ref(),
        )
    {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-046",
            "identity expression cannot change a qualified physical source Kind".into(),
        ));
    }
    Ok(())
}
