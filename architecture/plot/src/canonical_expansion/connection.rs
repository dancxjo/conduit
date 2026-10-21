//! Materialize checked connections and callable boundary bindings.
use super::*;

pub(super) fn connect(
    source: StageSource,
    sink: StageSink,
    origin: crate::ExpandedConnectionOrigin,
    connections: &mut ConnectionInventory,
    inputs: &mut BTreeMap<String, Vec<TrackedEndpoint>>,
    outputs: &mut BTreeMap<String, TrackedEndpoint>,
) -> Result<(), CanonicalExpansionDiagnostic> {
    match (source, sink) {
        (StageSource::Internal(source), StageSink::Internal(sink)) => {
            let connection_track = connection_track(source.track, sink.track)?;
            validate_connection_contract(&source.port, &sink.port, connection_track)?;
            let Endpoint {
                gear_id: source_gear_id,
                port: source_port,
            } = source.endpoint;
            let Endpoint {
                gear_id: sink_gear_id,
                port: sink_port,
            } = sink.endpoint;
            connections.push_with_origin(
                CheckedConnection {
                    source_gear_id,
                    source_port_id: source_port.port_id,
                    sink_gear_id,
                    sink_port_id: sink_port.port_id,
                    value_kind: match connection_track {
                        conduit_core::ConnectionTrack::Payload => source_port.value_kind.clone(),
                        conduit_core::ConnectionTrack::NormalClose => {
                            conduit_core::kind_id(conduit_core::EMPTY_INFO_ID)
                        }
                        conduit_core::ConnectionTrack::Quiescence => {
                            conduit_core::kind_id(conduit_core::EMPTY_INFO_ID)
                        }
                        conduit_core::ConnectionTrack::AbnormalTerminal => source_port
                            .abnormal_kind
                            .clone()
                            .expect("abnormal projection validated its exact Fore kind"),
                    },
                    track: connection_track,
                    temporal: if connection_track == conduit_core::ConnectionTrack::Payload {
                        source_port.temporal
                    } else {
                        conduit_core::PortTemporal::Value
                    },
                },
                origin,
            );
        }
        (
            StageSource::FaceInput(name, value_type, temporal, abnormal_kind, track),
            StageSink::Internal(mut sink),
        ) => {
            sink.track = track;
            if track == conduit_core::ConnectionTrack::Payload {
                validate_front_contract(&name, &value_type, temporal, &sink.port, true)?;
            } else {
                let source = conduit_core::PortDescriptor {
                    port_id: conduit_core::port_id(name.as_str()),
                    value_kind: value_type,
                    direction: conduit_core::PortDirection::Output,
                    temporal,
                    abnormal_kind,
                };
                validate_connection_contract(&source, &sink.port, track)?;
            }
            let endpoints = inputs.entry(name.clone()).or_default();
            if endpoints.iter().any(|endpoint| {
                endpoint.gear_id == sink.gear_id && endpoint.port.port_id == sink.port.port_id
            }) {
                return Err(CanonicalExpansionDiagnostic::new(
                    "CND-FRM-047",
                    format!("runtime front input '{name}' repeats one internal binding"),
                ));
            }
            endpoints.push(sink);
        }
        (
            StageSource::Internal(source),
            StageSink::FaceOutput(name, value_type, temporal, abnormal_kind),
        ) => {
            if source.track == conduit_core::ConnectionTrack::Payload {
                validate_front_contract(&name, &value_type, temporal, &source.port, false)?;
            } else {
                let sink = conduit_core::PortDescriptor {
                    port_id: conduit_core::port_id(name.as_str()),
                    value_kind: value_type,
                    direction: conduit_core::PortDirection::Input,
                    temporal,
                    abnormal_kind,
                };
                validate_connection_contract(&source.port, &sink, source.track)?;
            }
            insert_boundary(outputs, name, source)?;
        }
        (StageSource::FaceInput(_, _, _, _, _), StageSink::FaceOutput(_, _, _, _)) => {
            return Err(CanonicalExpansionDiagnostic::new(
                "CND-FRM-046",
                "runtime front passthrough must cross an admitted gear".into(),
            ));
        }
    }
    Ok(())
}

fn connection_track(
    source: conduit_core::ConnectionTrack,
    sink: conduit_core::ConnectionTrack,
) -> Result<conduit_core::ConnectionTrack, CanonicalExpansionDiagnostic> {
    use conduit_core::ConnectionTrack;
    match (source, sink) {
        (ConnectionTrack::Payload, track) | (track, ConnectionTrack::Payload) => Ok(track),
        (left, right) if left == right => Ok(left),
        _ => Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-045",
            "one Cord cannot combine normal-close and abnormal-terminal tracks".into(),
        )),
    }
}

fn insert_boundary(
    boundaries: &mut BTreeMap<String, TrackedEndpoint>,
    name: String,
    endpoint: TrackedEndpoint,
) -> Result<(), CanonicalExpansionDiagnostic> {
    if boundaries.insert(name.clone(), endpoint).is_some() {
        return Err(CanonicalExpansionDiagnostic::new(
            "CND-FRM-047",
            format!("runtime front port '{name}' has multiple internal bindings"),
        ));
    }
    Ok(())
}
