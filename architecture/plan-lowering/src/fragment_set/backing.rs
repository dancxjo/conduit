//! Borrowed capacity inspection of the qualified final local lowering owners.
use super::*;
use core::mem::size_of;
fn table<T>(value: &Vec<T>) -> Option<usize> {
    value.capacity().checked_mul(size_of::<T>())
}
fn add(total: &mut usize, bytes: usize) -> Option<()> {
    *total = total.checked_add(bytes)?;
    Some(())
}
macro_rules! tables {
    ($total:ident, $owner:ident, $($field:ident),+ $(,)?) => { $(add(&mut $total, table(&$owner.$field)?)?;)+ };
}
/// Actual String/Vec capacities, excluding the inline owner and its borrowed
/// source. Unsupported nested owners refuse inspection instead of undercounting.
/// This checks retained drift, not verifier peak or any execution permission.
pub fn lowered_local_fragment_set_backing_bytes(value: &LoweredFragmentSet) -> Option<usize> {
    let mut bytes = table(&value.partitions)?;
    for part in &value.partitions {
        if !part.fusions.is_empty()
            || !part.shared_pools.is_empty()
            || !part.remote_endpoints.is_empty()
            || !part.resource_cords.is_empty()
        {
            return None;
        }
        tables!(
            bytes,
            part,
            nodes,
            node_specs,
            cords,
            remote_endpoints,
            resource_cords,
            fore_ports,
            routes,
            host_calls,
            resources,
            signs,
            states,
            fusions,
            shared_pools
        );
        let identity = &part.identity;
        tables!(
            bytes,
            identity,
            placements,
            ports,
            connections,
            remote_endpoints,
            fore_endpoints,
            host_calls,
            resources,
            resource_connections
        );
        add(&mut bytes, identity.plan_id.0.capacity())?;
        add(&mut bytes, identity.fragment_id.0.capacity())?;
        for (_, id) in &identity.placements {
            add(&mut bytes, id.0.capacity())?;
        }
        for port in &identity.ports {
            add(&mut bytes, port.port_id.0.capacity())?;
        }
        for (_, id) in &identity.connections {
            add(&mut bytes, id.0.capacity())?;
        }
        for fore in &identity.fore_endpoints {
            add(&mut bytes, fore.front_port_id.0.capacity())?;
            add(&mut bytes, fore.value_kind.0.capacity())?;
        }
        for (_, _, id) in &identity.host_calls {
            add(&mut bytes, id.0.capacity())?;
        }
        for (_, _, binding) in &identity.resources {
            if binding.protected.is_some() || binding.compute.is_some() {
                return None;
            }
            add(&mut bytes, binding.pool_id.0.capacity())?;
            add(&mut bytes, binding.class_id.0.capacity())?;
            if let Some(content) = &binding.content {
                for capacity in [
                    content.contract.content_profile.0.capacity(),
                    content.owner_host.0.capacity(),
                    content.owner_boot.0.capacity(),
                    content.base_id.0.capacity(),
                    content.residence_profile.0.capacity(),
                ] {
                    add(&mut bytes, capacity)?;
                }
            }
        }
        for state in &part.states {
            add(&mut bytes, state.contract.local_retained_backing_bytes()?)?;
        }
        for node in &part.nodes {
            if !node.terminal_transductions.is_empty() {
                return None;
            }
            tables!(bytes, node, inputs, outputs, terminal_transductions);
            add(&mut bytes, node.placement_id.0.capacity())?;
            for port in node.inputs.iter().chain(&node.outputs) {
                add(&mut bytes, port.port_id.0.capacity())?;
                add(&mut bytes, port.value_kind.0.capacity())?;
                if let Some(id) = &port.abnormal_kind {
                    add(&mut bytes, id.0.capacity())?;
                }
            }
        }
        for cord in &part.cords {
            add(&mut bytes, cord.connection_id.0.capacity())?;
        }
        for fore in &part.fore_ports {
            if fore.selected_line.is_some() {
                return None;
            }
            add(&mut bytes, fore.front_port_id.0.capacity())?;
            add(&mut bytes, fore.value_kind.0.capacity())?;
            if let Some(id) = &fore.abnormal_kind {
                add(&mut bytes, id.0.capacity())?;
            }
            if let Some(contract) = &fore.value_contract {
                if !contract.constraints.is_empty() {
                    return None;
                }
                add(&mut bytes, table(&contract.constraints)?)?;
                add(&mut bytes, contract.value_kind.0.capacity())?;
            }
        }
        for route in &part.routes {
            add(&mut bytes, table(&route.targets)?)?;
        }
        for call in &part.host_calls {
            add(&mut bytes, call.contract_id.0.capacity())?;
            if let Some(id) = &call.target_kind {
                add(&mut bytes, id.0.capacity())?;
            }
        }
        for sign in &part.signs {
            use conduit_core::ExpectedSign::*;
            match &sign.expected {
                PlacementPrepared(id) | PlacementTerminal(id) => add(&mut bytes, id.0.capacity())?,
                ConnectionTerminal(id) => add(&mut bytes, id.0.capacity())?,
                PlanFragmentReceived | PlanTerminal => {}
            }
        }
    }
    Some(bytes)
}
