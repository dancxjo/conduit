//! Exact one-Play execution for one admitted semantic Patchbay action.

use super::presentation_validation::validate_presentation_invocation;
use super::*;
use conduit_core::{bind_active_play, BaseImplementationId};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerError, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedSignLog,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef,
    ValueStorage,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
use std::collections::BTreeMap;

const NODES: usize = 2;
const CORDS: usize = 1;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 4;
const ROUTE_SLOTS: usize = NODES * PORTS;
const ROUTE_TARGETS: usize = 1;
const HOST_BINDINGS: usize = NODES;
const PENDING_REQUESTS: usize = 1;
const SIGN_ITEMS: u16 = 32;
const MAXIMUM_DECISIONS: u32 = 32;

type InteractionScheduler = FixedScheduler<
    OperationDriver<InteractionOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDINGS,
    PENDING_REQUESTS,
>;

enum InteractionOperation {
    Source { value: ValueRef, emitted: bool },
    Apply { pending: bool },
}

impl Operation for InteractionOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value, .. } => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Apply { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Apply { pending },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending => {
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_INTERACTION_VALUE_BYTES) {
                        Ok(input) => input,
                        Err(_) => return failed(FailureCode::InvalidInput, 1),
                    },
                }
            }
            (
                Self::Apply { pending },
                OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome,
                },
            ) if *pending => {
                *pending = false;
                match outcome.disposition {
                    HostOperationDisposition::Completed
                        if outcome.output.is_none() && outcome.failure.is_none() =>
                    {
                        OperationAction::Complete
                    }
                    HostOperationDisposition::Denied => failed(FailureCode::HostOperationDenied, 2),
                    HostOperationDisposition::Cancelled => failed(FailureCode::Cancelled, 3),
                    _ => failed(FailureCode::HostOperationFailed, 4),
                }
            }
            _ => failed(FailureCode::InvalidLifecycle, 5),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { emitted, .. } if !*emitted => {
                *emitted = true;
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    fn cancel(&mut self) {
        if let Self::Apply { pending } = self {
            *pending = false;
        }
    }
}

