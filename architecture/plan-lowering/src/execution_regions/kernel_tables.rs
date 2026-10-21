//! Dense per-region tables for the existing kernel, with original identity maps.
use super::PreparedExecutionRegions;
use crate::lowering::{LoweredHostCall, LoweredRoute, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
use alloc::vec::Vec;
use conduit_kernel::{
    scheduler::{CordSpec, NodeSpec},
    CordEndpoint, CordId, NodeId, RemoteEndpointId, RouteRange, RouteTarget,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RegionTableError {
    UnknownRegion,
    UnsupportedExternalBoundary,
    UnsupportedRetainedContract,
    InvalidIdentity,
    CapacityExceeded,
}

/// A projection, not a new graph or a worker. Local numeric ordinals are dense
/// for the existing scheduler; each row retains its original lowered identity.
/// Transfer endpoints use the original Cord ordinal in both participating
/// regions. No Line, Host effect authority or connectivity is added.
pub struct PreparedRegionKernelTables<'prepared, 'plan> {
    prepared: &'prepared PreparedExecutionRegions<'plan>,
    region_index: usize,
    original_nodes: Vec<NodeId>,
    original_cords: Vec<CordId>,
    nodes: Vec<NodeSpec<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>>,
    cords: Vec<CordSpec>,
    routes: Vec<LoweredRoute>,
    host_calls: Vec<LoweredHostCall>,
}

impl<'plan> PreparedExecutionRegions<'plan> {
    pub fn prepare_kernel_tables<'prepared>(
        &'prepared self,
        region_index: usize,
    ) -> Result<PreparedRegionKernelTables<'prepared, 'plan>, RegionTableError> {
        use RegionTableError::{CapacityExceeded, InvalidIdentity};
        let region = self
            .regions()
            .get(region_index)
            .ok_or(RegionTableError::UnknownRegion)?;
        let lowered = self.lowered();
        // These contracts need their own executable ownership installation.
        // Refuse them rather than dropping them from a partial worker table.
        if !lowered.remote_endpoints.is_empty() || !lowered.fore_ports.is_empty() {
            return Err(RegionTableError::UnsupportedExternalBoundary);
        }
        if !lowered.states.is_empty()
            || !lowered.fusions.is_empty()
            || !lowered.resources.is_empty()
            || !lowered.resource_cords.is_empty()
            || !lowered.shared_pools.is_empty()
            || !self.activations().entries.is_empty()
        {
            return Err(RegionTableError::UnsupportedRetainedContract);
        }
        let original_nodes = region.nodes().to_vec();
        let original_cords = region.cords().to_vec();
        let local_node = |node| {
            original_nodes
                .iter()
                .position(|n| *n == node)
                .map(|n| u16::try_from(n).map(NodeId).map_err(|_| CapacityExceeded))
                .transpose()
        };
        let local_cord = |cord| {
            original_cords
                .iter()
                .position(|c| *c == cord)
                .ok_or(InvalidIdentity)
                .and_then(|n| u16::try_from(n).map(CordId).map_err(|_| CapacityExceeded))
        };
        let endpoint = |endpoint, cord| -> Result<CordEndpoint, RegionTableError> {
            match endpoint {
                CordEndpoint::Local { node, port } => {
                    if let Some(node) = local_node(node)? {
                        return Ok(CordEndpoint::local(node, port));
                    }
                    if self.transfers().iter().any(|t| {
                        t.cord() == cord
                            && (t.source_region() == region_index
                                || t.sink_region() == region_index)
                    }) {
                        Ok(CordEndpoint::Remote(RemoteEndpointId(cord.0)))
                    } else {
                        Err(InvalidIdentity)
                    }
                }
                CordEndpoint::Remote(_) => Err(RegionTableError::UnsupportedExternalBoundary),
            }
        };
        let mut nodes = Vec::with_capacity(original_nodes.len());
        for original in &original_nodes {
            let index = lowered
                .nodes
                .iter()
                .position(|n| n.node == *original)
                .ok_or(InvalidIdentity)?;
            let mut spec = *lowered.node_specs.get(index).ok_or(InvalidIdentity)?;
            for input in &mut spec.input_cords {
                if let Some(cord) = input {
                    *cord = local_cord(*cord)?;
                }
            }
            nodes.push(spec);
        }
        let mut cords = Vec::with_capacity(original_cords.len());
        let mut slot_start = 0u16;
        for original in &original_cords {
            let mut spec = lowered
                .cords
                .iter()
                .find(|c| c.spec.cord == *original)
                .ok_or(InvalidIdentity)?
                .spec;
            spec.source = endpoint(spec.source, *original)?;
            spec.sink = endpoint(spec.sink, *original)?;
            spec.cord = local_cord(*original)?;
            spec.slot_start = slot_start;
            slot_start = slot_start
                .checked_add(spec.item_capacity)
                .ok_or(CapacityExceeded)?;
            cords.push(spec);
        }
        let mut routes = Vec::new();
        let mut target_start = 0u16;
        for original in &lowered.routes {
            let Some(source_node) = local_node(original.source_node)? else {
                continue;
            };
            let mut targets = Vec::with_capacity(original.targets.len());
            for target in &original.targets {
                let cord = local_cord(target.cord)?;
                let sink = cords.get(usize::from(cord.0)).ok_or(InvalidIdentity)?.sink;
                targets.push(RouteTarget { cord, sink });
            }
            let len = u16::try_from(targets.len()).map_err(|_| CapacityExceeded)?;
            routes.push(LoweredRoute {
                source_node,
                source_port: original.source_port,
                range: RouteRange {
                    start: target_start,
                    len,
                },
                targets,
            });
            target_start = target_start.checked_add(len).ok_or(CapacityExceeded)?;
        }
        let mut host_calls = Vec::new();
        for original in &lowered.host_calls {
            if let Some(node) = local_node(original.node)? {
                let mut call = original.clone();
                call.node = node;
                host_calls.push(call);
            }
        }
        Ok(PreparedRegionKernelTables {
            prepared: self,
            region_index,
            original_nodes,
            original_cords,
            nodes,
            cords,
            routes,
            host_calls,
        })
    }
}
impl<'prepared, 'plan> PreparedRegionKernelTables<'prepared, 'plan> {
    pub fn prepared(&self) -> &'prepared PreparedExecutionRegions<'plan> {
        self.prepared
    }
    pub fn region_index(&self) -> usize {
        self.region_index
    }
    pub fn original_nodes(&self) -> &[NodeId] {
        &self.original_nodes
    }
    pub fn original_cords(&self) -> &[CordId] {
        &self.original_cords
    }
    pub fn nodes(&self) -> &[NodeSpec<FIXED_KERNEL_STORAGE_PORTS_PER_NODE>] {
        &self.nodes
    }
    pub fn cords(&self) -> &[CordSpec] {
        &self.cords
    }
    pub fn routes(&self) -> &[LoweredRoute] {
        &self.routes
    }
    pub fn host_calls(&self) -> &[LoweredHostCall] {
        &self.host_calls
    }
}
