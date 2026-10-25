//! Guaranteed minimum value storage, distinct from permitted maximum size.
use crate::planning_input::ConnectionGear;
use crate::PlannerError;
use conduit_core::{CheckedFront, ConnectionTrack, FrontValueLocation, ValueConstraint};
use conduit_plot::CheckedConnection;

fn minimum_at(front: &CheckedFront, location: FrontValueLocation) -> u32 {
    front
        .value_contracts()
        .iter()
        .filter(|bound| bound.location == location)
        .flat_map(|bound| &bound.contract.constraints)
        .filter_map(|constraint| match constraint {
            ValueConstraint::ByteLength { minimum, .. } => Some(*minimum),
            _ => None,
        })
        .max()
        .unwrap_or_default()
}

fn required_minimum(
    source: &CheckedFront,
    source_port: &conduit_core::PortId,
    sink: &CheckedFront,
    sink_port: &conduit_core::PortId,
    track: ConnectionTrack,
) -> u32 {
    let source_location = match track {
        ConnectionTrack::Payload => FrontValueLocation::Output(source_port.clone()),
        ConnectionTrack::AbnormalTerminal => {
            FrontValueLocation::OutputAbnormal(source_port.clone())
        }
        ConnectionTrack::NormalClose | ConnectionTrack::Quiescence => return 0,
    };
    minimum_at(source, source_location).max(minimum_at(
        sink,
        FrontValueLocation::Input(sink_port.clone()),
    ))
}

pub(crate) fn validate_capacity(
    connection: &CheckedConnection,
    source: &ConnectionGear<'_>,
    sink: &ConnectionGear<'_>,
    byte_capacity: u32,
) -> Result<(), PlannerError> {
    let required = required_minimum(
        &source.front(),
        &connection.source_port_id,
        &sink.front(),
        &connection.sink_port_id,
        connection.track,
    );
    if byte_capacity < required {
        return Err(PlannerError::InvalidConnectionBudget(alloc::format!(
            "value/storage-bound@1: '{}:{}' to '{}:{}' requires at least {} bytes before Play, admitted {}",
            connection.source_gear_id.as_str(), connection.source_port_id.as_str(),
            connection.sink_gear_id.as_str(), connection.sink_port_id.as_str(),
            required, byte_capacity,
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, port_id, CheckedValueContract, FrontValueContract};

    fn contract(
        location: FrontValueLocation,
        minimum: Option<u32>,
        maximum: u32,
    ) -> FrontValueContract {
        FrontValueContract {
            location,
            contract: CheckedValueContract::new(
                kind_id("value/bytes"),
                maximum,
                minimum
                    .map(|minimum| ValueConstraint::ByteLength { minimum, maximum })
                    .into_iter()
                    .collect(),
            )
            .unwrap(),
        }
    }

    #[test]
    fn variable_upper_bound_does_not_require_a_full_size_cord() {
        let source =
            CheckedFront::new(vec![], vec![], vec![], None).with_value_contracts(vec![contract(
                FrontValueLocation::Output(port_id("out")),
                None,
                256,
            )]);
        let sink = CheckedFront::new(vec![], vec![], vec![], None);
        assert_eq!(
            required_minimum(
                &source,
                &port_id("out"),
                &sink,
                &port_id("in"),
                ConnectionTrack::Payload
            ),
            0
        );
    }

    #[test]
    fn terminal_tracks_use_their_own_minimum_and_nonvalue_tracks_need_none() {
        let source = CheckedFront::new(vec![], vec![], vec![], None).with_value_contracts(vec![
            contract(FrontValueLocation::Output(port_id("out")), Some(8192), 8192),
            contract(
                FrontValueLocation::OutputAbnormal(port_id("out")),
                Some(73),
                73,
            ),
        ]);
        let sink = CheckedFront::new(vec![], vec![], vec![], None);
        for (track, expected) in [
            (ConnectionTrack::Payload, 8192),
            (ConnectionTrack::AbnormalTerminal, 73),
            (ConnectionTrack::NormalClose, 0),
            (ConnectionTrack::Quiescence, 0),
        ] {
            assert_eq!(
                required_minimum(&source, &port_id("out"), &sink, &port_id("in"), track),
                expected
            );
        }
    }
}