fn failed(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

impl PatchbayInteraction {
    pub fn execute<F>(
        &mut self,
        graph: Option<&crate::PatchbayGraph>,
        request: PatchbayInteractionRequest,
        invoke: F,
    ) -> Result<InteractionReceipt, InteractionError>
    where
        F: FnOnce(&PatchbayInteractionRequest) -> PatchbayInvocationOutcome,
    {
        self.execute_with_subject_resolver(request, invoke, |candidate| {
            graph
                .ok_or(PatchbayRefusal::OperationUnavailable)
                .and_then(|graph| {
                    graph
                        .resolve_subject_ref(candidate)
                        .map(|_| ())
                        .map_err(|error| match error {
                            crate::PatchbayGraphError::StaleGraphBasis => {
                                PatchbayRefusal::StalePresentation
                            }
                            _ => PatchbayRefusal::UnknownSubject,
                        })
                })
        })
    }

    /// Executes selection against the exact portable Presentation consumed by a renderer.
    /// Renderer-local hit/focus facts have already been reduced to `candidate`; no DOM or
    /// geometry identity crosses this boundary.
    pub fn execute_presentation<F>(
        &mut self,
        presentation: &conduit_presentation::Presentation,
        request: PatchbayInteractionRequest,
        invoke: F,
    ) -> Result<InteractionReceipt, InteractionError>
    where
        F: FnOnce(&PatchbayInteractionRequest) -> PatchbayInvocationOutcome,
    {
        let invocation_refusal = validate_presentation_invocation(presentation, &request).err();
        self.execute_with_subject_resolver(
            request,
            move |request| {
                invocation_refusal
                    .map_or_else(|| invoke(request), PatchbayInvocationOutcome::Refused)
            },
            |candidate| {
                if presentation.basis.expanded_form_id.as_ref() != Some(&candidate.expanded_form_id)
                {
                    return Err(PatchbayRefusal::StalePresentation);
                }
                presentation
                    .subjects
                    .iter()
                    .any(|subject| subject.identity == candidate.subject_identity)
                    .then_some(())
                    .ok_or(PatchbayRefusal::UnknownSubject)
            },
        )
    }

    fn execute_with_subject_resolver<F, R>(
        &mut self,
        request: PatchbayInteractionRequest,
        invoke: F,
        resolve_subject: R,
    ) -> Result<InteractionReceipt, InteractionError>
    where
        F: FnOnce(&PatchbayInteractionRequest) -> PatchbayInvocationOutcome,
        R: Fn(&crate::PatchbaySubjectRef) -> Result<(), PatchbayRefusal>,
    {
        let duplicate_delivery = self
            .history
            .iter()
            .any(|receipt| receipt.request.request_id() == request.request_id());
        let expanded = expanded_request(&request)?;
        let planned_request = request_from_expanded(&expanded)?;
        if planned_request != request {
            return Err(InteractionError::Form(
                "expanded interaction request differs from typed input".into(),
            ));
        }
        let advertisement = self.advertisement();
        let hosts = [advertisement.clone()];
        let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
            .map_err(|error| InteractionError::Planning(error.to_string()))?;
        let plan = conduit_planner::plan_expanded_canonical_with_options(
            &expanded,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: QUEUE_SLOTS as u16,
                connection_byte_capacity: MAX_INTERACTION_VALUE_BYTES * QUEUE_SLOTS as u32,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|error| InteractionError::Planning(error.to_string()))?;
        let fragment = plan
            .fragments
            .first()
            .ok_or_else(|| InteractionError::Planning("interaction Plan has no fragment".into()))?;
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| InteractionError::Execution(format!("lowering: {error:?}")))?;
        validate_shape(&lowered)?;

        let encoded = planned_request.encode()?;
        let mut values = HostedValueStore::new(
            2,
            MAX_INTERACTION_VALUE_BYTES,
            MAX_INTERACTION_VALUE_BYTES * 2,
        )
        .map_err(|error| InteractionError::Execution(format!("value store: {error:?}")))?;
        let request_value = values
            .store(&encoded)
            .map_err(|error| InteractionError::Execution(format!("request value: {error:?}")))?;
        let operations = operations(fragment, &lowered, request_value)?;
        let drivers = operations
            .map(OperationDriver::new)
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(scheduler_error("prepare interaction driver"))?
            .try_into()
            .map_err(|_| InteractionError::Execution("interaction driver count changed".into()))?;
        let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(protocol_error("install route"))?;
        }
        routes.seal().map_err(protocol_error("seal routes"))?;
        let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(1);
        for operation in &lowered.host_operations {
            bindings
                .install(operation.node, operation.binding)
                .map_err(protocol_error("install host operation"))?;
        }
        bindings
            .seal()
            .map_err(protocol_error("seal host operations"))?;
        let sign_bytes = u32::from(SIGN_ITEMS)
            * u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
                .map_err(|_| InteractionError::Execution("Sign size overflow".into()))?;
        let signs = HostedSignLog::new(SIGN_ITEMS, sign_bytes)
            .map_err(|error| InteractionError::Execution(format!("Sign store: {error:?}")))?;
        let node_specs =
            lowered.node_specs.clone().try_into().map_err(|_| {
                InteractionError::Execution("interaction node count changed".into())
            })?;
        let cord_specs = [lowered.cords[0].spec];
        let mut scheduler = InteractionScheduler::new_with_host_operations(
            node_specs, cord_specs, routes, bindings, drivers, values, signs,
        )
        .map_err(scheduler_error("prepare scheduler"))?;

        let active_play_id = bind_active_play(
            &plan.plan_id,
            &advertisement.host_id,
            &advertisement.boot_id,
            self.play_sequence,
        )
        .active_play_id;
        self.play_sequence = self.play_sequence.saturating_add(1);
        let mut invoke = Some(invoke);
        let mut disposition = None;
        let mut selected = None;
        for _ in 0..MAXIMUM_DECISIONS {
            while let Some(host_request) = scheduler.next_host_request() {
                let bytes = scheduler
                    .host_value(host_request.input.value)
                    .map_err(scheduler_error("read interaction request"))?;
                let decoded = PatchbayInteractionRequest::decode(bytes)?;
                let outcome = if decoded != planned_request {
                    disposition = Some(InteractionDisposition::Failed);
                    failed_outcome()
                } else {
                    match &decoded {
                        PatchbayInteractionRequest::Select {
                            expanded_form_id,
                            subject_identity,
                            ..
                        } => {
                            let candidate = crate::PatchbaySubjectRef {
                                expanded_form_id: expanded_form_id.clone(),
                                subject_identity: subject_identity.clone(),
                            };
                            match resolve_subject(&candidate) {
                                Ok(_) => {
                                    selected = Some(candidate);
                                    disposition = Some(InteractionDisposition::Succeeded);
                                    completed_outcome()
                                }
                                Err(reason) => {
                                    disposition = Some(InteractionDisposition::Refused(reason));
                                    denied_outcome()
                                }
                            }
                        }
                        PatchbayInteractionRequest::Invoke { .. }
                        | PatchbayInteractionRequest::Edit { .. } => {
                            if matches!(&decoded, PatchbayInteractionRequest::Invoke { .. })
                                && decoded.control_request()?.is_none()
                            {
                                return Err(InteractionError::InvalidIdentity);
                            }
                            match if duplicate_delivery {
                                PatchbayInvocationOutcome::Refused(
                                    PatchbayRefusal::DuplicateDelivery,
                                )
                            } else {
                                invoke
                                    .take()
                                    .map_or(PatchbayInvocationOutcome::Failed, |invoke| {
                                        invoke(&decoded)
                                    })
                            } {
                                PatchbayInvocationOutcome::Succeeded => {
                                    disposition = Some(InteractionDisposition::Succeeded);
                                    completed_outcome()
                                }
                                PatchbayInvocationOutcome::Refused(reason) => {
                                    disposition = Some(InteractionDisposition::Refused(reason));
                                    denied_outcome()
                                }
                                PatchbayInvocationOutcome::Failed => {
                                    disposition = Some(InteractionDisposition::Failed);
                                    failed_outcome()
                                }
                            }
                        }
                    }
                };
                scheduler
                    .complete_host_operation(host_request.node, host_request.request, outcome)
                    .map_err(scheduler_error("complete interaction host operation"))?;
            }
            match scheduler.step() {
                Ok(SchedulerStatus::Complete) => break,
                Ok(SchedulerStatus::Progress { .. }) => {}
                Ok(SchedulerStatus::Idle) => {
                    return Err(InteractionError::Execution(
                        "interaction Play became idle before terminal".into(),
                    ));
                }
                Ok(SchedulerStatus::Cancelled) => {
                    disposition = Some(InteractionDisposition::Failed);
                    break;
                }
                Err(SchedulerError::OperationFailed(_)) if disposition.is_some() => break,
                Err(error) => return Err(scheduler_error("step interaction kernel")(error)),
            }
        }
        let disposition = disposition.ok_or_else(|| {
            InteractionError::Execution("interaction Play exceeded its decision bound".into())
        })?;
        if disposition == InteractionDisposition::Succeeded {
            if let Some(selected) = selected {
                self.selected = Some(selected);
            }
        }
        let receipt = InteractionReceipt {
            request,
            source_document_id: plan.source_document_id.clone(),
            checked_form_id: plan.checked_form_id.clone(),
            expanded_form_id: plan.expanded_form_id.clone(),
            plan_id: plan.plan_id.clone(),
            plan,
            active_play_id,
            disposition,
            signs: scheduler.signs().events().collect(),
        };
        self.retain(receipt.clone());
        Ok(receipt)
    }
}

