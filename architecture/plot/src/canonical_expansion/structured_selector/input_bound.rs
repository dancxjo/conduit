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
    let mut laws = crate::expression_program::input_bound::semantic_laws_for_identity(
        checked
            .input_type
            .value_kind()
            .expect("checked expression has semantic input")
            .clone(),
        checked.input_maximum_bytes,
        checked.input_type == checked.value_type
            && matches!(checked.syntax, crate::ExpressionSyntax::Input(_)),
    )
    .map_err(diagnostic)?;
    crate::expression_program::value_contract::retain_laws(
        &mut laws,
        checked.input_value_contract.as_ref(),
        checked.output_value_contract.as_ref(),
    );
    Ok(laws)
}

fn diagnostic(error: impl core::fmt::Debug) -> CanonicalExpansionDiagnostic {
    CanonicalExpansionDiagnostic::new(
        "CND-FRM-046",
        format!("pure expression input bound: {error:?}"),
    )
}

pub(super) struct ExpressionBoundaryContracts {
    pub(super) input: Option<conduit_core::CheckedValueContract>,
    pub(super) output: Option<conduit_core::CheckedValueContract>,
}

/// Resolve the carrier contracts before checking the expression body. Physical
/// expression typing must not reconstruct or weaken the surrounding evidence
/// contracts from the carrier Kind alone.
pub(super) fn boundary_contracts(
    source: &StageSource,
    right: Option<&Stage>,
    plot: &CheckedCanonicalPlot,
    gears: &[CheckedGear],
) -> ExpressionBoundaryContracts {
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
    let input = match source {
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
    let output = right
        .and_then(|s| s.input.as_ref())
        .and_then(|s| s.first())
        .and_then(|sink| match sink {
            StageSink::FaceOutput(name, ..) => retained(
                plot.runtime_front
                    .value_contract(&FrontValueLocation::Output(conduit_core::port_id(name))),
            ),
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
    ExpressionBoundaryContracts { input, output }
}

impl ExpressionBoundaryContracts {
    pub(super) fn retain(
        self,
        checked: &mut crate::CheckedExpression,
    ) -> Result<(), CanonicalExpansionDiagnostic> {
        checked.input_value_contract = self.input;
        checked.output_value_contract = self.output;
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
}
