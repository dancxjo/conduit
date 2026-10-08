//! Fixed numeric preparation data; no Plan objects, heap storage or provider pointers.
use super::{CORDS, NODES, PORTS};
use conduit_kernel::{
    HostCallBinding, NodeId, PortId, RouteRange, RouteTarget,
    scheduler::{CordSpec, NodeSpec},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedTimerRoute {
    pub node: NodeId,
    pub port: PortId,
    pub range: RouteRange,
    pub target: RouteTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedTimerGraph {
    pub nodes: [NodeSpec<PORTS>; NODES],
    pub cords: [CordSpec; CORDS],
    pub routes: [Option<PreparedTimerRoute>; CORDS],
    pub bindings: [Option<(NodeId, HostCallBinding)>; NODES],
    pub timer: NodeId,
    pub count: NodeId,
    pub presentation: NodeId,
    pub period: u64,
    pub start: u64,
    pub sign_bytes: u32,
}
