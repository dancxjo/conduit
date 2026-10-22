//! Fixed-source assigned projection -> ordinary kernel tables, without allocation.
//! The trusted resident layout comes from Make, never the incoming assignment.
//! This preparation step neither authorizes effects nor admits evaluator storage.
use conduit_core as c;
use conduit_kernel as k;
use k::scheduler::{
    AssignedConnectionTrack as Track, AssignedPressurePolicy, CordCapacity, CordSpec, NodeSpec,
};

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
        use AssignedKernelRefusal as E;
        if !matches!(
            u16_at(bytes, 8)?,
            c::ASSIGNED_KERNEL_PLAN_SCHEMA | c::ASSIGNED_NATIVE_PLAN_SCHEMA
        ) {
            return Err(E::Unsupported);
        }
        let identity =
            c::decode_assigned_plan_with_computations(bytes, maxima, required, computations)?;
        if N == 0
            || N > usize::from(u16::MAX)
            || P == 0
            || P > usize::from(u16::MAX)
            || identity.counts != layout.counts
            || usize::from(identity.counts[0]) != N
            || usize::from(identity.counts[3]) != C
        {
            return Err(E::Layout);
        }
        if identity.counts[9] != 0 || identity.counts[12] != 0 {
            return Err(E::Unsupported);
        }
        validate_layout(bytes, layout)?;
        let mut nodes = [NodeSpec {
            input_cords: [None; P],
            maximum_step_fuel: 0,
        }; N];
        let mut node_seen = [false; N];
        let mut cords = [CordSpec::inactive(); C];
        let mut cord_seen = [false; C];
        let mut ranges = [None; R];
        let mut range_count = 0;
        let empty_target = k::RouteTarget {
            cord: k::CordId(u16::MAX),
            sink: k::CordEndpoint::local(k::NodeId(u16::MAX), k::PortId(u16::MAX)),
        };
        let mut targets = [empty_target; T];
        let mut target_count = 0;
        let mut calls = k::FixedHostCallBindings::<H>::new(1);
        let mut startup_order = [k::NodeId(u16::MAX); N];
        let mut startup_positions = [usize::MAX; N];
        let mut startup_count = 0;
        for record in c::AssignedPlanRecords::new(bytes)? {
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
                    let source = endpoint::<N, P>(data, 18)?;
                    let sink = endpoint::<N, P>(data, 23)?;
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
                    let k::CordEndpoint::Local { node, port } = sink else {
                        return Err(E::Unsupported);
                    };
                    if nodes[usize::from(node.0)].input_cords[usize::from(port.0)]
                        .replace(spec.cord)
                        .is_some()
                    {
                        return Err(E::Layout);
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
                    if data.len() != 6 {
                        return Err(E::Unsupported);
                    }
                    let destination = targets.get_mut(target_count).ok_or(E::Capacity)?;
                    *destination = k::RouteTarget {
                        cord: k::CordId(u16_at(data, 0)?),
                        sink: k::CordEndpoint::local(
                            k::NodeId(u16_at(data, 2)?),
                            k::PortId(u16_at(data, 4)?),
                        ),
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
        if node_seen.contains(&false) || cord_seen.contains(&false) {
            return Err(E::Layout);
        }
        if startup_count != N {
            return Err(E::Layout);
        }
        for record in c::AssignedPlanRecords::new(bytes)? {
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
        let mut routes = k::FixedRoutes::<R, T>::new(P as u16);
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
        Ok(Self {
            identity,
            nodes,
            cords,
            routes,
            calls,
            startup_order,
        })
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
fn validate_layout(bytes: &[u8], layout: &ResidentLayout<'_>) -> Result<(), AssignedKernelRefusal> {
    use AssignedKernelRefusal as E;
    if layout.records.len() > 256 {
        return Err(E::Capacity);
    }
    let mut seen = [false; 256];
    for record in c::AssignedPlanRecords::new(bytes)? {
        let record = record?;
        if matches!(
            record.tag,
            c::ASSIGNED_CONFIGURATION | c::ASSIGNED_SIGN | c::ASSIGNED_TERMINAL
        ) {
            continue;
        }
        let expected = c::AssignedRecordRequirement::from_kernel_record(record, u16_at(bytes, 8)?);
        let index = layout
            .records
            .iter()
            .enumerate()
            .position(|(i, r)| !seen[i] && *r == expected)
            .ok_or(E::Layout)?;
        seen[index] = true;
    }
    if seen[..layout.records.len()].contains(&false) {
        return Err(E::Layout);
    }
    Ok(())
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
fn endpoint<const N: usize, const P: usize>(
    data: &[u8],
    offset: usize,
) -> Result<k::CordEndpoint, AssignedKernelRefusal> {
    if data[offset] != 0 {
        return Err(AssignedKernelRefusal::Unsupported);
    }
    let node = u16_at(data, offset + 1)?;
    let port = u16_at(data, offset + 3)?;
    if usize::from(node) >= N || usize::from(port) >= P {
        return Err(AssignedKernelRefusal::Capacity);
    }
    Ok(k::CordEndpoint::local(k::NodeId(node), k::PortId(port)))
}
