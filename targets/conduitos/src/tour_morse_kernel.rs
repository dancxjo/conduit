//! Fixed-storage kernel installation for the shared explicit fan-out Tour Form.

use crate::{
    text_kernel_operations::{
        LiteralOperation, LiteralState, PresentationOperation, UpperOperation,
    },
    tour_morse_operations::{IndicatorOperation, MorseOperation, TourMorseOperation},
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
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment};

const MAX_NODES: usize = 5;
const MAX_CORDS: usize = 4;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 4;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = 4;
const HOST_BINDING_SLOTS: usize = MAX_NODES * MAX_NODES;
const PENDING_REQUESTS: usize = 4;
const VALUE_SLOTS: usize = 12;
const VALUE_BYTES: usize =
    conduit_text::MAXIMUM_MORSE_PATTERN_BYTES * 4 + conduit_text::MAX_TEXT_BYTES as usize * 4;
const SIGN_CAPACITY: usize = 96;

type Driver = OperationDriver<TourMorseOperation, PORTS>;
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

pub struct TourMorseKernel {
    scheduler: Scheduler,
    upper_node: NodeId,
    text_presentation_node: NodeId,
    morse_node: NodeId,
    indicator_node: NodeId,
}

impl TourMorseKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        expected_literal: &str,
    ) -> Result<Self, SchedulerError> {
        validate_shape(fragment, lowered, expected_literal)?;
        let literal_index = node(fragment, conduit_text::TEXT_LITERAL_KIND)?;
        let upper_index = node(fragment, conduit_text::TEXT_UPPER_KIND)?;
        let text_presentation_index =
            node(fragment, conduit_semantic_catalog::TEXT_PRESENTATION_KIND)?;
        let morse_index = node(fragment, conduit_text::TEXT_MORSE_KIND)?;
        let indicator_index = node(
            fragment,
            conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND,
        )?;
        let literal = configured_text(&fragment.placements[literal_index].configuration, "value")?;
        let mut values = FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32)?;
        let text = values.store(literal.as_bytes())?;
        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let cords = [
            lowered.cords[0].spec,
            lowered.cords[1].spec,
            lowered.cords[2].spec,
            lowered.cords[3].spec,
        ];
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
        let mut drivers: [Option<Driver>; MAX_NODES] = [None, None, None, None, None];
        drivers[literal_index] = Some(OperationDriver::new(TourMorseOperation::Literal(
            LiteralOperation {
                text,
                state: LiteralState::Emitting,
            },
        ))?);
        drivers[upper_index] = Some(OperationDriver::new(TourMorseOperation::Upper(
            UpperOperation {
                pending: false,
                emitted: false,
            },
        ))?);
        drivers[text_presentation_index] = Some(OperationDriver::new(
            TourMorseOperation::TextPresentation(PresentationOperation {
                pending: false,
                complete: false,
            }),
        )?);
        drivers[morse_index] = Some(OperationDriver::new(TourMorseOperation::Morse(
            MorseOperation {
                pending: false,
                emitted: false,
            },
        ))?);
        drivers[indicator_index] = Some(OperationDriver::new(TourMorseOperation::Indicator(
            IndicatorOperation {
                pending: false,
                complete: false,
            },
        ))?);
        let [
            Some(first),
            Some(second),
            Some(third),
            Some(fourth),
            Some(fifth),
        ] = drivers
        else {
            return Err(SchedulerError::InvalidPlan);
        };
        let minimum_sign_bytes = (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGN_CAPACITY>::new(lowered.sign_bytes.max(minimum_sign_bytes))?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_operations(
                nodes,
                cords,
                routes,
                bindings,
                [first, second, third, fourth, fifth],
                values,
                signs,
            )?,
            upper_node: NodeId(upper_index as u16),
            text_presentation_node: NodeId(text_presentation_index as u16),
            morse_node: NodeId(morse_index as u16),
            indicator_node: NodeId(indicator_index as u16),
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

    pub fn is_upper_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.upper_node
    }

    pub fn is_text_presentation_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.text_presentation_node
    }

    pub fn is_morse_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.morse_node
    }

    pub fn is_indicator_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.indicator_node
    }

    pub fn complete_upper(
        &mut self,
        request: HostOperationRequest,
        output: &[u8],
    ) -> Result<(), SchedulerError> {
        self.complete_value(
            request,
            self.upper_node,
            conduit_text::MAX_TEXT_BYTES,
            output,
        )
    }

    pub fn complete_morse(
        &mut self,
        request: HostOperationRequest,
        output: &[u8],
    ) -> Result<(), SchedulerError> {
        self.complete_value(
            request,
            self.morse_node,
            conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32,
            output,
        )
    }

    fn complete_value(
        &mut self,
        request: HostOperationRequest,
        node: NodeId,
        maximum: u32,
        output: &[u8],
    ) -> Result<(), SchedulerError> {
        if request.node != node || request.operation != conduit_kernel::HostOperationId(0) {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        let value = self.scheduler.store_host_value(output)?;
        let output = BoundedValueRef::new(value, maximum)
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

    pub fn complete_presentation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<(), SchedulerError> {
        if request.operation != conduit_kernel::HostOperationId(0)
            || (request.node != self.text_presentation_node && request.node != self.indicator_node)
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

fn node(fragment: &PlanFragment, kind: &str) -> Result<usize, SchedulerError> {
    fragment
        .placements
        .iter()
        .position(|placement| placement.kind_id.as_str() == kind)
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
        .filter(|value| value.len() <= conduit_text::MAXIMUM_MORSE_INPUT_BYTES)
        .ok_or(SchedulerError::InvalidPlan)
}

fn validate_shape(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    expected_literal: &str,
) -> Result<(), SchedulerError> {
    if fragment.placements.len() != MAX_NODES
        || fragment.connections.len() != MAX_CORDS
        || lowered.nodes.len() != MAX_NODES
        || lowered.cords.len() != MAX_CORDS
        || lowered.host_operations.len() != 4
        || !lowered.remote_endpoints.is_empty()
        || configured_text(
            &fragment.placements[node(fragment, conduit_text::TEXT_LITERAL_KIND)?].configuration,
            "value",
        )? != expected_literal
    {
        return Err(SchedulerError::InvalidPlan);
    }
    for (kind, implementation) in [
        (
            conduit_text::TEXT_LITERAL_KIND,
            crate::offer::TEXT_LITERAL_IMPLEMENTATION,
        ),
        (
            conduit_text::TEXT_UPPER_KIND,
            crate::offer::TEXT_UPPER_IMPLEMENTATION,
        ),
        (
            conduit_semantic_catalog::TEXT_PRESENTATION_KIND,
            crate::offer::TEXT_PRESENTATION_IMPLEMENTATION,
        ),
        (
            conduit_text::TEXT_MORSE_KIND,
            crate::offer::TEXT_MORSE_IMPLEMENTATION,
        ),
        (
            conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND,
            crate::offer::INDICATOR_PRESENTATION_IMPLEMENTATION,
        ),
    ] {
        if fragment.placements[node(fragment, kind)?]
            .implementation_id
            .as_str()
            != implementation
        {
            return Err(SchedulerError::InvalidPlan);
        }
    }
    Ok(())
}
