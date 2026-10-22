//! Numeric admission for the unchanged production scheduler. No semantic planning.
use crate::{backs::*, *};
use conduit_kernel::{
    scheduler::{CordSpec, NodeSpec, SchedulerError},
    *,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackKind {
    Keyboard,
    Keymap,
    Upper,
    Edit,
    Face,
    Application,
    Document,
    Editor,
}

pub struct Admission {
    pub active_nodes: u16,
    pub active_cords: u16,
    pub nodes: [NodeSpec<PORTS>; NODES],
    pub cords: [CordSpec; CORDS],
    pub backs: [BackKind; NODES],
    pub ranges: [Option<RouteRange>; ROUTES],
    pub targets: [Option<RouteTarget>; CORDS],
    pub bindings: [Option<HostCallBinding>; NODES],
}
impl Admission {
    pub fn new(active_nodes: u16, active_cords: u16) -> Self {
        Self {
            active_nodes,
            active_cords,
            nodes: [NodeSpec {
                input_cords: [None; PORTS],
                maximum_step_fuel: 1,
            }; NODES],
            cords: [CordSpec::inactive(); CORDS],
            backs: [BackKind::Upper; NODES],
            ranges: [None; ROUTES],
            targets: [None; CORDS],
            bindings: [None; NODES],
        }
    }
    pub fn install_route(
        &mut self,
        node: NodeId,
        port: PortId,
        range: RouteRange,
        targets: &[RouteTarget],
    ) -> Result<(), SchedulerError> {
        if usize::from(node.0) >= NODES
            || usize::from(port.0) >= PORTS
            || targets.len() != usize::from(range.len)
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let slot = usize::from(node.0) * PORTS + usize::from(port.0);
        let end = usize::from(range.start)
            .checked_add(targets.len())
            .ok_or(SchedulerError::InvalidPlan)?;
        let output = self
            .targets
            .get_mut(usize::from(range.start)..end)
            .ok_or(SchedulerError::InvalidPlan)?;
        if self.ranges[slot].is_some() || output.iter().any(Option::is_some) {
            return Err(SchedulerError::InvalidPlan);
        }
        for (output, value) in output.iter_mut().zip(targets) {
            *output = Some(*value);
        }
        self.ranges[slot] = Some(range);
        Ok(())
    }
    pub fn build(self) -> Result<Scheduler, SchedulerError> {
        let mut values = FixedValueStore::<10, VALUE_BYTES>::new(24_576)?;
        let empty = values.store(&[])?;
        let mut drivers = [PlannedBack::Upper(StreamTransformBack::new(false)); NODES];
        for (index, kind) in self
            .backs
            .into_iter()
            .enumerate()
            .take(usize::from(self.active_nodes))
        {
            drivers[index] = match kind {
                BackKind::Keyboard => PlannedBack::Keyboard(KeyboardBack {
                    empty,
                    pending: None,
                    next: 0,
                    maximum: None,
                }),
                BackKind::Keymap => PlannedBack::Keymap(StreamTransformBack::new(true)),
                BackKind::Upper => PlannedBack::Upper(StreamTransformBack::new(false)),
                BackKind::Edit => {
                    PlannedBack::TextEdit(StreamTransformBack::initially_empty(values.store(&[])?))
                }
                BackKind::Document => {
                    PlannedBack::Document(DocumentBack::new(values.store(&[0; 8])?))
                }
                BackKind::Editor => PlannedBack::Editor(StreamTransformBack::new(false)),
                BackKind::Face => PlannedBack::Face(FaceBack {
                    pending: None,
                    next: 0,
                }),
                BackKind::Application => PlannedBack::Application(ApplicationBack {
                    empty: values.store(&[])?,
                    pending: None,
                    next: 0,
                    initial: true,
                }),
            };
        }
        let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
        for (slot, range) in self.ranges.into_iter().enumerate() {
            let Some(range) = range else {
                continue;
            };
            let mut targets = [RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(NodeId(0), PortId(0)),
            }; CORDS];
            let start = usize::from(range.start);
            let len = usize::from(range.len);
            let source = self
                .targets
                .get(start..start.checked_add(len).ok_or(SchedulerError::InvalidPlan)?)
                .ok_or(SchedulerError::InvalidPlan)?;
            for (target, source) in targets.iter_mut().zip(source) {
                *target = source.ok_or(SchedulerError::InvalidPlan)?;
            }
            routes.install(
                NodeId((slot / PORTS) as u16),
                PortId((slot % PORTS) as u16),
                range,
                &targets[..len],
            )?;
        }
        routes.seal()?;
        let mut bindings = FixedHostCallBindings::<NODES>::new(1);
        for (node, binding) in self.bindings.into_iter().enumerate() {
            if let Some(binding) = binding {
                bindings.install(NodeId(node as u16), binding)?;
            }
        }
        bindings.seal()?;
        let signs = FixedSignLog::<SIGN_ITEMS>::new(
            (SIGN_ITEMS * core::mem::size_of::<KernelEvent>()) as u32,
        )?;
        Scheduler::new_with_active_counts_and_host_calls(
            usize::from(self.active_nodes),
            usize::from(self.active_cords),
            self.nodes,
            self.cords,
            routes,
            bindings,
            drivers,
            values,
            signs,
        )
    }
}
