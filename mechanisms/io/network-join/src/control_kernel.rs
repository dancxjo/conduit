//! Exact fixed storage for the three-gear network control Play. This prepares
//! the production kernel; it neither issues authority nor dispatches Host Calls.
use crate::NetworkControlBack;
use conduit_core::PlanFragment;
use conduit_kernel::{self as k, scheduler::FixedScheduler, ValueStorage};
use conduit_plan_lowering::lowering::{lower_plan_fragment, LoweredPlanFragment};

// This exact three-gear control uses only ordinal zero. Renew this finite
// profile if its Fore grows; do not reserve the general 16-port envelope.
pub const CONTROL_PORTS: usize = 1;
pub type NetworkControlKernel = FixedScheduler<
    NetworkControlBack,
    k::FixedValueStore<8, 167>,
    k::HostedSignLog,
    3,
    2,
    CONTROL_PORTS,
    2,
    { 3 * CONTROL_PORTS },
    2,
    3,
    3,
>;
// The retained footprint includes the exact preallocated local sign backing.
// There are no remote endpoints and therefore no remote-sign allocation.
pub const CONTROL_SIGN_BACKING_BYTES: usize =
    match k::HostedSignLog::canonical_storage_bytes(128, 0) {
        Some(bytes) => bytes,
        None => panic!("fixed network Sign storage"),
    };
pub const CONTROL_KERNEL_BYTES: usize =
    core::mem::size_of::<NetworkControlKernel>() + CONTROL_SIGN_BACKING_BYTES;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkControlRoles {
    pub credentials: k::NodeId,
    pub join: k::NodeId,
    pub attachment_sign: k::NodeId,
}
#[derive(Debug, PartialEq, Eq)]
pub enum ControlKernelRefusal {
    Identity,
    Lowering,
    Shape,
    Tables,
    Storage,
}

