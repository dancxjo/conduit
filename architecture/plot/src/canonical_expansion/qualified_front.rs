//! Qualified Source Fores may not lose their predicate when inlined.
use super::*;
use conduit_core::{FrontValueLocation as Location, ValueConstraint};

/// The shared evidence carrier does not erase two known physical source Types.
/// Generic Quantity keeps its existing runtime physical refinement gate.
pub(super) fn incompatible_physical_sources(
    input: Option<&conduit_core::CheckedValueContract>,
    output: Option<&conduit_core::CheckedValueContract>,
) -> bool {
    fn source(contract: &conduit_core::CheckedValueContract) -> Option<&KindId> {
        contract.constraints.iter().find_map(|constraint| {
            if let ValueConstraint::NearQuantity { source_kind, .. } = constraint {
                Some(source_kind)
            } else {
                None
            }
        })
    }
    match (input.and_then(source), output.and_then(source)) {
        (Some(input), Some(output)) => {
            input != output
                && input.as_str() != conduit_core::QUANTITY_INFO_ID
                && output.as_str() != conduit_core::QUANTITY_INFO_ID
        }
        _ => false,
    }
}

pub(super) fn validate_connections(
    gears: &[CheckedGear],
    connections: &[CheckedConnection],
) -> Result<(), CanonicalExpansionDiagnostic> {
    let contract = |id: &GearId, location: Location| {
        gears
            .iter()
            .find(|gear| &gear.gear_id == id)
            .and_then(|gear| {
                gear.semantic_contract
                    .value_contracts()
                    .iter()
                    .find(|entry| entry.location == location)
            })
            .map(|entry| &entry.contract)
    };
    for connection in connections {
        if connection.track == conduit_core::ConnectionTrack::Payload
            && incompatible_physical_sources(
                contract(
                    &connection.source_gear_id,
                    Location::Output(connection.source_port_id.clone()),
                ),
                contract(
                    &connection.sink_gear_id,
                    Location::Input(connection.sink_port_id.clone()),
                ),
            )
        {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-045",
                "qualified cord connects incompatible physical source Kinds".into(),
            ));
        }
    }
    Ok(())
}

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
