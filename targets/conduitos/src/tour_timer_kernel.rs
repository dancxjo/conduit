//! Root preparation for the separately reusable standing timer kernel.
use crate::machine::KernelInterest;
use conduit_core::{ConfigurationValue, PlanFragment};
use conduit_kernel::{NodeId, scheduler::SchedulerError};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment};
#[path = "tour_timer_runtime.rs"]
pub(crate) mod runtime;
pub use runtime::TourTimerKernel;
use runtime::{CORDS, NODES, PORTS, PreparedTimerGraph, PreparedTimerRoute};
const _: () = assert!(PORTS == FIXED_KERNEL_STORAGE_PORTS_PER_NODE);

impl TourTimerKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        Self::from_prepared_graph(Self::prepare_graph(fragment, lowered)?)
    }

    pub(crate) fn prepare_graph(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<PreparedTimerGraph, SchedulerError> {
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
        let mut routes = [None; CORDS];
        if lowered.routes.len() > routes.len() || lowered.host_calls.len() > NODES {
            return Err(SchedulerError::InvalidPlan);
        }
        for (slot, route) in routes.iter_mut().zip(&lowered.routes) {
            let [target] = route.targets.as_slice() else {
                return Err(SchedulerError::InvalidPlan);
            };
            *slot = Some(PreparedTimerRoute {
                node: route.source_node,
                port: route.source_port,
                range: route.range,
                target: *target,
            });
        }
        let mut bindings = [None; NODES];
        for (slot, operation) in bindings.iter_mut().zip(&lowered.host_calls) {
            *slot = Some((operation.node, operation.binding));
        }
        Ok(PreparedTimerGraph {
            nodes: lowered
                .node_specs
                .as_slice()
                .try_into()
                .map_err(|_| SchedulerError::InvalidPlan)?,
            cords: [lowered.cords[0].spec, lowered.cords[1].spec],
            routes,
            bindings,
            timer: NodeId(timer as u16),
            count: NodeId(count as u16),
            presentation: NodeId(presentation as u16),
            period,
            start,
            sign_bytes: lowered.sign_bytes,
        })
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
