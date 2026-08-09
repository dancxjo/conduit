//! Exact lowering of the finite Line candidates sealed into a remote Cord.

use conduit_core::{BoundLink, ConnectionBase, PlanFragment, PlannedConnection};
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
        .link_binding
        .as_ref()
        .ok_or_else(|| invalid(connection))?;
    if connection.base == ConnectionBase::Local || selected.base != connection.base {
        return Err(invalid(connection));
    }

    let selected_bound = selected.bound_link();
    let singleton;
    let candidates = if connection.route_candidates.is_empty() {
        singleton = [selected_bound.clone()];
        singleton.as_slice()
    } else {
        connection.route_candidates.as_slice()
    };
    if candidates.first() != Some(&selected_bound) {
        return Err(invalid(connection));
    }

    let first = RemoteEndpointId(super::as_u16(remote_endpoints.len())?);
    for candidate in candidates {
        validate_local_endpoint(fragment, connection, candidate, direction)?;
        let endpoint = RemoteEndpointId(super::as_u16(remote_endpoints.len())?);
        let (source_fragment_id, sink_fragment_id, local, peer) = match direction {
            RemoteCordDirection::Egress => (
                fragment.fragment_id.clone(),
                super::fragment_id_for_host(fragment, &candidate.sink.host_id)?,
                candidate.source.clone(),
                candidate.sink.clone(),
            ),
            RemoteCordDirection::Ingress => (
                super::fragment_id_for_host(fragment, &candidate.source.host_id)?,
                fragment.fragment_id.clone(),
                candidate.sink.clone(),
                candidate.source.clone(),
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
            binding: candidate.clone(),
        });
    }
    Ok(first)
}

fn validate_local_endpoint(
    fragment: &PlanFragment,
    connection: &PlannedConnection,
    candidate: &BoundLink,
    direction: RemoteCordDirection,
) -> Result<(), LoweringError> {
    let local = match direction {
        RemoteCordDirection::Egress => &candidate.source,
        RemoteCordDirection::Ingress => &candidate.sink,
    };
    if candidate.base == ConnectionBase::Local
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
