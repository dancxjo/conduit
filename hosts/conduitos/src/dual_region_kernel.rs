//! One fixed production-kernel installation for two independent Plan regions.

use crate::{
    machine::KernelInterest,
    text_kernel_operations::{
        LiteralOperation, LiteralState, PlannedOperation, PresentationOperation,
        TickPresentationOperation, TimerOperation, TimerState, UpperOperation,
    },
};
use conduit_core::{ConfigurationValue, PlanFragment};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, KernelEvent, NodeId, SignSink, ValueRef,
    ValueStorage,
    scheduler::{
        FixedScheduler, HostOperationRequest, OperationDriver, SchedulerError, SchedulerStatus,
    },
};
use conduit_runtime::lowering::{LoweredPlanFragment, MAXIMUM_KERNEL_PORTS_PER_NODE};

const MAX_NODES: usize = 5;
const MAX_CORDS: usize = 3;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 3;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = 3;
const HOST_BINDING_SLOTS: usize = MAX_NODES * MAX_NODES;
const PENDING_REQUESTS: usize = 4;
const VALUE_SLOTS: usize = 10;
const VALUE_BYTES: usize = (conduit_std_catalog::MAX_TEXT_BYTES as usize) * 4;
const SIGN_CAPACITY: usize = 96;

type Driver = OperationDriver<PlannedOperation, PORTS>;
type Scheduler = FixedScheduler<
    Driver,
    FixedValueStore<VALUE_SLOTS, VALUE_BYTES>,
    FixedSignLog<SIGN_CAPACITY>,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

pub struct DualRegionKernel {
    scheduler: Scheduler,
    timer_node: NodeId,
    tick_presentation_node: NodeId,
    upper_node: NodeId,
    text_presentation_node: NodeId,
}

