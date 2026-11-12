//! Fixed-source assigned projection -> ordinary kernel tables, without allocation.
//! The trusted resident layout comes from Make, never the incoming assignment.
//! This preparation step neither authorizes effects nor admits evaluator storage.
use crate as k;
use conduit_assigned_plan as c;
mod boundary;
mod line_fore;
mod local_fore;
use line_fore::ForeProfile;
#[path = "assigned/layout.rs"]
mod layout_check;
pub(crate) use local_fore::LocalForeProvenance;
mod bounded;
pub use bounded::BoundedAssignedKernelTables;
mod record_input;
mod write_records;
use boundary::endpoint;
use k::scheduler::{
    AssignedConnectionTrack as Track, AssignedPressurePolicy, CordCapacity, CordSpec, NodeSpec,
};
use record_input::RecordInput;

pub struct ResidentLayout<'a> {
    pub records: &'a [c::AssignedRecordRequirement],
    pub counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
}
#[derive(Debug, PartialEq, Eq)]
pub enum AssignedKernelRefusal {
    Projection(c::AssignedPlanRefusal),
    Layout,
    Capacity,
    Unsupported,
    Kernel(k::ProtocolError),
}
impl From<c::AssignedPlanRefusal> for AssignedKernelRefusal {
    fn from(value: c::AssignedPlanRefusal) -> Self {
        Self::Projection(value)
    }
}
impl From<k::ProtocolError> for AssignedKernelRefusal {
    fn from(value: k::ProtocolError) -> Self {
        Self::Kernel(value)
    }
}

pub struct AssignedKernelTables<
    const N: usize,
    const C: usize,
    const P: usize,
    const R: usize,
    const T: usize,
    const H: usize,
