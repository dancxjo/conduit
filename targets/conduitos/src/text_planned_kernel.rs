//! Allocation-independent installation of the lowered ordinary text Plan.

use crate::text_kernel_operations::{
    LiteralOperation, LiteralState, PlannedOperation, PresentationOperation, UpperOperation,
};
use conduit_core::{ConfigurationValue, PlanFragment};
#[cfg(test)]
use conduit_kernel::RequestId;
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, KernelEvent, NodeId, SignSink, ValueRef,
    ValueStorage,
    scheduler::{
        FixedScheduler, HostOperationRequest, OperationDriver, SchedulerError, SchedulerStatus,
    },
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment};

const MAX_NODES: usize = 3;
const MAX_CORDS: usize = 2;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 2;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = 2;
const HOST_BINDING_SLOTS: usize = MAX_NODES * MAX_NODES;
const PENDING_REQUESTS: usize = 3;
const VALUE_SLOTS: usize = 6;
const VALUE_BYTES: usize = (conduit_text::MAX_TEXT_BYTES as usize) * 3;
const SIGN_CAPACITY: usize = 64;

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

pub struct TextPlannedKernel {
    scheduler: Scheduler,
    upper_node: NodeId,
    presentation_node: NodeId,
}

impl TextPlannedKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        validate_shape(fragment, lowered)?;
        let mut values = FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32)?;
        let literal_index = fragment
            .placements
            .iter()
            .position(|placement| placement.kind_id.as_str() == conduit_text::TEXT_LITERAL_KIND)
            .ok_or(SchedulerError::InvalidPlan)?;
        let presentation_index = fragment
            .placements
            .iter()
            .position(|placement| {
                placement.kind_id.as_str() == conduit_semantic_catalog::TEXT_PRESENTATION_KIND
            })
            .ok_or(SchedulerError::InvalidPlan)?;
        let upper_index = fragment
            .placements
            .iter()
            .position(|placement| placement.kind_id.as_str() == conduit_text::TEXT_UPPER_KIND)
            .ok_or(SchedulerError::InvalidPlan)?;
        let literal = configured_text(&fragment.placements[literal_index].configuration, "value")?;
        let text = values.store(literal.as_bytes())?;
        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| SchedulerError::InvalidPlan)?;
        let cords = [lowered.cords[0].spec, lowered.cords[1].spec];
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
        let literal_driver = OperationDriver::new(PlannedOperation::Literal(LiteralOperation {
            text,
            state: LiteralState::Emitting,
        }))?;
        let presentation_driver =
            OperationDriver::new(PlannedOperation::Presentation(PresentationOperation {
                pending: false,
                complete: false,
            }))?;
        let upper_driver = OperationDriver::new(PlannedOperation::Upper(UpperOperation {
            pending: false,
            emitted: false,
        }))?;
        let mut drivers = [None, None, None];
        drivers[literal_index] = Some(literal_driver);
        drivers[upper_index] = Some(upper_driver);
        drivers[presentation_index] = Some(presentation_driver);
        let [Some(first), Some(second), Some(third)] = drivers else {
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
                [first, second, third],
                values,
                signs,
            )?,
            upper_node: NodeId(upper_index as u16),
            presentation_node: NodeId(presentation_index as u16),
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

    pub fn complete_presentation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<(), SchedulerError> {
        if request.node != self.presentation_node
            || request.operation != conduit_kernel::HostOperationId(0)
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

    pub fn complete_upper(
        &mut self,
        request: HostOperationRequest,
        output: &[u8],
    ) -> Result<(), SchedulerError> {
        if !self.is_upper_request(&request) {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        let value = self.scheduler.store_host_value(output)?;
        let output = BoundedValueRef::new(value, conduit_text::MAX_TEXT_BYTES)
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
    #[cfg(test)]
    fn fail_presentation(&mut self, request: HostOperationRequest) -> Result<(), SchedulerError> {
        if !self.is_presentation_request(&request) {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        self.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::HostOperationFailed,
                    detail: 1,
                }),
            },
        )
    }
    pub fn is_presentation_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.presentation_node
            && request.operation == conduit_kernel::HostOperationId(0)
    }
    pub fn is_upper_request(&self, request: &HostOperationRequest) -> bool {
        request.node == self.upper_node && request.operation == conduit_kernel::HostOperationId(0)
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
        .filter(|value| {
            value.len() <= conduit_text::MAX_TEXT_BYTES as usize
                && core::str::from_utf8(value.as_bytes()).is_ok()
        })
        .ok_or(SchedulerError::InvalidPlan)
}