pub struct PreparedNetworkKernel {
    pub kernel: NetworkControlKernel,
    pub roles: NetworkControlRoles,
}
impl PreparedNetworkKernel {
    /// Retain original mandatory Signs after complete scheduler retirement.
    /// Active work returns the identical Box. This retires no native provider,
    /// commits no evidence and grants no replacement execution or erase rights.
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
            .unwrap_or_else(|_| unreachable!("exclusive terminal scheduler preflight"));
        let k::scheduler::RetiredExecution { signs, .. } = retired;
        Ok(signs)
    }
}
/// Finite execution backing only. This does not contain Source, Plan, grants,
/// initialized providers or permission to dispatch. Private fields guarantee
/// the exact empty 128-entry Sign log and unique uninitialized kernel backing.
pub struct PreparedNetworkKernelStorage {
    backing: alloc::boxed::Box<core::mem::MaybeUninit<PreparedNetworkKernel>>,
    signs: k::HostedSignLog,
}
impl PreparedNetworkKernelStorage {
    pub const fn allocation_bytes() -> usize {
        core::mem::size_of::<PreparedNetworkKernel>() + CONTROL_SIGN_BACKING_BYTES
    }
    pub fn try_new() -> Result<Self, ControlKernelRefusal> {
        let layout = core::alloc::Layout::new::<PreparedNetworkKernel>();
        // SAFETY: exact unique nonzero backing, initialized only by Box::write.
        let raw = unsafe { alloc::alloc::alloc(layout) }
            .cast::<core::mem::MaybeUninit<PreparedNetworkKernel>>();
        if raw.is_null() {
            return Err(ControlKernelRefusal::Storage);
        }
        let backing = unsafe { alloc::boxed::Box::from_raw(raw) };
        let signs =
            k::HostedSignLog::try_new(128, (128 * core::mem::size_of::<k::KernelEvent>()) as u32)
                .map_err(|_| ControlKernelRefusal::Storage)?;
        Ok(Self { backing, signs })
    }
    /// Observes allocation identity only; it cannot borrow an initialized kernel.
    pub fn as_ptr(&self) -> *const PreparedNetworkKernel {
        self.backing.as_ptr()
    }
    #[inline(never)]
    pub fn finish(
        self,
        parts: PreparedNetworkKernelParts,
    ) -> Result<alloc::boxed::Box<PreparedNetworkKernel>, ControlKernelRefusal> {
        let kernel = parts.finish_with_signs(self.signs)?;
        Ok(alloc::boxed::Box::write(self.backing, kernel))
    }
}
/// Opaque result of actual canonical fragment verification and lowering.
/// No caller-supplied numeric tables can construct this certificate.
pub struct CheckedNetworkLowering {
    fragment_id: conduit_core::FragmentId,
    lowered: LoweredPlanFragment,
}
impl CheckedNetworkLowering {
    pub fn prepare(fragment: &PlanFragment) -> Result<Self, ControlKernelRefusal> {
        if !conduit_core::verify_plan_fragment(fragment) {
            return Err(ControlKernelRefusal::Identity);
        }
        Self::prepare_lowering(fragment).map_err(|_| ControlKernelRefusal::Lowering)
    }
    /// Preserve canonical lowering diagnostics for ordinary planner callers.
    pub fn prepare_lowering(
        fragment: &PlanFragment,
    ) -> Result<Self, conduit_plan_lowering::lowering::LoweringError> {
        if !conduit_core::verify_plan_fragment(fragment) {
            return Err(conduit_plan_lowering::lowering::LoweringError::InvalidFragment);
        }
        Ok(Self {
            fragment_id: fragment.fragment_id.clone(),
            lowered: lower_plan_fragment(fragment)?,
        })
    }
    pub fn lowered(&self) -> &LoweredPlanFragment {
        &self.lowered
    }
    pub fn kernel_parts(
        &self,
        fragment: &PlanFragment,
    ) -> Result<PreparedNetworkKernelParts, ControlKernelRefusal> {
        if fragment.fragment_id != self.fragment_id || !conduit_core::verify_plan_fragment(fragment)
        {
            return Err(ControlKernelRefusal::Identity);
        }
        checked_parts(fragment, &self.lowered)
    }
}
/// Exact checked numeric tables. This is structural preparation, not permission to run.
pub struct PreparedNetworkKernelParts {
    nodes: [k::scheduler::NodeSpec<CONTROL_PORTS>; 3],
    cords: [k::scheduler::CordSpec; 2],
    routes: k::FixedRoutes<{ 3 * CONTROL_PORTS }, 2>,
    bindings: k::FixedHostCallBindings<3>,
    roles: NetworkControlRoles,
}
impl PreparedNetworkKernel {
    /// Root must still retain exact selected Host Call owners and admit this
    /// complete fixed storage/work profile before starting Play. This function
    /// accepts no serialized assignment or descriptor-derived resident layout.
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, ControlKernelRefusal> {
        use ControlKernelRefusal as E;
        let checked = CheckedNetworkLowering::prepare(fragment)?;
        if checked.lowered() != lowered {
            return Err(E::Lowering);
        }
        checked.kernel_parts(fragment)?.finish()
    }
}
fn checked_parts(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<PreparedNetworkKernelParts, ControlKernelRefusal> {
    use ControlKernelRefusal as E;
    if lowered.nodes.len() != 3
        || lowered.node_specs.len() != 3
        || lowered.cords.len() != 2
        || lowered.host_calls.len() != 3
        || lowered
            .nodes
            .iter()
            .enumerate()
            .any(|(i, n)| usize::from(n.node.0) != i)
        || !lowered.remote_endpoints.is_empty()
        || !lowered.states.is_empty()
        || !lowered.fusions.is_empty()
        || !lowered.fore_ports.is_empty()
        || !lowered.resource_cords.is_empty()
        || !lowered.shared_pools.is_empty()
        || !fragment.execution_regions.is_empty()
        || lowered
            .nodes
            .iter()
            .any(|n| !n.terminal_transductions.is_empty())
        || lowered.resources.len() != 2
        || lowered.node_specs.iter().any(|node| {
            node.input_cords[CONTROL_PORTS..]
                .iter()
                .any(Option::is_some)
        })
        || fragment
            .placements
            .iter()
            .any(|gear| gear.inputs.len() > CONTROL_PORTS || gear.outputs.len() > CONTROL_PORTS)
    {
        return Err(E::Shape);
    }
    if lowered.cord_value_slots > 8
        || lowered.cord_value_bytes > 8 * 167
        || lowered.sign_items > 128
        || lowered.sign_bytes > (128 * core::mem::size_of::<k::KernelEvent>()) as u32
    {
        return Err(E::Storage);
    }
    let find = |kind: &str| {
        let mut found = lowered.nodes.iter().filter(|n| {
            fragment
                .placements
                .iter()
                .any(|g| g.placement_id == n.placement_id && g.kind_id.as_str() == kind)
        });
        let node = found.next().ok_or(E::Shape)?.node;
        if found.next().is_some() {
            return Err(E::Shape);
        }
        Ok(node)
    };
    let roles = NetworkControlRoles {
        credentials: find(conduit_net::NETWORK_CREDENTIALS_OPERATION)?,
        join: find(conduit_net::NETWORK_JOIN_OPERATION)?,
        attachment_sign: find(conduit_net::NETWORK_ATTACHMENT_SIGN_OPERATION)?,
    };
    if roles.credentials == roles.join
        || roles.join == roles.attachment_sign
        || roles.credentials == roles.attachment_sign
    {
        return Err(E::Shape);
    }
    let mut routes = k::FixedRoutes::<{ 3 * CONTROL_PORTS }, 2>::new(CONTROL_PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| E::Tables)?;
    }
    routes.seal().map_err(|_| E::Tables)?;
    let mut bindings = k::FixedHostCallBindings::<3>::new(1);
    for call in &lowered.host_calls {
        if call.call != k::HostCallId(0) || call.maximum_in_flight != 1 {
            return Err(E::Shape);
        }
        bindings
            .install(call.node, call.binding)
            .map_err(|_| E::Tables)?;
    }
    bindings.seal().map_err(|_| E::Tables)?;
    Ok(PreparedNetworkKernelParts {
        nodes: core::array::from_fn(|i| k::scheduler::NodeSpec {
            input_cords: [lowered.node_specs[i].input_cords[0]],
            maximum_step_fuel: lowered.node_specs[i].maximum_step_fuel,
        }),
        cords: core::array::from_fn(|i| lowered.cords[i].spec),
        routes,
        bindings,
        roles,
    })
}
pub type NetworkControlDirectTables<'a> =
    conduit_plan_lowering::local_tables::DirectLocalKernelTables<'a, 3, 2, 1, 3, 2, 3>;
