//! The ordinary literal → upper → Face scheduler, with its original capacities.
//! Root checks semantic Plan/configuration identities before numerical admission.
pub mod backs;
use conduit_kernel::{
    scheduler::{CordSpec, NodeSpec, SchedulerError},
    *,
};
pub const NODES: usize = 3;
pub const CORDS: usize = 2;
pub const ROUTES: usize = NODES * crate::PORTS;
pub const MAX_VALUE_BYTES: usize = 256;
pub const SIGN_ITEMS: usize = 64;
pub type Scheduler = conduit_kernel::scheduler::FixedScheduler<
    backs::PlannedBack,
    FixedValueStore<6, { MAX_VALUE_BYTES * 3 }>,
    FixedSignLog<SIGN_ITEMS>,
    NODES,
    CORDS,
    { crate::PORTS },
    2,
    ROUTES,
    2,
    9,
    3,
>;
pub struct Admission {
    pub nodes: [NodeSpec<{ crate::PORTS }>; NODES],
    pub cords: [CordSpec; CORDS],
    pub ranges: [Option<RouteRange>; ROUTES],
    pub targets: [Option<RouteTarget>; CORDS],
    pub bindings: [Option<HostCallBinding>; NODES],
    pub literal_node: NodeId,
    pub upper_node: NodeId,
    pub face_node: NodeId,
    pub literal: [u8; MAX_VALUE_BYTES],
    pub literal_bytes: u16,
    pub sign_bytes: u32,
}
impl Admission {
    pub fn new(literal_node: NodeId, upper_node: NodeId, face_node: NodeId) -> Self {
        Self {
            nodes: [NodeSpec {
                input_cords: [None; crate::PORTS],
                maximum_step_fuel: 1,
            }; NODES],
            cords: [CordSpec::inactive(); CORDS],
            ranges: [None; ROUTES],
            targets: [None; CORDS],
            bindings: [None; NODES],
            literal_node,
            upper_node,
            face_node,
            literal: [0; MAX_VALUE_BYTES],
            literal_bytes: 0,
            sign_bytes: 0,
        }
    }
    pub fn build(self) -> Result<Scheduler, SchedulerError> {
        let indices = [
            self.literal_node.0 as usize,
            self.upper_node.0 as usize,
            self.face_node.0 as usize,
        ];
        if indices.iter().any(|n| *n >= NODES)
            || indices[0] == indices[1]
            || indices[0] == indices[2]
            || indices[1] == indices[2]
            || self.literal_bytes as usize > MAX_VALUE_BYTES
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let literal = &self.literal[..self.literal_bytes as usize];
        core::str::from_utf8(literal).map_err(|_| SchedulerError::InvalidPlan)?;
        let mut values =
            FixedValueStore::<6, { MAX_VALUE_BYTES * 3 }>::new((MAX_VALUE_BYTES * 3) as u32)?;
        let text = values.store(literal)?;
        let mut drivers = [backs::PlannedBack::Upper(backs::UpperBack {
            pending: false,
            emitted: false,
        }); NODES];
        drivers[indices[0]] = backs::PlannedBack::Literal(backs::LiteralBack {
            text,
            state: backs::LiteralState::Emitting,
        });
        drivers[indices[1]] = backs::PlannedBack::Upper(backs::UpperBack {
            pending: false,
            emitted: false,
        });
        drivers[indices[2]] = backs::PlannedBack::Face(backs::FaceBack {
            pending: false,
            complete: false,
        });
        let mut routes = FixedRoutes::<ROUTES, CORDS>::new(crate::PORTS as u16);
        for (slot, range) in self.ranges.into_iter().enumerate() {
            let Some(range) = range else {
                continue;
            };
            let mut targets = [RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(0), PortId(0)),
            }; CORDS];
            let start = range.start as usize;
            let end = start
                .checked_add(range.len as usize)
                .ok_or(SchedulerError::InvalidPlan)?;
            let source = self
                .targets
                .get(start..end)
                .ok_or(SchedulerError::InvalidPlan)?;
            for (target, source) in targets.iter_mut().zip(source) {
                *target = source.ok_or(SchedulerError::InvalidPlan)?;
            }
            routes.install(
                NodeId((slot / crate::PORTS) as u16),
                PortId((slot % crate::PORTS) as u16),
                range,
                &targets[..source.len()],
            )?;
        }
        routes.seal()?;
        let mut bindings = FixedHostCallBindings::<9>::new(NODES as u16);
        for (node, binding) in self.bindings.into_iter().enumerate() {
            if let Some(binding) = binding {
                bindings.install(NodeId(node as u16), binding)?;
            }
        }
        bindings.seal()?;
        let signs = FixedSignLog::<SIGN_ITEMS>::new(
            self.sign_bytes
                .max((SIGN_ITEMS * core::mem::size_of::<KernelEvent>()) as u32),
        )?;
        Scheduler::new_with_host_calls(
            self.nodes, self.cords, routes, bindings, drivers, values, signs,
        )
    }
}
