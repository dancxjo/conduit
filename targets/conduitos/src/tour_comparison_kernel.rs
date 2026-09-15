//! Fixed-storage execution kernels for both realizations of the Tour comparison.

use alloc::{boxed::Box, vec::Vec};
use conduit_core::{ConfigurationValue, PlanFragment};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, KernelEvent, SignSink, ValueRef, ValueStorage,
    scheduler::{
        FixedScheduler, HostOperationRequest, OperationDriver, SchedulerError, SchedulerStatus,
    },
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment};

use crate::{
    text_kernel_operations::{LiteralOperation, LiteralState},
    tour_morse_operations::{
        IndicatorOperation, LeafOperation, MorseOperation, TourMorseOperation,
    },
};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 6;
const ROUTE_SLOTS: usize = 7 * PORTS;
const ROUTE_TARGETS: usize = 6;
const HOST_BINDING_SLOTS: usize = 49;
const PENDING_REQUESTS: usize = 6;
const VALUE_SLOTS: usize = 16;
const MAX_VALUE_BYTES: usize = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES;
const VALUE_BUDGET: usize = MAX_VALUE_BYTES * 8;
const SIGN_CAPACITY: usize = 80;

type Driver = OperationDriver<TourMorseOperation, PORTS>;
type Scheduler<const N: usize, const C: usize> = FixedScheduler<
    Driver,
    FixedValueStore<VALUE_SLOTS, MAX_VALUE_BYTES>,
    FixedSignLog<SIGN_CAPACITY>,
    N,
    C,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ComparisonNodeKind {
    Literal,
    DirectMorse,
    Characters,
    Lookup,
    Intersperse,
    Flatten,
    SymbolsToPattern,
    Indicator,
}

pub enum ComparisonKernel {
    Direct(Box<BoxedKernel<3, 2>>),
    Recursive(Box<BoxedKernel<7, 6>>),
}

pub struct BoxedKernel<const N: usize, const C: usize> {
    scheduler: Scheduler<N, C>,
    kinds: [ComparisonNodeKind; N],
}

impl ComparisonKernel {
    pub fn prepare_direct(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        BoxedKernel::prepare(fragment, lowered).map(|kernel| Self::Direct(Box::new(kernel)))
    }

    pub fn prepare_recursive(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        BoxedKernel::prepare(fragment, lowered).map(|kernel| Self::Recursive(Box::new(kernel)))
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        delegate!(self, kernel => kernel.scheduler.step())
    }

    pub fn next_host_request(&mut self) -> Option<HostOperationRequest> {
        delegate!(self, kernel => kernel.scheduler.next_host_request())
    }

    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        delegate!(self, kernel => kernel.scheduler.host_value(value))
    }

    pub fn request_kind(&self, request: &HostOperationRequest) -> ComparisonNodeKind {
        delegate!(self, kernel => kernel.kinds[usize::from(request.node.0)])
    }

    pub fn complete_value(
        &mut self,
        request: HostOperationRequest,
        output: &[u8],
        maximum: u32,
    ) -> Result<(), SchedulerError> {
        delegate!(self, kernel => {
            let value = kernel.scheduler.store_host_value(output)?;
            let output = BoundedValueRef::new(value, maximum)
                .map_err(|_| SchedulerError::InvalidHostOperationAccess)?;
            kernel.scheduler.complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )
        })
    }

    pub fn complete_presentation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<(), SchedulerError> {
        delegate!(self, kernel => kernel.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        ))
    }

    pub fn decisions(&self) -> u32 {
        delegate!(self, kernel => kernel.scheduler.decisions())
    }

    pub fn sign_count(&self) -> u16 {
        delegate!(self, kernel => kernel.scheduler.signs().len())
    }

    pub fn pending_host_operations(&self) -> usize {
        delegate!(self, kernel => kernel.scheduler.pending_host_operation_count())
    }
}

macro_rules! delegate {
    ($value:expr, $binding:ident => $body:expr) => {
        match $value {
            Self::Direct($binding) => $body,
            Self::Recursive($binding) => $body,
        }
    };
}
use delegate;

