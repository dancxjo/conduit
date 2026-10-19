//! Qualified Source Fores may not lose their predicate when inlined.
use super::*;
use conduit_core::{FrontValueLocation as Location, ValueConstraint};
pub(super) fn validate(
    plot: &CheckedCanonicalPlot,
    inputs: &BTreeMap<String, Vec<TrackedEndpoint>>,
    outputs: &BTreeMap<String, TrackedEndpoint>,
    gears: &[CheckedGear],
) -> Result<(), CanonicalExpansionDiagnostic> {
    for declared in plot.runtime_front.value_contracts() {
        if !declared
            .contract
            .constraints
            .iter()
            .any(|c| matches!(c, ValueConstraint::NearQuantity { .. }))
        {
            continue;
        }
        let (endpoints, input) = match &declared.location {
            Location::Input(port) => (
                inputs.get(port.as_str()).map(Vec::as_slice).unwrap_or(&[]),
                true,
            ),
            Location::Output(port) => (
                outputs
                    .get(port.as_str())
                    .map(core::slice::from_ref)
                    .unwrap_or(&[]),
                false,
            ),
            _ => continue,
        };
        if endpoints.is_empty()
            || endpoints.iter().any(|endpoint| {
                let location = if input {
                    Location::Input(endpoint.port.port_id.clone())
                } else {
                    Location::Output(endpoint.port.port_id.clone())
                };
                gears
                    .iter()
                    .find(|gear| gear.gear_id == endpoint.gear_id)
                    .and_then(|gear| {
                        gear.semantic_contract
                            .value_contracts()
                            .iter()
                            .find(|c| c.location == location)
                    })
                    .is_none_or(|contract| contract.contract != declared.contract)
            })
        {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "qualified Fore must retain its checked predicate at every executable boundary"
                    .into(),
            ));
        }
    }
    Ok(())
}