impl DualRegionKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        validate_shape(fragment, lowered)?;
        let literal_index = placement_index(fragment, conduit_std_catalog::TEXT_LITERAL_KIND)?;
        let upper_index = placement_index(fragment, conduit_std_catalog::TEXT_UPPER_KIND)?;
        let text_presentation_index =
            placement_index(fragment, conduit_std_catalog::TEXT_PRESENTATION_KIND)?;
        let timer_index = placement_index(fragment, conduit_std_catalog::TICK_KIND)?;
        let tick_presentation_index =
            placement_index(fragment, conduit_std_catalog::TICK_PRESENTATION_KIND)?;

        let literal = configured_text(&fragment.placements[literal_index].configuration, "value")?;
        let period_ms =
            configured_u64(&fragment.placements[timer_index].configuration, "period-ms")?;
        let mut values = FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32)?;
        let literal_value = values.store(literal.as_bytes())?;
        let wait = values.store(&period_ms.to_le_bytes())?;
        let tick = values.store(&0_u64.to_le_bytes())?;

        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let cords = lowered
            .cords
            .iter()
            .map(|cord| cord.spec)
            .collect::<alloc::vec::Vec<_>>()
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
        for route in &lowered.routes {
            routes.install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )?;
        }
        routes.seal()?;
        let mut bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(MAX_NODES as u16);
        for operation in &lowered.host_operations {
            bindings.install(operation.node, operation.binding)?;
        }
        bindings.seal()?;

        let mut drivers = [const { None }; MAX_NODES];
        drivers[literal_index] = Some(OperationDriver::new(PlannedOperation::Literal(
            LiteralOperation {
                text: literal_value,
                state: LiteralState::Emitting,
            },
        ))?);
        drivers[upper_index] = Some(OperationDriver::new(PlannedOperation::Upper(
            UpperOperation {
                pending: false,
                emitted: false,
            },
        ))?);
        drivers[text_presentation_index] = Some(OperationDriver::new(
            PlannedOperation::Presentation(PresentationOperation {
                pending: false,
                complete: false,
            }),
        )?);
        drivers[timer_index] = Some(OperationDriver::new(PlannedOperation::Timer(
            TimerOperation {
                wait: BoundedValueRef::new(wait, 8)?,
                tick,
                state: TimerState::Waiting,
            },
        ))?);
        drivers[tick_presentation_index] = Some(OperationDriver::new(
            PlannedOperation::TickPresentation(TickPresentationOperation {
                pending: false,
                complete: false,
            }),
        )?);
        let drivers = drivers
            .into_iter()
            .collect::<Option<alloc::vec::Vec<_>>>()
            .ok_or(SchedulerError::InvalidPlan)?
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let minimum_sign_bytes = (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGN_CAPACITY>::new(lowered.sign_bytes.max(minimum_sign_bytes))?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_operations(
                nodes, cords, routes, bindings, drivers, values, signs,
            )?,
            timer_node: NodeId(timer_index as u16),
            tick_presentation_node: NodeId(tick_presentation_index as u16),
            upper_node: NodeId(upper_index as u16),
            text_presentation_node: NodeId(text_presentation_index as u16),
        })
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        self.scheduler.step()
    }

    pub fn next_host_request(&mut self) -> Option<HostOperationRequest> {
        self.scheduler.next_host_request()
    }

    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        self.scheduler.host_value(value)
    }

    pub fn is_timer_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.timer_node && request.operation == conduit_kernel::HostOperationId(0)
    }

    pub fn timer_interest(
        &self,
        request: HostOperationRequest,
    ) -> Result<KernelInterest, SchedulerError> {
        if !self.is_timer_request(&request) {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        Ok(KernelInterest {
            node: request.node,
            request: request.request,
            input: request.input,
        })
    }

    pub fn complete_timer(&mut self, interest: KernelInterest) -> Result<(), SchedulerError> {
        self.scheduler.complete_host_operation(
            interest.node,
            interest.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
    }

    pub fn is_upper_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.upper_node && request.operation == conduit_kernel::HostOperationId(0)
    }

    pub fn complete_upper(
        &mut self,
        request: HostOperationRequest,
        output: &[u8],
    ) -> Result<(), SchedulerError> {
        if !self.is_upper_request(&request) {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        let value = self.scheduler.store_host_value(output)?;
        let output = BoundedValueRef::new(value, conduit_std_catalog::MAX_TEXT_BYTES)
            .map_err(|_| SchedulerError::InvalidHostOperationAccess)?;
        self.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(output),
                failure: None,
            },
        )
    }

    pub fn is_text_presentation_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.text_presentation_node
            && request.operation == conduit_kernel::HostOperationId(0)
    }

    pub fn is_tick_presentation_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.tick_presentation_node
            && request.operation == conduit_kernel::HostOperationId(0)
    }

    pub fn complete_presentation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<(), SchedulerError> {
        if !self.is_text_presentation_request(&request)
            && !self.is_tick_presentation_request(&request)
        {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        self.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
    }

    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        self.scheduler.cancel()
    }

    pub fn decisions(&self) -> u32 {
        self.scheduler.decisions()
    }

    pub fn sign_count(&self) -> u16 {
        self.scheduler.signs().len()
    }

    pub fn pending_host_operations(&self) -> usize {
        self.scheduler.pending_host_operation_count()
    }
}

fn placement_index(fragment: &PlanFragment, kind: &str) -> Result<usize, SchedulerError> {
    fragment
        .placements
        .iter()
        .position(|placement| placement.kind_id.as_str() == kind)
        .ok_or(SchedulerError::InvalidPlan)
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

fn configured_text<'a>(
    entries: &'a [conduit_core::ConfigurationEntry],
    key: &str,
) -> Result<&'a str, SchedulerError> {
    entries
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, ConfigurationValue::Text(value)) if candidate == key => {
                Some(value.as_str())
            }
            _ => None,
        })
        .filter(|value| value.len() <= conduit_std_catalog::MAX_TEXT_BYTES as usize)
        .ok_or(SchedulerError::InvalidPlan)
}

fn validate_shape(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<(), SchedulerError> {
    if fragment.placements.len() != MAX_NODES
        || fragment.connections.len() != MAX_CORDS
        || fragment.execution_regions.len() != 2
        || lowered.nodes.len() != MAX_NODES
        || lowered.cords.len() != MAX_CORDS
        || lowered.routes.len() != MAX_CORDS
        || lowered.host_operations.len() != 4
        || lowered.cord_value_slots != MAX_CORDS as u16
        || lowered.cord_value_bytes != 64 * MAX_CORDS as u32
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
}