impl<const N: usize, const C: usize> BoxedKernel<N, C> {
    fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        if fragment.placements.len() != N
            || fragment.connections.len() != C
            || lowered.nodes.len() != N
            || lowered.cords.len() != C
            || !lowered.remote_endpoints.is_empty()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut values = FixedValueStore::<VALUE_SLOTS, MAX_VALUE_BYTES>::new(VALUE_BUDGET as u32)?;
        let mut drivers = Vec::with_capacity(N);
        let mut kinds = Vec::with_capacity(N);
        for placement in &fragment.placements {
            let (kind, operation) = operation(placement, &mut values)?;
            kinds.push(kind);
            drivers.push(OperationDriver::new(operation)?);
        }
        let drivers: [Driver; N] = drivers
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let kinds: [ComparisonNodeKind; N] =
            kinds.try_into().map_err(|_| SchedulerError::InvalidPlan)?;
        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let cords = lowered
            .cords
            .iter()
            .map(|cord| cord.spec)
            .collect::<Vec<_>>()
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
        let mut bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(N as u16);
        for host_operation in &lowered.host_operations {
            bindings.install(host_operation.node, host_operation.binding)?;
        }
        bindings.seal()?;
        let minimum_sign_bytes = (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGN_CAPACITY>::new(lowered.sign_bytes.max(minimum_sign_bytes))?;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_operations(
                nodes, cords, routes, bindings, drivers, values, signs,
            )?,
            kinds,
        })
    }
}

fn operation(
    placement: &conduit_core::PlannedGear,
    values: &mut FixedValueStore<VALUE_SLOTS, MAX_VALUE_BYTES>,
) -> Result<(ComparisonNodeKind, TourMorseOperation), SchedulerError> {
    let (kind, expected_implementation, operation) = match placement.kind_id.as_str() {
        conduit_text::TEXT_LITERAL_KIND => {
            let literal = configured_text(&placement.configuration, "value")?;
            let text = values.store(literal.as_bytes())?;
            (
                ComparisonNodeKind::Literal,
                crate::offer::TEXT_LITERAL_IMPLEMENTATION,
                TourMorseOperation::Literal(LiteralOperation {
                    text,
                    state: LiteralState::Emitting,
                }),
            )
        }
        conduit_text::TEXT_MORSE_KIND => leaf(
            ComparisonNodeKind::DirectMorse,
            crate::offer::TEXT_MORSE_IMPLEMENTATION,
            conduit_text::MAXIMUM_MORSE_INPUT_BYTES as u32,
            true,
        ),
        conduit_text::TEXT_CHARACTERS_KIND => leaf(
            ComparisonNodeKind::Characters,
            crate::offer::TEXT_CHARACTERS_IMPLEMENTATION,
            conduit_text::MAX_TEXT_BYTES,
            false,
        ),
        conduit_text::MORSE_LOOKUP_KIND => leaf(
            ComparisonNodeKind::Lookup,
            crate::offer::MORSE_LOOKUP_IMPLEMENTATION,
            conduit_text::MAXIMUM_MORSE_CHARACTERS_BYTES as u32,
            false,
        ),
        conduit_text::MORSE_INTERSPERSE_KIND => leaf(
            ComparisonNodeKind::Intersperse,
            crate::offer::MORSE_INTERSPERSE_IMPLEMENTATION,
            conduit_text::MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES as u32,
            false,
        ),
        conduit_text::MORSE_FLATTEN_KIND => leaf(
            ComparisonNodeKind::Flatten,
            crate::offer::MORSE_FLATTEN_IMPLEMENTATION,
            conduit_text::MAXIMUM_MORSE_GAPPED_GROUPS_BYTES as u32,
            false,
        ),
        conduit_text::MORSE_SYMBOLS_TO_PATTERN_KIND => leaf(
            ComparisonNodeKind::SymbolsToPattern,
            crate::offer::MORSE_SYMBOLS_TO_PATTERN_IMPLEMENTATION,
            conduit_text::MAXIMUM_MORSE_SYMBOLS_BYTES as u32,
            false,
        ),
        conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND => (
            ComparisonNodeKind::Indicator,
            crate::offer::INDICATOR_PRESENTATION_IMPLEMENTATION,
            TourMorseOperation::Indicator(IndicatorOperation {
                pending: false,
                complete: false,
            }),
        ),
        _ => return Err(SchedulerError::InvalidPlan),
    };
    if placement.implementation_id.as_str() != expected_implementation {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok((kind, operation))
}

fn leaf(
    kind: ComparisonNodeKind,
    implementation: &'static str,
    maximum_input_bytes: u32,
    direct: bool,
) -> (ComparisonNodeKind, &'static str, TourMorseOperation) {
    let operation = if direct {
        TourMorseOperation::Morse(MorseOperation {
            pending: false,
            emitted: false,
        })
    } else {
        TourMorseOperation::Leaf(LeafOperation {
            maximum_input_bytes,
            pending: false,
            emitted: false,
        })
    };
    (kind, implementation, operation)
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
        .filter(|value| *value == "HELLO")
        .ok_or(SchedulerError::InvalidPlan)
}
