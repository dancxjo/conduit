//! Shared finite table installation for the existing std execution kernel.
//! Partitions retain their own provenance; this module creates no Plan or Play.
use super::{
    InstalledBack, InstalledScheduler, HOST_BINDING_SLOTS, HOST_CALLS_PER_NODE, MAX_CORDS,
    MAX_NODES, PORTS, ROUTE_SLOTS, ROUTE_TARGETS,
};
use conduit_kernel::scheduler::{AssignedTerminalTransduction, CordSpec, NodeSpec};
use conduit_kernel::{FixedHostCallBindings, FixedRoutes, HostedSignLog, HostedValueStore};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) struct KernelTables {
    active_nodes: usize,
    active_cords: usize,
    nodes: [NodeSpec<PORTS>; MAX_NODES],
    cords: [CordSpec; MAX_CORDS],
    routes: FixedRoutes<ROUTE_SLOTS, ROUTE_TARGETS>,
    host_bindings: FixedHostCallBindings<HOST_BINDING_SLOTS>,
    terminal_transductions: [[Option<AssignedTerminalTransduction>; PORTS]; MAX_NODES],
}

impl KernelTables {
    pub(super) fn prepare(partitions: &[&LoweredPlanFragment]) -> Result<Self, String> {
        let mut tables = Self {
            active_nodes: 0,
            active_cords: 0,
            nodes: [NodeSpec {
                input_cords: [None; PORTS],
                maximum_step_fuel: 1,
            }; MAX_NODES],
            cords: [CordSpec::inactive(); MAX_CORDS],
            routes: FixedRoutes::new(PORTS as u16),
            host_bindings: FixedHostCallBindings::new(HOST_CALLS_PER_NODE),
            terminal_transductions: [[None; PORTS]; MAX_NODES],
        };
        for partition in partitions {
            if partition.nodes.len() != partition.node_specs.len() {
                return Err("invalid kernel partition tables".into());
            }
            for (node, spec) in partition.nodes.iter().zip(&partition.node_specs) {
                if usize::from(node.node.0) != tables.active_nodes {
                    return Err("kernel partition nodes must be disjoint and contiguous".into());
                }
                *tables
                    .nodes
                    .get_mut(tables.active_nodes)
                    .ok_or_else(|| "combined kernel node capacity exceeded".to_string())? = *spec;
                for contract in &node.terminal_transductions {
                    let assigned = contract.assigned();
                    let input = usize::from(assigned.input.0);
                    if input >= PORTS
                        || tables.terminal_transductions[tables.active_nodes][input]
                            .replace(assigned)
                            .is_some()
                    {
                        return Err("invalid duplicate lowered terminal input".into());
                    }
                }
                tables.active_nodes += 1;
            }
            for cord in &partition.cords {
                if usize::from(cord.spec.cord.0) != tables.active_cords {
                    return Err("kernel partition Cords must be disjoint and contiguous".into());
                }
                *tables
                    .cords
                    .get_mut(tables.active_cords)
                    .ok_or_else(|| "combined kernel Cord capacity exceeded".to_string())? =
                    cord.spec;
                tables.active_cords += 1;
            }
            for route in &partition.routes {
                tables
                    .routes
                    .install(
                        route.source_node,
                        route.source_port,
                        route.range,
                        &route.targets,
                    )
                    .map_err(|error| format!("install std route: {error:?}"))?;
            }
            for operation in &partition.host_calls {
                tables
                    .host_bindings
                    .install(operation.node, operation.binding)
                    .map_err(|error| format!("install std host-call: {error:?}"))?;
            }
        }
        tables
            .routes
            .seal()
            .map_err(|error| format!("seal std routes: {error:?}"))?;
        tables
            .host_bindings
            .seal()
            .map_err(|error| format!("seal std Host Calls: {error:?}"))?;
        Ok(tables)
    }

    pub(super) fn install(
        self,
        drivers: [InstalledBack; MAX_NODES],
        values: HostedValueStore,
        sign: HostedSignLog,
    ) -> Result<InstalledScheduler, String> {
        let mut scheduler = InstalledScheduler::new_with_active_counts_and_host_calls(
            self.active_nodes,
            self.active_cords,
            self.nodes,
            self.cords,
            self.routes,
            self.host_bindings,
            drivers,
            values,
            sign,
        )
        .map_err(|error| format!("install std scheduler: {error:?}"))?;
        scheduler
            .bind_terminal_transductions(self.terminal_transductions)
            .map_err(|error| format!("bind std terminal transductions: {error:?}"))?;
        Ok(scheduler)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generic_tables_retain_lowered_remote_cord_endpoints() {
        let exact = conduit_signal_conformance::exact_distributed_signal_plan().unwrap();
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .unwrap();
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        assert_eq!(lowered.remote_endpoints.len(), 1);
        assert!(KernelTables::prepare(&[&lowered]).is_ok());
    }
}
