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

pub fn plan_expanded_canonical(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
) -> Result<Plan, PlannerError> {
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
                byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY
                    .max(required_value_bytes)
                    .min(source.limits.max_queue_bytes)
                    .min(sink.limits.max_queue_bytes),
            },
        );
    }
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