impl PreparedNetworkKernelParts {
    /// Consume only immutable verified canonical tables, never caller-owned
    /// numeric parts. All network-specific profile limits remain unchanged.
    pub fn check_direct(
        tables: &NetworkControlDirectTables<'_>,
    ) -> Result<NetworkControlRoles, ControlKernelRefusal> {
        let (plan, fragment) = tables.preparation_scope();
        let view = plan
            .fragment(&fragment.fragment_id)
            .ok_or(ControlKernelRefusal::Identity)?;
        let empty = fragment
            .placements
            .iter()
            .all(|g| view.configuration_matches(&g.placement_id, &[]));
        check_direct_profile(
            conduit_core::PlannedGearRefs::Owned(&fragment.placements),
            &fragment.execution_regions,
            tables.cord_value_slots(),
            tables.cord_value_bytes(),
            tables.sign_items(),
            tables.sign_bytes(),
            empty,
        )
    }
    pub fn from_direct(
        tables: NetworkControlDirectTables<'_>,
    ) -> Result<Self, ControlKernelRefusal> {
        let roles = Self::check_direct(&tables)?;
        let parts = tables.into_parts();
        Ok(Self {
            nodes: parts.nodes,
            cords: parts.cords,
            routes: parts.routes,
            bindings: parts.calls,
            roles,
        })
    }
    pub fn roles(&self) -> NetworkControlRoles {
        self.roles
    }
    /// Materialize full unchanged storage only after caller retires construction metadata.
    pub fn finish(self) -> Result<PreparedNetworkKernel, ControlKernelRefusal> {
        let signs =
            k::HostedSignLog::try_new(128, (128 * core::mem::size_of::<k::KernelEvent>()) as u32)
                .map_err(|_| ControlKernelRefusal::Storage)?;
        self.finish_with_signs(signs)
    }
    fn finish_with_signs(
        self,
        signs: k::HostedSignLog,
    ) -> Result<PreparedNetworkKernel, ControlKernelRefusal> {
        use ControlKernelRefusal as E;
        let Self {
            nodes,
            cords,
            routes,
            bindings,
            roles,
        } = self;
        let mut values = k::FixedValueStore::<8, 167>::new(8 * 167).map_err(|_| E::Storage)?;
        let empty = values.store(&[]).map_err(|_| E::Storage)?;
        let drivers = core::array::from_fn(|i| {
            if i == usize::from(roles.credentials.0) {
                NetworkControlBack::credentials(empty).expect("reserved empty input")
            } else if i == usize::from(roles.join.0) {
                NetworkControlBack::join()
            } else {
                NetworkControlBack::attachment_sign()
            }
        });
        let kernel = NetworkControlKernel::new_with_host_calls(
            nodes, cords, routes, bindings, drivers, values, signs,
        )
        .map_err(|_| E::Tables)?;
        Ok(PreparedNetworkKernel { kernel, roles })
    }
}

