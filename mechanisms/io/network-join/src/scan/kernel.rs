//! Exact one-shot scan → evidence execution profile under the ordinary kernel.
//! Preparation grants neither radio possession nor another recovery round.
use super::NetworkScanBack;
use conduit_core::PlanFragment;
use conduit_kernel::{self as k, scheduler::FixedScheduler, ValueStorage};
use conduit_net::{MAXIMUM_SCAN_OUTPUT_BYTES, NETWORK_SCAN_OPERATION, NETWORK_SCAN_SIGN_OPERATION};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

const VALUE_BYTES: usize = MAXIMUM_SCAN_OUTPUT_BYTES as usize;
const SIGN_ITEMS: u16 = 128;
pub type NetworkScanKernel = FixedScheduler<
    NetworkScanBack,
    k::FixedValueStore<2, VALUE_BYTES>,
    k::HostedSignLog,
    2,
    1,
    1,
    1,
    2,
    1,
    2,
    2,
>;
pub const SCAN_SIGN_BACKING_BYTES: usize =
    match k::HostedSignLog::canonical_storage_bytes(SIGN_ITEMS, 0) {
        Some(bytes) => bytes,
        None => panic!("fixed scan Sign storage"),
    };
pub const SCAN_KERNEL_BYTES: usize =
    core::mem::size_of::<PreparedScanKernel>() + SCAN_SIGN_BACKING_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanKernelRefusal {
    Identity,
    Lowering,
    Shape,
    Storage,
    Tables,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanKernelRoles {
    pub scan: k::NodeId,
    pub evidence: k::NodeId,
}
pub struct PreparedScanKernel {
    pub kernel: NetworkScanKernel,
    pub roles: ScanKernelRoles,
}
/// Opaque canonical numeric preparation; arbitrary caller tables cannot enter.
pub struct PreparedScanKernelParts {
    nodes: [k::scheduler::NodeSpec<1>; 2],
    cord: k::scheduler::CordSpec,
    routes: k::FixedRoutes<2, 1>,
    calls: k::FixedHostCallBindings<2>,
    roles: ScanKernelRoles,
}
impl PreparedScanKernelParts {
    pub fn prepare(fragment: &PlanFragment) -> Result<Self, ScanKernelRefusal> {
        if !conduit_core::verify_plan_fragment(fragment) {
            return Err(ScanKernelRefusal::Identity);
        }
        let lowered = lower_plan_fragment(fragment).map_err(|_| ScanKernelRefusal::Lowering)?;
        Self::from_canonical(fragment, &lowered)
    }
    fn from_canonical(
        f: &PlanFragment,
        l: &LoweredPlanFragment,
    ) -> Result<Self, ScanKernelRefusal> {
        use ScanKernelRefusal as E;
        if l.nodes.len() != 2
            || l.node_specs.len() != 2
            || l.cords.len() != 1
            || l.host_calls.len() != 2
            || l.resources.len() != 1
            || l.nodes
                .iter()
                .enumerate()
                .any(|(i, n)| usize::from(n.node.0) != i)
            || !l.remote_endpoints.is_empty()
            || !l.states.is_empty()
            || !l.fusions.is_empty()
            || !l.fore_ports.is_empty()
            || !l.resource_cords.is_empty()
            || !l.shared_pools.is_empty()
            || !f.execution_regions.is_empty()
            || l.nodes.iter().any(|n| !n.terminal_transductions.is_empty())
            || f.placements
                .iter()
                .any(|g| g.inputs.len() > 1 || g.outputs.len() > 1 || !g.configuration.is_empty())
            || l.node_specs
                .iter()
                .any(|n| n.input_cords[1..].iter().any(Option::is_some))
        {
            return Err(E::Shape);
        }
        if l.cord_value_slots != 1
            || l.cord_value_bytes > MAXIMUM_SCAN_OUTPUT_BYTES
            || l.sign_items > SIGN_ITEMS
            || l.sign_bytes > u32::from(SIGN_ITEMS) * core::mem::size_of::<k::KernelEvent>() as u32
        {
            return Err(E::Storage);
        }
        let find = |kind: &str| {
            let mut nodes = l.nodes.iter().filter(|n| {
                f.placements
                    .iter()
                    .any(|g| g.placement_id == n.placement_id && g.kind_id.as_str() == kind)
            });
            let node = nodes.next().ok_or(E::Shape)?.node;
            if nodes.next().is_some() {
                return Err(E::Shape);
            }
            Ok(node)
        };
        let roles = ScanKernelRoles {
            scan: find(NETWORK_SCAN_OPERATION)?,
            evidence: find(NETWORK_SCAN_SIGN_OPERATION)?,
        };
        if roles.scan == roles.evidence || l.resources[0].node != roles.scan {
            return Err(E::Shape);
        }
        let cord = l.cords[0].spec;
        if cord.source_local() != Some((roles.scan, k::PortId(0)))
            || cord.sink_local() != Some((roles.evidence, k::PortId(0)))
            || cord.item_capacity != 1
            || cord.byte_capacity > MAXIMUM_SCAN_OUTPUT_BYTES
            || cord.maximum_value_bytes > MAXIMUM_SCAN_OUTPUT_BYTES
            || l.node_specs[usize::from(roles.scan.0)].input_cords[0].is_some()
            || l.node_specs[usize::from(roles.evidence.0)].input_cords[0] != Some(cord.cord)
        {
            return Err(E::Shape);
        }
        let mut routes = k::FixedRoutes::<2, 1>::new(1);
        for r in &l.routes {
            routes
                .install(r.source_node, r.source_port, r.range, &r.targets)
                .map_err(|_| E::Tables)?;
        }
        routes.seal().map_err(|_| E::Tables)?;
        let mut calls = k::FixedHostCallBindings::<2>::new(1);
        for c in &l.host_calls {
            let expected = if c.node == roles.scan {
                conduit_net::NETWORK_SCAN_HOST_CALL
            } else if c.node == roles.evidence {
                conduit_net::NETWORK_SCAN_SIGN_HOST_CALL
            } else {
                return Err(E::Shape);
            };
            if c.call != k::HostCallId(0)
                || c.maximum_in_flight != 1
                || c.contract_id.as_str() != expected
            {
                return Err(E::Shape);
            }
            calls.install(c.node, c.binding).map_err(|_| E::Tables)?;
        }
        calls.seal().map_err(|_| E::Tables)?;
        Ok(Self {
            nodes: core::array::from_fn(|i| k::scheduler::NodeSpec {
                input_cords: [l.node_specs[i].input_cords[0]],
                maximum_step_fuel: l.node_specs[i].maximum_step_fuel,
            }),
            cord,
            routes,
            calls,
            roles,
        })
    }
    fn finish(self, signs: k::HostedSignLog) -> Result<PreparedScanKernel, ScanKernelRefusal> {
        use ScanKernelRefusal as E;
        // Two original physical values cover the pinned empty seed and the
        // single maximum observation output until all Host consumers retire.
        let mut values = k::FixedValueStore::<2, VALUE_BYTES>::new((2 * VALUE_BYTES) as u32)
            .map_err(|_| E::Storage)?;
        let empty = values.store(&[]).map_err(|_| E::Storage)?;
        let drivers = core::array::from_fn(|i| {
            if i == usize::from(self.roles.scan.0) {
                NetworkScanBack::source(empty).expect("reserved empty scan input")
            } else {
                NetworkScanBack::evidence_sink()
            }
        });
        let kernel = NetworkScanKernel::new_with_host_calls(
            self.nodes,
            [self.cord],
            self.routes,
            self.calls,
            drivers,
            values,
            signs,
        )
        .map_err(|_| E::Tables)?;
        Ok(PreparedScanKernel {
            kernel,
            roles: self.roles,
        })
    }
}
/// Fallible separately reserved execution backing. No initialized access before
/// canonical tables and mandatory Sign backing have both been prepared.
pub struct PreparedScanKernelStorage {
    backing: alloc::boxed::Box<core::mem::MaybeUninit<PreparedScanKernel>>,
    signs: k::HostedSignLog,
}
impl PreparedScanKernelStorage {
    pub const fn allocation_bytes() -> usize {
        SCAN_KERNEL_BYTES
    }
    pub fn try_new() -> Result<Self, ScanKernelRefusal> {
        let layout = core::alloc::Layout::new::<PreparedScanKernel>();
        // SAFETY: exact unique nonzero allocation, kept uninitialized until
        // Box::write; a failed Sign reservation drops this same backing.
        let raw = unsafe { alloc::alloc::alloc(layout) }
            .cast::<core::mem::MaybeUninit<PreparedScanKernel>>();
        if raw.is_null() {
            return Err(ScanKernelRefusal::Storage);
        }
        // SAFETY: same allocation and exact original layout.
        let backing = unsafe { alloc::boxed::Box::from_raw(raw) };
        let signs = k::HostedSignLog::try_new(
            SIGN_ITEMS,
            u32::from(SIGN_ITEMS) * core::mem::size_of::<k::KernelEvent>() as u32,
        )
        .map_err(|_| ScanKernelRefusal::Storage)?;
        Ok(Self { backing, signs })
    }
    pub fn finish(
        self,
        parts: PreparedScanKernelParts,
    ) -> Result<alloc::boxed::Box<PreparedScanKernel>, ScanKernelRefusal> {
        let prepared = parts.finish(self.signs)?;
        Ok(alloc::boxed::Box::write(self.backing, prepared))
    }
}
impl PreparedScanKernel {
    /// Return original mandatory Signs only after the production scheduler's
    /// terminal retirement gate. This grants no native cleanup or quota renewal.
    #[inline(never)]
    pub fn try_retire_signs(
        self: alloc::boxed::Box<Self>,
    ) -> Result<k::HostedSignLog, alloc::boxed::Box<Self>> {
        if !self.kernel.can_retire() {
            return Err(self);
        }
        let Self { kernel, .. } = *self;
        let retired = kernel
            .try_retire()
            .unwrap_or_else(|_| unreachable!("exclusive terminal scan scheduler"));
        Ok(retired.signs)
    }
}
