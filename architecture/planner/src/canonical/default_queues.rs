//! Default queue budgets are preferences bounded by both selected offers.
//! Explicit caller requirements still go through the strict admission path.

use super::*;
use conduit_core::{DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY};

fn required_value_bytes(
    source: &conduit_core::CheckedFront,
    source_port: &conduit_core::PortId,
    sink: &conduit_core::CheckedFront,
    sink_port: &conduit_core::PortId,
    track: conduit_core::ConnectionTrack,
) -> Result<u32, PlannerError> {
    let bound_at = |front: &conduit_core::CheckedFront,
                    location: conduit_core::FrontValueLocation| {
        front
            .value_contracts()
            .iter()
            .find(|bound| bound.location == location)
            .map(|contract| u64::from(contract.contract.maximum_bytes))
            .unwrap_or_default()
    };
    let source_location = match track {
        conduit_core::ConnectionTrack::AbnormalTerminal => {
            conduit_core::FrontValueLocation::OutputAbnormal(source_port.clone())
        }
        _ => conduit_core::FrontValueLocation::Output(source_port.clone()),
    };
    let required = bound_at(source, source_location).max(bound_at(
        sink,
        conduit_core::FrontValueLocation::Input(sink_port.clone()),
    ));
    u32::try_from(required)
        .map_err(|_| PlannerError::InvalidConnectionBudget("value bound overflow".into()))
}

// An opaque Value has no checked byte envelope from which to size a single
// retained value. Prefer the finite capacity supported by both selected Backs;
// this is storage planning, not a new semantic domain or resource grant.
fn preferred_bytes(
    temporal: conduit_core::PortTemporal,
    required: u32,
    source: u32,
    sink: u32,
) -> Result<u32, PlannerError> {
    if source == 0 || sink == 0 || required > source || required > sink {
        return Err(PlannerError::QueueRequirementAboveHostLimit(
            "checked value envelope exceeds selected Back queue capacity".into(),
        ));
    }
    let preference = if temporal == conduit_core::PortTemporal::Value && required == 0 {
        source.min(sink)
    } else {
        DEFAULT_CONNECTION_BYTE_CAPACITY
    };
    Ok(preference.max(required).min(source).min(sink))
}

/// Queue preferences bounded by both selected Backs and checked value contracts.
/// Explicit required capacities must still use the strict planning entrances.
pub fn default_expanded_connection_limits(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
) -> Result<BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>, PlannerError> {
    plot.validate_expansion()
        .map_err(|error| PlannerError::InvalidPlotIdentity(error.to_string()))?;
    let mut limits = BTreeMap::new();
    for connection in &plot.connections {
        let capability = |gear: &conduit_core::GearId| {
            let choice = placements
                .by_gear
                .get(gear)
                .ok_or_else(|| PlannerError::MissingPlacement(gear.as_str().to_string()))?;
            crate::find_capability(hosts, &choice.host_id, &choice.capability_id)
        };
        let source = capability(&connection.source_gear_id)?;
        let sink = capability(&connection.sink_gear_id)?;
        let source_gear = plot
            .gears
            .iter()
            .find(|gear| gear.gear_id == connection.source_gear_id)
            .ok_or_else(|| {
                PlannerError::MissingPlacement(connection.source_gear_id.as_str().to_string())
            })?;
        let sink_gear = plot
            .gears
            .iter()
            .find(|gear| gear.gear_id == connection.sink_gear_id)
            .ok_or_else(|| {
                PlannerError::MissingPlacement(connection.sink_gear_id.as_str().to_string())
            })?;
        let required_value_bytes = required_value_bytes(
            &source_gear.checked_front(),
            &connection.source_port_id,
            &sink_gear.checked_front(),
            &connection.sink_port_id,
            connection.track,
        )?;
        if [source, sink]
            .iter()
            .any(|offer| offer.limits.max_queue_items == 0 || offer.limits.max_queue_bytes == 0)
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires nonzero queue capacity",
                connection.source_gear_id.as_str(),
                connection.sink_gear_id.as_str(),
            )));
        }
        limits.insert(
            (
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            ),
            ConnectionQueueLimits {
                item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY
                    .min(source.limits.max_queue_items)
                    .min(sink.limits.max_queue_items),
                byte_capacity: preferred_bytes(
                    connection.temporal,
                    required_value_bytes,
                    source.limits.max_queue_bytes,
                    sink.limits.max_queue_bytes,
                )?,
            },
        );
    }
    Ok(limits)
}

pub fn plan_expanded_canonical(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
) -> Result<Plan, PlannerError> {
    let limits = default_expanded_connection_limits(plot, hosts, placements)?;
    plan_expanded_canonical_with_connection_limits(
        plot,
        hosts,
        placements,
        bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        kind_id, port_id, CheckedFront, CheckedValueContract, FrontValueContract,
        FrontValueLocation, PortDescriptor, PortDirection, PortTemporal,
    };

    #[test]
    fn abnormal_cord_budget_uses_terminal_contract_not_payload_contract() {
        let source_port = PortDescriptor {
            port_id: port_id("work"),
            value_kind: kind_id("value/bytes"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Flow { closes: true },
            abnormal_kind: Some(kind_id("value/text")),
        };
        let source = CheckedFront::new(Vec::new(), Vec::new(), vec![source_port], None)
            .with_value_contracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("work")),
                    contract: CheckedValueContract::new(kind_id("value/bytes"), 8_192, vec![])
                        .unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::OutputAbnormal(port_id("work")),
                    contract: CheckedValueContract::new(kind_id("value/text"), 73, vec![]).unwrap(),
                },
            ]);
        let sink_port = PortDescriptor {
            port_id: port_id("explain"),
            value_kind: kind_id("value/text"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        };
        let sink =
            CheckedFront::new(vec![], vec![sink_port], vec![], None).with_value_contracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("explain")),
                    contract: CheckedValueContract::new(kind_id("value/text"), 73, vec![]).unwrap(),
                },
            ]);

        assert_eq!(
            required_value_bytes(
                &source,
                &port_id("work"),
                &sink,
                &port_id("explain"),
                conduit_core::ConnectionTrack::AbnormalTerminal,
            ),
            Ok(73)
        );
        assert_eq!(
            required_value_bytes(
                &source,
                &port_id("work"),
                &sink,
                &port_id("explain"),
                conduit_core::ConnectionTrack::Payload,
            ),
            Ok(8_192)
        );
    }
}

#[cfg(test)]
mod opaque_value_tests {
    use super::*;
    use conduit_core::PortTemporal;

    #[test]
    fn opaque_value_capacity_uses_finite_selected_back_support() {
        assert_eq!(
            preferred_bytes(PortTemporal::Value, 0, 6144, 8192).unwrap(),
            6144
        );
        assert_eq!(
            preferred_bytes(PortTemporal::Value, 0, 8192, 4096).unwrap(),
            4096
        );
        assert!(preferred_bytes(PortTemporal::Value, 0, 0, 8192).is_err());
        assert!(preferred_bytes(PortTemporal::Value, 8193, 6144, 8192).is_err());
        // Authored envelopes retain their existing capacity preference rather
        // than inheriting a wider implementation storage ceiling.
        assert_eq!(
            preferred_bytes(PortTemporal::Value, 256, 6144, 8192).unwrap(),
            256
        );
        assert_eq!(
            preferred_bytes(PortTemporal::Flow { closes: true }, 0, 6144, 8192).unwrap(),
            DEFAULT_CONNECTION_BYTE_CAPACITY
        );
    }
}
