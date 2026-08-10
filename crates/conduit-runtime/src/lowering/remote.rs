//! Exact lowering of the finite Line candidates sealed into a remote Cord.

use conduit_core::{AdmittedLine, ConnectionBase, PlanFragment, PlannedConnection};
use conduit_kernel::{CordId, RemoteEndpointId};

use super::{LoweredRemoteEndpoint, LoweringError, RemoteCordDirection};

pub(super) fn lower_remote_endpoints(
    fragment: &PlanFragment,
    connection: &PlannedConnection,
    cord: CordId,
    direction: RemoteCordDirection,
    remote_endpoints: &mut Vec<LoweredRemoteEndpoint>,
) -> Result<RemoteEndpointId, LoweringError> {
    let selected = connection
        .selected_line
        .as_ref()
        .ok_or_else(|| invalid(connection))?;
    if selected.binding.base == ConnectionBase::Local {
        return Err(invalid(connection));
    }

    let candidates = connection.admitted_lines.as_slice();
    if candidates.first() != Some(selected) {
        return Err(invalid(connection));
    }

    let first = RemoteEndpointId(super::as_u16(remote_endpoints.len())?);
    for candidate in candidates {
        validate_local_endpoint(fragment, connection, candidate, direction)?;
        let endpoint = RemoteEndpointId(super::as_u16(remote_endpoints.len())?);
        let (source_fragment_id, sink_fragment_id, local, peer) = match direction {
            RemoteCordDirection::Egress => (
                fragment.fragment_id.clone(),
                super::fragment_id_for_host(fragment, &candidate.binding.sink.host_id)?,
                candidate.binding.source.clone(),
                candidate.binding.sink.clone(),
            ),
            RemoteCordDirection::Ingress => (
                super::fragment_id_for_host(fragment, &candidate.binding.source.host_id)?,
                fragment.fragment_id.clone(),
                candidate.binding.sink.clone(),
                candidate.binding.source.clone(),
            ),
        };
        remote_endpoints.push(LoweredRemoteEndpoint {
            endpoint,
            cord,
            connection_id: connection.connection_id.clone(),
            source_fragment_id,
            sink_fragment_id,
            direction,
            local,
            peer,
            value_kind: connection.value_kind.clone(),
            temporal: connection.temporal,
            line: candidate.clone(),
        });
    }
    Ok(first)
}

fn validate_local_endpoint(
    fragment: &PlanFragment,
    connection: &PlannedConnection,
    candidate: &AdmittedLine,
    direction: RemoteCordDirection,
) -> Result<(), LoweringError> {
    let local = match direction {
        RemoteCordDirection::Egress => &candidate.binding.source,
        RemoteCordDirection::Ingress => &candidate.binding.sink,
    };
    if candidate.binding.base == ConnectionBase::Local
        || local.host_id != fragment.host_id
        || local.boot_id != fragment.boot_id
    {
        return Err(invalid(connection));
    }
    Ok(())
}

fn invalid(connection: &PlannedConnection) -> LoweringError {
    LoweringError::InvalidRemoteConnection(connection.connection_id.clone())
}
use alloc::vec::Vec;