/// Complete opaque borrowed Core scope, with identical fixed numeric lowering.
pub type NetworkControlFlatDirectTables<'a> =
    conduit_plan_lowering::local_tables::FlatDirectLocalKernelTables<'a, 3, 2, 1, 3, 2, 3>;
impl PreparedNetworkKernelParts {
    pub fn check_flat_direct(
        tables: &NetworkControlFlatDirectTables<'_>,
    ) -> Result<NetworkControlRoles, ControlKernelRefusal> {
        let view = tables.preparation_scope().view();
        check_direct_profile(
            view.placements,
            view.header.execution_regions,
            tables.cord_value_slots(),
            tables.cord_value_bytes(),
            tables.sign_items(),
            tables.sign_bytes(),
            view.placements.iter().all(|g| g.configuration.is_empty()),
        )
    }
    pub fn from_flat_direct(
        tables: NetworkControlFlatDirectTables<'_>,
    ) -> Result<Self, ControlKernelRefusal> {
        let roles = Self::check_flat_direct(&tables)?;
        let parts = tables.into_parts();
        Ok(Self {
            nodes: parts.nodes,
            cords: parts.cords,
            routes: parts.routes,
            bindings: parts.calls,
            roles,
        })
    }
}

fn check_direct_profile(
    placements: conduit_core::PlannedGearRefs<'_>,
    regions: &[conduit_core::ExecutionRegion],
    slots: u16,
    bytes: u32,
    sign_items: u16,
    sign_bytes: u32,
    configurations_empty: bool,
) -> Result<NetworkControlRoles, ControlKernelRefusal> {
    use ControlKernelRefusal as E;
    if !regions.is_empty()
        || placements.iter().map(|g| g.resources.len()).sum::<usize>() != 2
        || placements.iter().any(|g| g.host_calls.len() != 1)
    {
        return Err(E::Shape);
    }
    if slots > 8
        || bytes > 8 * 167
        || sign_items > 128
        || sign_bytes > (128 * core::mem::size_of::<k::KernelEvent>()) as u32
    {
        return Err(E::Storage);
    }
    if !configurations_empty {
        return Err(E::Shape);
    }
    let find = |kind: &str| {
        let mut nodes = placements
            .iter()
            .enumerate()
            .filter(|(_, g)| g.kind_id == kind);
        let node = nodes.next().ok_or(E::Shape)?.0;
        if nodes.next().is_some() {
            return Err(E::Shape);
        }
        Ok(k::NodeId(node as u16))
    };
    let roles = NetworkControlRoles {
        credentials: find(conduit_net::NETWORK_CREDENTIALS_OPERATION)?,
        join: find(conduit_net::NETWORK_JOIN_OPERATION)?,
        attachment_sign: find(conduit_net::NETWORK_ATTACHMENT_SIGN_OPERATION)?,
    };
    if roles.credentials == roles.join
        || roles.join == roles.attachment_sign
        || roles.credentials == roles.attachment_sign
    {
        return Err(E::Shape);
    }
    Ok(roles)
}