fn validate_shape(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<(), SchedulerError> {
    if fragment.placements.len() != MAX_NODES
        || fragment.connections.len() != MAX_CORDS
        || lowered.nodes.len() != MAX_NODES
        || lowered.cords.len() != MAX_CORDS
        || lowered.routes.len() != 2
        || lowered.host_operations.len() != 2
        || lowered.cord_value_slots != 2
        || lowered.cord_value_bytes != conduit_text::MAX_TEXT_BYTES * 2
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(SchedulerError::InvalidPlan);
    }
    let literal = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_text::TEXT_LITERAL_KIND)
        .ok_or(SchedulerError::InvalidPlan)?;
    let presentation = fragment
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::TEXT_PRESENTATION_KIND
        })
        .ok_or(SchedulerError::InvalidPlan)?;
    let upper = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_text::TEXT_UPPER_KIND)
        .ok_or(SchedulerError::InvalidPlan)?;
    if literal.implementation_id.as_str() != crate::offer::TEXT_LITERAL_IMPLEMENTATION
        || upper.implementation_id.as_str() != crate::offer::TEXT_UPPER_IMPLEMENTATION
        || presentation.implementation_id.as_str() != crate::offer::TEXT_PRESENTATION_IMPLEMENTATION
        || upper.host_operations.len() != 1
        || upper.host_operations[0].contract_id.as_str()
            != crate::functional_offers::TEXT_UPPER_HOST_OPERATION
        || upper.host_operations[0]
            .target_kind
            .as_ref()
            .map(|kind| kind.as_str())
            != Some(crate::functional_offers::TEXT_UPPER_HOST_OPERATION_TARGET)
        || upper.host_operations[0].maximum_in_flight != 1
        || upper.host_operations[0].maximum_input_bytes != conduit_text::MAX_TEXT_BYTES
        || upper.host_operations[0].maximum_output_bytes != conduit_text::MAX_TEXT_BYTES
        || configured_text(&literal.configuration, "value")? != crate::ordinary_plan::TEXT_LITERAL
        || configured_u64(&presentation.configuration, "maximum-values")?
            != conduit_semantic_catalog::MAX_TEXT_VALUES
    {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::BootIdentities,
        offer::{CpuFeatures, HostOffer},
        ordinary_plan,
    };

    fn kernel() -> TextPlannedKernel {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        ordinary_plan::prepare(&identities, &offer, "build")
            .unwrap()
            .kernel
    }

    #[test]
    fn ordinary_cancellation_is_terminal() {
        let mut kernel = kernel();
        kernel.cancel().unwrap();
        assert_eq!(kernel.step(), Ok(SchedulerStatus::Cancelled));
    }

    #[test]
    fn malformed_presentation_completion_is_rejected() {
        let mut kernel = kernel();
        assert_eq!(
            kernel.complete_presentation(HostOperationRequest {
                node: NodeId(99),
                request: RequestId(99),
                operation: conduit_kernel::HostOperationId(0),
                input: BoundedValueRef::new(
                    ValueRef {
                        slot: 0,
                        generation: 0,
                        byte_len: 1,
                    },
                    1,
                )
                .unwrap(),
            }),
            Err(SchedulerError::InvalidHostOperationAccess)
        );
    }

    #[test]
    fn mutated_upper_effect_identity_is_rejected_before_play() {
        let identities = BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        };
        let offer = HostOffer::new(
            &identities,
            "build",
            CpuFeatures {
                sse2: true,
                rdrand: true,
                invariant_tsc: true,
            },
            256 * 1024,
        );
        let prepared = ordinary_plan::prepare(&identities, &offer, "build").unwrap();
        let mut fragment = prepared.plan.fragments[0].clone();
        let upper = fragment
            .placements
            .iter_mut()
            .find(|placement| placement.kind_id.as_str() == conduit_text::TEXT_UPPER_KIND)
            .unwrap();
        upper.host_operations[0].target_kind = Some(conduit_core::KindId::from("wrong/transform"));
        assert!(conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).is_err());
    }

    #[test]
    fn presentation_base_loss_remains_a_distinct_terminal_failure() {
        let mut kernel = kernel();
        let request = loop {
            assert!(matches!(
                kernel.step(),
                Ok(SchedulerStatus::Progress { .. })
            ));
            if let Some(request) = kernel.next_host_request() {
                if kernel.is_upper_request(&request) {
                    let input = kernel.host_value(request.input.value).unwrap();
                    let output = crate::text_upper::uppercase(input).unwrap();
                    let bytes = output.as_bytes().to_vec();
                    kernel.complete_upper(request, &bytes).unwrap();
                } else {
                    break request;
                }
            }
        };
        kernel.fail_presentation(request).unwrap();
        loop {
            match kernel.step() {
                Ok(SchedulerStatus::Progress { .. }) => {}
                outcome => {
                    assert_eq!(
                        outcome,
                        Err(SchedulerError::OperationFailed(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::HostOperationFailed,
                            detail: 23
                        }))
                    );
                    break;
                }
            }
        }
    }
}
