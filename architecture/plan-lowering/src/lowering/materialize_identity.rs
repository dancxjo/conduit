//! Final owned correlations; all source tables coexist and are counted.
use super::*;
pub(super) struct IdentityTables<'a> {
    pub(super) nodes: &'a [LoweredNode],
    pub(super) cords: &'a [LoweredCord],
    pub(super) remote: &'a [LoweredRemoteEndpoint],
    pub(super) fore: &'a [LoweredForePort],
    pub(super) calls: &'a [LoweredHostCall],
    pub(super) resources: &'a [LoweredResource],
    pub(super) resource_cords: &'a [LoweredResourceCord],
}
pub(super) fn identity(
    fragment: &PlanFragment,
    ports: Vec<KernelPortIdentity>,
    tables: IdentityTables<'_>,
) -> Result<KernelIdentityMap, LoweringError> {
    let IdentityTables {
        nodes,
        cords,
        remote,
        fore,
        calls,
        resources,
        resource_cords,
    } = tables;
    let mut bindings = reserved(resources.len())?;
    bindings.extend(
        resources
            .iter()
            .zip(fragment.placements.iter().flat_map(|p| &p.resources))
            .map(|(item, binding)| (item.node, item.binding.resource, binding.clone())),
    );
    Ok(KernelIdentityMap {
        plan_id: fragment.plan_id.clone(),
        fragment_id: fragment.fragment_id.clone(),
        placements: collect_exact(nodes.iter().map(|n| (n.node, n.placement_id.clone())))?,
        ports,
        connections: collect_exact(cords.iter().map(|c| (c.spec.cord, c.connection_id.clone())))?,
        remote_endpoints: collect_exact(
            remote.iter().map(|r| (r.endpoint, r.connection_id.clone())),
        )?,
        fore_endpoints: collect_exact(fore.iter().map(|f| KernelForeEndpointIdentity {
            endpoint: f.endpoint,
            cord: f.cord,
            front_port_id: f.front_port_id.clone(),
            direction: f.direction,
            track: f.track,
            value_kind: f.value_kind.clone(),
        }))?,
        host_calls: collect_exact(
            calls
                .iter()
                .map(|c| (c.node, c.call, c.contract_id.clone())),
        )?,
        resources: bindings,
        resource_connections: collect_exact(
            resource_cords
                .iter()
                .map(|r| (r.source_node, r.resource, r.connection_id.clone())),
        )?,
    })
}
