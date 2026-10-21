//! Installation view shared by ordinary children and exact region projections.
use crate::prelude::*;
use conduit_kernel::scheduler::NodeSpec;
use conduit_plan_lowering::{
    execution_regions::PreparedRegionKernelTables,
    lowering::{
        LoweredCord, LoweredForePort, LoweredHostCall, LoweredNode, LoweredPlanFragment,
        LoweredRoute, FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
    },
};

pub(super) struct ChildPreparationTables {
    pub nodes: Vec<LoweredNode>,
    pub node_specs: Vec<NodeSpec<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>>,
    pub cords: Vec<LoweredCord>,
    pub fore_ports: Vec<LoweredForePort>,
    pub routes: Vec<LoweredRoute>,
    pub host_calls: Vec<LoweredHostCall>,
    pub cord_value_slots: u16,
    pub cord_value_bytes: u32,
    pub sign_items: u16,
}
impl From<LoweredPlanFragment> for ChildPreparationTables {
    fn from(lowered: LoweredPlanFragment) -> Self {
        Self {
            nodes: lowered.nodes,
            node_specs: lowered.node_specs,
            cords: lowered.cords,
            fore_ports: lowered.fore_ports,
            routes: lowered.routes,
            host_calls: lowered.host_calls,
            cord_value_slots: lowered.cord_value_slots,
            cord_value_bytes: lowered.cord_value_bytes,
            sign_items: lowered.sign_items,
        }
    }
}
impl ChildPreparationTables {
    pub(super) fn from_region(tables: &PreparedRegionKernelTables<'_, '_>) -> Result<Self, String> {
        let original = tables.prepared().lowered();
        let mut nodes = Vec::with_capacity(tables.original_nodes().len());
        for (local, original_node) in tables.original_nodes().iter().enumerate() {
            let mut node = original
                .nodes
                .iter()
                .find(|n| n.node == *original_node)
                .ok_or_else(|| "region node identity disappeared".to_string())?
                .clone();
            let local = conduit_kernel::NodeId(
                u16::try_from(local).map_err(|_| "region node capacity overflow")?,
            );
            node.node = local;
            for port in node.inputs.iter_mut().chain(&mut node.outputs) {
                port.node = local;
            }
            nodes.push(node);
        }
        let mut cords = Vec::with_capacity(tables.cords().len());
        let mut slots = 0u16;
        let mut bytes = 0u32;
        for (id, spec) in tables.original_cords().iter().zip(tables.cords()) {
            let original = original
                .cords
                .iter()
                .find(|c| c.spec.cord == *id)
                .ok_or_else(|| "region Cord identity disappeared".to_string())?;
            slots = slots
                .checked_add(spec.item_capacity)
                .ok_or("region slot capacity overflow")?;
            bytes = bytes
                .checked_add(spec.byte_capacity)
                .ok_or("region byte capacity overflow")?;
            cords.push(LoweredCord {
                connection_id: original.connection_id.clone(),
                spec: *spec,
            });
        }
        Ok(Self {
            nodes,
            node_specs: tables.nodes().to_vec(),
            cords,
            fore_ports: Vec::new(),
            routes: tables.routes().to_vec(),
            host_calls: tables.host_calls().to_vec(),
            cord_value_slots: slots,
            cord_value_bytes: bytes,
            sign_items: original.sign_items,
        })
    }
}