fn validate_shape(
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
) -> Result<(), InteractionError> {
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || lowered.routes.len() != 1
        || lowered.host_operations.len() != 1
        || lowered.cord_value_slots != QUEUE_SLOTS as u16
    {
        return Err(InteractionError::Execution(format!(
            "interaction shape changed: nodes={} cords={} routes={} host-operations={} slots={}",
            lowered.nodes.len(),
            lowered.cords.len(),
            lowered.routes.len(),
            lowered.host_operations.len(),
            lowered.cord_value_slots
        )));
    }
    Ok(())
}

fn operations(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    request: ValueRef,
) -> Result<[InteractionOperation; NODES], InteractionError> {
    let mut operations = Vec::with_capacity(NODES);
    for node in &lowered.nodes {
        let placement = fragment
            .placements
            .get(usize::from(node.node.0))
            .ok_or_else(|| InteractionError::Execution("node placement is absent".into()))?;
        let operation = match placement.kind_id.as_str() {
            SELECT_KIND | INVOKE_KIND | EDIT_KIND => InteractionOperation::Source {
                value: request,
                emitted: false,
            },
            APPLY_KIND => InteractionOperation::Apply { pending: false },
            _ => {
                return Err(InteractionError::Execution(
                    "unplanned interaction Kind".into(),
                ))
            }
        };
        operations.push(operation);
    }
    operations
        .try_into()
        .map_err(|_| InteractionError::Execution("interaction operation count changed".into()))
}

fn completed_outcome() -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: None,
        failure: None,
    }
}

fn denied_outcome() -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Denied,
        output: None,
        failure: Some(Failure {
            code: FailureCode::HostOperationDenied,
            detail: 1,
        }),
    }
}

fn failed_outcome() -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Failed,
        output: None,
        failure: Some(Failure {
            code: FailureCode::HostOperationFailed,
            detail: 1,
        }),
    }
}

fn protocol_error(
    context: &'static str,
) -> impl FnOnce(conduit_kernel::ProtocolError) -> InteractionError {
    move |error| InteractionError::Execution(format!("{context}: {error:?}"))
}

fn scheduler_error(context: &'static str) -> impl FnOnce(SchedulerError) -> InteractionError {
    move |error| InteractionError::Execution(format!("{context}: {error:?}"))
}