> {
    pub identity: c::AssignedPlanView,
    pub nodes: [NodeSpec<P>; N],
    pub cords: [CordSpec; C],
    pub routes: k::FixedRoutes<R, T>,
    pub calls: k::FixedHostCallBindings<H>,
    startup_order: [k::NodeId; N],
}
impl<
        const N: usize,
        const C: usize,
        const P: usize,
        const R: usize,
        const T: usize,
        const H: usize,
    > AssignedKernelTables<N, C, P, R, T, H>
{
    /// Exact resident structure is checked before producing any executable tables.
    /// This local profile refuses remote endpoints and validates startup dependencies.
    pub fn prepare(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
    ) -> Result<Self, AssignedKernelRefusal> {
        Self::prepare_input(
            RecordInput::Bytes(bytes),
            maxima,
            required,
            computations,
            layout,
        )
    }
    /// Admit borrowed canonical records through the same validator and table
    /// builder. The layout and computation inventory remain independently trusted.
    pub fn prepare_records(
        header: &[u8],
        records: &[c::AssignedRecord<'_>],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
    ) -> Result<Self, AssignedKernelRefusal> {
        Self::prepare_input(
            RecordInput::Records { header, records },
            maxima,
            required,
            computations,
            layout,
        )
    }
    fn prepare_input(
        input: RecordInput<'_>,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
    ) -> Result<Self, AssignedKernelRefusal> {
        Self::prepare_profile(input, maxima, required, computations, layout, None)
    }
    /// Explicit local fore admission over a contiguous canonical projection.
    /// Root retains typed value contracts and separately authorizes submissions.
    pub fn prepare_with_fore(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        fore: &[c::AssignedForeRequirement],
    ) -> Result<Self, AssignedKernelRefusal> {
        Self::prepare_profile(
            RecordInput::Bytes(bytes),
            maxima,
            required,
            computations,
            layout,
            Some(ForeProfile::Local(fore)),
        )
    }
    /// The same exact local fore profile over borrowed canonical records.
    pub fn prepare_records_with_fore(
        header: &[u8],
        records: &[c::AssignedRecord<'_>],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        fore: &[c::AssignedForeRequirement],
    ) -> Result<Self, AssignedKernelRefusal> {
        Self::prepare_profile(
            RecordInput::Records { header, records },
            maxima,
            required,
            computations,
            layout,
            Some(ForeProfile::Local(fore)),
        )
    }
    fn prepare_profile(
        input: RecordInput<'_>,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        fore: Option<ForeProfile<'_>>,
    ) -> Result<Self, AssignedKernelRefusal> {
        Self::prepare_capacity_profile(input, maxima, required, computations, layout, fore, true)
    }
    #[allow(clippy::too_many_arguments)]
    fn prepare_capacity_profile(
        input: RecordInput<'_>,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        fore: Option<ForeProfile<'_>>,
        exact_counts: bool,
    ) -> Result<Self, AssignedKernelRefusal> {
        let mut nodes = [NodeSpec {
            input_cords: [None; P],
            maximum_step_fuel: 0,
        }; N];
        let mut cords = [CordSpec::inactive(); C];
        let mut routes = k::FixedRoutes::new(P as u16);
        let mut calls = k::FixedHostCallBindings::new(1);
        let mut startup_order = [k::NodeId(u16::MAX); N];
        let identity = Self::write_capacity_profile(
            input,
            maxima,
            required,
            computations,
            layout,
            fore,
            exact_counts,
            &mut nodes,
            &mut cords,
            &mut routes,
            &mut calls,
            &mut startup_order,
        )?;
        Ok(Self {
            identity,
            nodes,
            cords,
            routes,
            calls,
            startup_order,
        })
    }
    #[allow(clippy::too_many_arguments)]
    fn write_capacity_profile(
        input: RecordInput<'_>,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        computations: c::AssignedComputationProfile<'_>,
        layout: &ResidentLayout<'_>,
        fore: Option<ForeProfile<'_>>,
        exact_counts: bool,
        nodes: &mut [NodeSpec<P>; N],
        cords: &mut [CordSpec; C],
        routes: &mut k::FixedRoutes<R, T>,
        calls: &mut k::FixedHostCallBindings<H>,
        startup_order: &mut [k::NodeId; N],
    ) -> Result<c::AssignedPlanView, AssignedKernelRefusal> {
        use AssignedKernelRefusal as E;
        let schema = u16_at(input.header(), 8)?;
        if schema == c::ASSIGNED_LINE_FORE_NATIVE_PLAN_SCHEMA {
            if !matches!(fore, Some(ForeProfile::Line(_))) {
                return Err(E::Unsupported);
            }
        } else if !matches!(
            schema,
            c::ASSIGNED_KERNEL_PLAN_SCHEMA
                | c::ASSIGNED_NATIVE_PLAN_SCHEMA
                | c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA
        ) {
            return Err(E::Unsupported);
        }
        let identity = input.decode(maxima, required, computations, fore)?;
        let active_nodes = usize::from(identity.counts[0]);
        let active_cords = usize::from(identity.counts[3]);
        if N == 0
            || N > usize::from(u16::MAX)
            || P == 0
            || P > usize::from(u16::MAX)
            || identity.counts != layout.counts
            || active_nodes == 0
            || active_nodes > N
            || active_cords > C
            || (exact_counts && (active_nodes != N || active_cords != C))
        {
            return Err(E::Layout);
        }
        if usize::from(identity.counts[9]) != fore.map_or(0, ForeProfile::len)
            || identity.counts[12] != 0
        {
            return Err(E::Unsupported);
        }
        layout_check::validate(input, layout, fore)?;
        nodes.fill(NodeSpec {
            input_cords: [None; P],
            maximum_step_fuel: 0,
        });
        let mut node_seen = [false; N];
        cords.fill(CordSpec::inactive());
        let mut cord_seen = [false; C];
        let mut ranges = [None; R];
        let mut range_count = 0;
        let empty_target = k::RouteTarget {
            cord: k::CordId(u16::MAX),
            sink: k::CordEndpoint::local(k::NodeId(u16::MAX), k::PortId(u16::MAX)),
        };
        let mut targets = [empty_target; T];
        let mut target_count = 0;
        // The assigned records are already Core-checked above. Preserve the
        // existing stride-one representation for zero/single-call images; a
        // multi-operation node selects its actual maximum call ID plus one.
        // This never enlarges the caller's funded H-slot table, including sparse
        // IDs and partial node rows: install still checks each actual slot.
        let mut calls_per_node = 1u16;
        for record in input.records()? {
            let record = record?;
            if record.tag == c::ASSIGNED_HOST_CALL {
                let stride = u16_at(record.payload, 2)?
                    .checked_add(1)
                    .ok_or(E::Capacity)?;
                calls_per_node = calls_per_node.max(stride);
                if usize::from(calls_per_node) > H {
                    return Err(E::Capacity);
                }
            }
        }
        *calls = k::FixedHostCallBindings::<H>::new(calls_per_node);
        startup_order.fill(k::NodeId(u16::MAX));
        let mut startup_positions = [usize::MAX; N];
        let mut startup_count = 0;
        for record in input.records()? {
            let record = record?;
            let data = record.payload;
            match record.tag {
                c::ASSIGNED_NODE => {
                    let node = usize::from(u16_at(data, 0)?);
                    let fuel = u16_at(data, 2)?;
                    let seen = node_seen.get_mut(node).ok_or(E::Capacity)?;
                    if *seen || fuel == 0 {
                        return Err(E::Layout);
                    }
                    *seen = true;
                    nodes[node].maximum_step_fuel = fuel;
                }
                c::ASSIGNED_CORD => {
                    let cord = usize::from(u16_at(data, 0)?);
                    let seen = cord_seen.get_mut(cord).ok_or(E::Capacity)?;
                    if *seen {
                        return Err(E::Layout);
                    }
                    *seen = true;
                    let source = endpoint::<N, P>(data, 18, fore, cord as u16, true)?;
                    let sink = endpoint::<N, P>(data, 23, fore, cord as u16, false)?;
                    let byte_capacity = u32_at(data, 32)?;
                    let maximum_value_bytes = u32_at(data, 37)?;
                    if maximum_value_bytes > byte_capacity {
                        return Err(E::Layout);
                    }
                    let mut spec = CordSpec::new(
                        k::CordId(cord as u16),
                        source,
                        sink,
                        CordCapacity {
                            slot_start: u16_at(data, 28)?,
                            item_capacity: u16_at(data, 30)?,
                            byte_capacity,
                            pressure_policy: match data[41] {
                                0 => AssignedPressurePolicy::PreserveOrder,
                                1 => AssignedPressurePolicy::CoalesceLatest,
                                _ => return Err(E::Unsupported),
                            },
                        },
                    );
                    spec.maximum_value_bytes = maximum_value_bytes;
                    spec.track = match data[36] {
                        0 => Track::Payload,
                        1 => Track::NormalClose,
                        2 => Track::AbnormalTerminal,
                        3 => Track::Quiescence,
                        _ => return Err(E::Unsupported),
                    };
                    if let k::CordEndpoint::Local { node, port } = sink {
                        if nodes[usize::from(node.0)].input_cords[usize::from(port.0)]
                            .replace(spec.cord)
                            .is_some()
                        {
                            return Err(E::Layout);
                        }
                    }
                    cords[cord] = spec;
                }
                c::ASSIGNED_ROUTE => {
                    let destination = ranges.get_mut(range_count).ok_or(E::Capacity)?;
                    *destination = Some((
                        k::NodeId(u16_at(data, 0)?),
                        k::PortId(u16_at(data, 2)?),
                        k::RouteRange {
                            start: u16_at(data, 4)?,
                            len: u16_at(data, 6)?,
                        },
                    ));
                    range_count += 1;
                }
                c::ASSIGNED_ROUTE_TARGET => {
                    let destination = targets.get_mut(target_count).ok_or(E::Capacity)?;
                    *destination = k::RouteTarget {
                        cord: k::CordId(u16_at(data, 0)?),
                        sink: boundary::route_sink(data, fore)?,
                    };
                    target_count += 1;
                }
                c::ASSIGNED_HOST_CALL => {
                    if u16_at(data, 36)? != 1 {
                        return Err(E::Unsupported);
                    }
                    calls.install(
                        k::NodeId(u16_at(data, 0)?),
                        k::HostCallBinding {
                            call: k::HostCallId(u16_at(data, 2)?),
                            maximum_input_bytes: u32_at(data, 38)?,
                            maximum_output_bytes: u32_at(data, 42)?,
                        },
                    )?;
                }
                c::ASSIGNED_STARTUP if data[0] == 1 => {
                    let node = u16_at(data, 1)?;
                    let position = startup_positions
                        .get_mut(usize::from(node))
                        .ok_or(E::Capacity)?;
                    if *position != usize::MAX {
                        return Err(E::Layout);
                    }
                    *position = startup_count;
                    *startup_order.get_mut(startup_count).ok_or(E::Capacity)? = k::NodeId(node);
                    startup_count += 1;
                }
                _ => {}
            }
        }
        if node_seen[..active_nodes].contains(&false) || cord_seen[..active_cords].contains(&false)
        {
            return Err(E::Layout);
        }
        if startup_count != active_nodes {
            return Err(E::Layout);
        }
        for record in input.records()? {
            let record = record?;
            if record.tag == c::ASSIGNED_STARTUP && record.payload[0] == 0 {
                let before = usize::from(u16_at(record.payload, 1)?);
                let after = usize::from(u16_at(record.payload, 3)?);
                let before = *startup_positions.get(before).ok_or(E::Capacity)?;
                let after = *startup_positions.get(after).ok_or(E::Capacity)?;
                if before >= after {
                    return Err(E::Layout);
                }
            }
        }
        *routes = k::FixedRoutes::<R, T>::new(P as u16);
        for (node, port, range) in ranges.into_iter().flatten() {
            let start = usize::from(range.start);
            let end = start
                .checked_add(usize::from(range.len))
                .ok_or(E::Capacity)?;
            let slice = targets
                .get(start..end)
                .filter(|_| end <= target_count)
                .ok_or(E::Capacity)?;
            routes.install(node, port, range, slice)?;
        }
        routes.seal()?;
        calls.seal()?;
        if let Some(profile) = fore {
            profile.pin(cords)?;
        }
        Ok(identity)
    }
    /// Prepare existing production Backs in the exact dependency-respecting
    /// assigned order. No Play may start until this succeeds for every node.
    pub fn prepare_backs<B, E>(
        &self,
        mut prepare: impl FnMut(k::NodeId) -> Result<B, E>,
    ) -> Result<[B; N], E> {
        let mut backs: [Option<B>; N] = core::array::from_fn(|_| None);
        for node in self.startup_order {
            backs[usize::from(node.0)] = Some(prepare(node)?);
        }
        Ok(backs.map(|back| back.expect("validated complete startup order")))
    }
    pub fn startup_order(&self) -> &[k::NodeId; N] {
        &self.startup_order
    }
}

fn u16_at(data: &[u8], offset: usize) -> Result<u16, AssignedKernelRefusal> {
    Ok(u16::from_le_bytes(
        data.get(offset..offset + 2)
            .ok_or(AssignedKernelRefusal::Layout)?
            .try_into()
            .map_err(|_| AssignedKernelRefusal::Layout)?,
    ))
}
fn u32_at(data: &[u8], offset: usize) -> Result<u32, AssignedKernelRefusal> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or(AssignedKernelRefusal::Layout)?
            .try_into()
            .map_err(|_| AssignedKernelRefusal::Layout)?,
    ))
}
