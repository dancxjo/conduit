//! Root preparation for the separately reusable standing timer kernel.
use crate::machine::KernelInterest;
use alloc::vec::Vec;
use conduit_core::{ConfigurationValue, PlanFragment};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    KernelEvent, NodeId, ValueStorage,
    scheduler::{FixedScheduler, SchedulerError},
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment};
#[path = "tour_timer_runtime.rs"]
mod runtime;
pub use runtime::TourTimerKernel;
use runtime::{CORDS, HOST_BINDINGS, NODES, PORTS, SIGNS, TimerBack, VALUE_BYTES, VALUES};
const _: () = assert!(PORTS == FIXED_KERNEL_STORAGE_PORTS_PER_NODE);

impl TourTimerKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        if fragment.placements.len() != NODES
            || fragment.connections.len() != CORDS
            || lowered.nodes.len() != NODES
            || lowered.cords.len() != CORDS
            || !lowered.remote_endpoints.is_empty()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let timer = node(fragment, conduit_time::TIME_EVERY_KIND)?;
        let count = node(fragment, conduit_semantic_catalog::STATE_COUNT_KIND)?;
        let presentation = node(fragment, conduit_semantic_catalog::COUNT_PRESENTATION_KIND)?;
        validate_implementation(fragment, timer, crate::offer::TIME_EVERY_IMPLEMENTATION)?;
        validate_implementation(fragment, count, crate::offer::STATE_COUNT_IMPLEMENTATION)?;
        validate_implementation(
            fragment,
            presentation,
            crate::offer::COUNT_PRESENTATION_IMPLEMENTATION,
        )?;
        let period = configured_milliseconds(&fragment.placements[timer].configuration, "freq")?;
        let start = configured_u64(&fragment.placements[count].configuration, "start")?;
        if period != 120 || start != 0 {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut values = FixedValueStore::<VALUES, 8>::new(VALUE_BYTES as u32)?;
        let wait = values.store(&period.to_le_bytes())?;
        let next_wait = values.store(&period.to_le_bytes())?;
        let tick = values.store(&conduit_time::encode_tick(0))?;
        let zero = values.store(&start.to_le_bytes())?;
        let one = values.store(&1_u64.to_le_bytes())?;
        let mut drivers = Vec::with_capacity(NODES);
        for index in 0..NODES {
            let operation = if index == timer {
                TimerBack::Every {
                    waits: [
                        BoundedValueRef::new(wait, 8)?,
                        BoundedValueRef::new(next_wait, 8)?,
                    ],
                    tick,
                    emitting: false,
                    next_wait: 0,
                }
            } else if index == count {
                TimerBack::Count {
                    zero,
                    one,
                    initial_emitted: false,
                    bumped: false,
                }
            } else {
                TimerBack::Presentation {
                    pending: None,
                    next_request: 1,
                }
            };
            drivers.push(operation);
        }
        let drivers: [TimerBack; NODES] = drivers
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let cords = [lowered.cords[0].spec, lowered.cords[1].spec];
        let mut routes = FixedRoutes::<{ NODES * PORTS }, CORDS>::new(PORTS as u16);
        for route in &lowered.routes {
            routes.install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )?;
        }
        routes.seal()?;
        let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(NODES as u16);
        for operation in &lowered.host_calls {
            bindings.install(operation.node, operation.binding)?;
        }
        bindings.seal()?;
        let minimum_sign_bytes = (SIGNS * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGNS>::new(lowered.sign_bytes.max(minimum_sign_bytes))?;
        Ok(Self::from_scheduler(
            FixedScheduler::new_with_host_calls(
                nodes, cords, routes, bindings, drivers, values, signs,
            )?,
            NodeId(timer as u16),
            NodeId(presentation as u16),
        ))
    }

    pub fn complete_timer(&mut self, interest: KernelInterest) -> Result<(), SchedulerError> {
        self.complete_request(interest.node, interest.request)
    }
}

fn node(fragment: &PlanFragment, kind: &str) -> Result<usize, SchedulerError> {
    fragment
        .placements
        .iter()
        .position(|placement| placement.kind_id.as_str() == kind)
        .ok_or(SchedulerError::InvalidPlan)
}

fn validate_implementation(
    fragment: &PlanFragment,
    node: usize,
    implementation: &str,
) -> Result<(), SchedulerError> {
    if fragment.placements[node].implementation_id.as_str() != implementation {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
}

fn configured_u64(
    entries: &[conduit_core::ConfigurationEntry],
    key: &str,
) -> Result<u64, SchedulerError> {
    entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::U64(value)) if candidate == key => Some(*value),
            _ => None,
        })
        .ok_or(SchedulerError::InvalidPlan)
}

fn configured_milliseconds(
    entries: &[conduit_core::ConfigurationEntry],
    key: &str,
) -> Result<u64, SchedulerError> {
    entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::Quantity(value)) if candidate == key => value
                .convert(conduit_core::QuantityUnit::Millisecond)
                .ok()
                .and_then(|value| u64::try_from(value.value()).ok()),
            _ => None,
        })
        .ok_or(SchedulerError::InvalidPlan)
}
