//! Canonical Tour fan-out/Morse scheduler with its original capacities.
//! Root checks semantic Plan/configuration identities before numerical admission.
pub mod backs;
use conduit_kernel::{
    scheduler::{CordSpec, NodeSpec, SchedulerError},
    *,
};
pub const NODES: usize = 5;
pub const CORDS: usize = 4;
pub const ROUTES: usize = NODES * crate::PORTS;
pub const MAX_INPUT_BYTES: usize = 32;
pub const MAX_VALUE_BYTES: usize = 645;
pub const VALUE_STORAGE_BYTES: usize = MAX_VALUE_BYTES * 4 + crate::text::MAX_VALUE_BYTES * 4;
pub const SIGN_ITEMS: usize = 96;
pub type Scheduler = conduit_kernel::scheduler::FixedScheduler<
    backs::TourMorseBack,
    FixedValueStore<12, VALUE_STORAGE_BYTES>,
    FixedSignLog<SIGN_ITEMS>,
    NODES,
    CORDS,
    { crate::PORTS },
    4,
    ROUTES,
    4,
    25,
    4,
>;
pub struct Admission {
    pub nodes: [NodeSpec<{ crate::PORTS }>; NODES],
    pub cords: [CordSpec; CORDS],
    pub ranges: [Option<RouteRange>; ROUTES],
    pub targets: [Option<RouteTarget>; CORDS],
    pub bindings: [Option<HostCallBinding>; NODES],
    /// Literal, Upper, TextFace, Morse, Indicator in that order.
    pub roles: [NodeId; NODES],
    pub literal: [u8; MAX_INPUT_BYTES],
    pub literal_bytes: u16,
    pub sign_bytes: u32,
}
impl Admission {
    pub fn new(roles: [NodeId; NODES]) -> Self {
        Self {
            nodes: [NodeSpec {
                input_cords: [None; crate::PORTS],
                maximum_step_fuel: 1,
            }; NODES],
            cords: [CordSpec::inactive(); CORDS],
            ranges: [None; ROUTES],
            targets: [None; CORDS],
            bindings: [None; NODES],
            roles,
            literal: [0; MAX_INPUT_BYTES],
            literal_bytes: 0,
            sign_bytes: 0,
        }
    }
    pub fn build(self) -> Result<Scheduler, SchedulerError> {
        let indices = self.roles.map(|node| node.0 as usize);
        if indices.iter().any(|n| *n >= NODES)
            || indices
                .iter()
                .enumerate()
                .any(|(i, n)| indices[..i].contains(n))
            || self.literal_bytes as usize > MAX_INPUT_BYTES
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let literal = &self.literal[..self.literal_bytes as usize];
        core::str::from_utf8(literal).map_err(|_| SchedulerError::InvalidPlan)?;
        let mut values =
            FixedValueStore::<12, VALUE_STORAGE_BYTES>::new(VALUE_STORAGE_BYTES as u32)?;
        let text = values.store(literal)?;
        let mut drivers = [backs::TourMorseBack::Upper(crate::text::backs::UpperBack {
            pending: false,
            emitted: false,
        }); NODES];
        drivers[indices[0]] = backs::TourMorseBack::Literal(crate::text::backs::LiteralBack {
            text,
            state: crate::text::backs::LiteralState::Emitting,
        });
        drivers[indices[1]] = backs::TourMorseBack::Upper(crate::text::backs::UpperBack {
            pending: false,
            emitted: false,
        });
        drivers[indices[2]] = backs::TourMorseBack::TextFace(crate::text::backs::FaceBack {
            pending: false,
            complete: false,
        });
        drivers[indices[3]] = backs::TourMorseBack::Morse(backs::MorseBack {
            pending: false,
            emitted: false,
        });
        drivers[indices[4]] = backs::TourMorseBack::Indicator(backs::IndicatorBack {
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
        let mut bindings = FixedHostCallBindings::<25>::new(NODES as u16);
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
