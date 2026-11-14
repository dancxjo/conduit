//! Selected external Fore and ordinary remote endpoint record projection.
use super::*;
pub(super) fn encode(
    plan: &GeneratedEmbeddedPlan,
    fore: &[conduit_core::AssignedForeRequirement],
    line_fore: Option<conduit_core::AssignedLineForeProfile<'_>>,
    record: &mut impl FnMut(u8, Vec<u8>) -> Result<(), GenerationError>,
) -> Result<(), GenerationError> {
    for port in fore {
        record(conduit_core::ASSIGNED_LOCAL_FORE, port.payload().to_vec())?;
    }
    if let Some(profile) = line_fore {
        for port in profile.local {
            record(
                conduit_core::ASSIGNED_LOCAL_FORE,
                conduit_core::assigned_line_fore_local_payload(port).to_vec(),
            )?;
        }
        for line in profile.lines {
            record(conduit_core::ASSIGNED_LOCAL_FORE, line.payload().to_vec())?;
        }
    }
    for remote in &plan.remote_endpoints {
        let mut value = Vec::new();
        for item in [
            &remote.line_id,
            &remote.local_host,
            &remote.local_boot,
            &remote.peer_host,
            &remote.peer_boot,
            &remote.connection_id,
            &remote.source_fragment_id,
            &remote.sink_fragment_id,
            &remote.local_endpoint,
            &remote.peer_endpoint,
            &remote.base_instance_id,
            &remote.link_binding_id,
            &remote.value_kind,
        ] {
            identity_to(&mut value, item);
        }
        u16_to(&mut value, remote.endpoint);
        u16_to(&mut value, remote.cord);
        identity_to(&mut value, remote.base.as_str());
        value.push(match remote.direction {
            RemoteCordDirection::Egress => 0,
            RemoteCordDirection::Ingress => 1,
        });
        value.extend_from_slice(&[
            remote.contract.scope as u8,
            remote.contract.traffic_shape as u8,
            remote.contract.duplex as u8,
            remote.contract.ordering as u8,
            remote.contract.reliability as u8,
            remote.contract.continuation as u8,
            remote.contract.security as u8,
        ]);
        u16_to(&mut value, remote.maximum_in_flight_items);
        u32_to(&mut value, remote.maximum_payload_bytes);
        u32_to(&mut value, remote.maximum_buffered_bytes);
        u32_to(&mut value, remote.maximum_frame_bytes);
        record(ASSIGNED_REMOTE_ENDPOINT, value)?;
    }
    Ok(())
}
