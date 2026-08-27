use super::planning::*;
use super::*;
use conduit_core::bind_active_play;
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerError, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedSignLog,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId, ValueRef,
    ValueStorage,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
pub(super) const VALUE_BYTES: u32 = 1024;
const QUEUE_SLOTS: usize = 2;
const NODES: usize = 2;
const CORDS: usize = 1;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const ROUTE_SLOTS: usize = NODES * PORTS;
const MAX_DECISIONS: usize = 32;

type Scheduler = FixedScheduler<
    OperationDriver<ObligationOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    1,
    NODES,
    1,
>;

enum ObligationOperation {
    Source { value: ValueRef, emitted: bool },
    Execute { pending: bool },
}

impl Operation for ObligationOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value, .. } => OperationAction::Emit {
                port: PortId(0),
                value: *value,
            },
            Self::Execute { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Execute { pending },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending => {
                *pending = true;
                match BoundedValueRef::new(value, VALUE_BYTES) {
                    Ok(input) => OperationAction::RequestHostOperation {
                        request: RequestId(0),
                        operation: HostOperationId(0),
                        input,
                    },
                    Err(_) => failed(FailureCode::InvalidInput, 1),
                }
            }
            (
                Self::Execute { pending },
                OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome,
                },
            ) if *pending => {
                *pending = false;
                if outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none()
                {
                    OperationAction::Complete
                } else {
                    failed(FailureCode::HostOperationFailed, 2)
                }
            }
            _ => failed(FailureCode::InvalidLifecycle, 3),
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
        if let Self::Execute { pending } = self {
            *pending = false;
        }
    }
}

pub fn run_obligation<F>(
    basis: ObligationBasis,
    prior: Option<ObligationRecord>,
    interrupt_after_checkpoint: bool,
    execute_command: F,
) -> Result<ObligationRecord, ObligationRefusal>
where
    F: FnOnce() -> bool,
{
    let mut attempts = Vec::new();
    let mut retention_gap = 0;
    let attempts_used = if let Some(prior) = prior {
        validate_prior(&prior, &basis)?;
        let used = u8::try_from(prior.attempts.len())
            .map_err(|_| ObligationRefusal::CorruptCheckpoint)?
            .saturating_add(
                u8::try_from(prior.retention_gap)
                    .map_err(|_| ObligationRefusal::CorruptCheckpoint)?,
            );
        attempts = prior.attempts;
        retention_gap = prior.retention_gap;
        used
    } else {
        0
    };
    if attempts_used >= MAX_ATTEMPTS {
        return Err(ObligationRefusal::AttemptBudgetExhausted);
    }
    let attempt_number = attempts_used + 1;
    let (form, plan, advertisement) = checked_plan(&basis)?;
    let fragment = &plan.fragments[0];
    let lowered = lower_plan_fragment(fragment).map_err(|_| ObligationRefusal::StepFailed)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || lowered.routes.len() != 1
        || lowered.host_operations.len() != 1
    {
        return Err(ObligationRefusal::StepFailed);
    }
    let encoded = serde_json::to_vec(&basis).map_err(|_| ObligationRefusal::StepFailed)?;
    if encoded.len() > VALUE_BYTES as usize {
        return Err(ObligationRefusal::StepFailed);
    }
    let mut scheduler = scheduler(fragment, &lowered, &encoded)?;
    let play = bind_active_play(
        &plan.plan_id,
        &advertisement.host_id,
        &advertisement.boot_id,
        u64::from(attempt_number),
    );
    let attempt_id = digest(format!(
        "attempt:{}:{attempt_number}",
        obligation_id(&basis)
    ));
    let checkpoint = ObligationCheckpoint {
        schema_version: OBLIGATION_SCHEMA_VERSION,
        obligation_id: obligation_id(&basis),
        basis: basis.clone(),
        checkpoint_id: checkpoint_id(&basis, attempt_number),
        completed_steps: vec!["basis-checked".into()],
        residual: vec![ResidualStep::ExecuteProofCatalog],
        attempts_used: attempt_number,
    };
    let mut execute_command = Some(execute_command);
    let mut checkpoint_reached = false;
    let mut step_succeeded = None;
    let verdict = 'run: {
        for _ in 0..MAX_DECISIONS {
            while let Some(request) = scheduler.next_host_request() {
                checkpoint_reached = true;
                if interrupt_after_checkpoint {
                    scheduler
                        .cancel()
                        .map_err(|_| ObligationRefusal::StepFailed)?;
                    break 'run ObligationVerdict::Interrupted;
                }
                let input = scheduler
                    .host_value(request.input.value)
                    .map_err(|_| ObligationRefusal::StepFailed)?;
                let decoded: ObligationBasis =
                    serde_json::from_slice(input).map_err(|_| ObligationRefusal::StepFailed)?;
                let success =
                    decoded == basis && execute_command.take().is_some_and(|execute| execute());
                step_succeeded = Some(success);
                let outcome = if success { completed() } else { host_failed() };
                scheduler
                    .complete_host_operation(request.node, request.request, outcome)
                    .map_err(|_| ObligationRefusal::StepFailed)?;
                if !success {
                    break 'run ObligationVerdict::Failed;
                }
            }
            match scheduler.step() {
                Ok(SchedulerStatus::Complete) => break 'run ObligationVerdict::Completed,
                Ok(SchedulerStatus::Progress { .. }) => {}
                _ => break 'run ObligationVerdict::Failed,
            }
        }
        ObligationVerdict::Failed
    };
    if !checkpoint_reached {
        return Err(ObligationRefusal::StepFailed);
    }
    let signs = scheduler
        .signs()
        .events()
        .take(MAX_SIGNS)
        .map(|event| format!("{:?}", event.kind))
        .collect();
    let receipt = step_succeeded.map(|succeeded| ValidationReceipt {
        receipt_id: digest(format!("receipt:{attempt_id}:{succeeded}")),
        command: basis.command.clone(),
        artifact_digest: basis.artifact_digest.clone(),
        succeeded,
    });
    attempts.push(AttemptRecord {
        attempt_id,
        play_id: play.active_play_id.as_str().into(),
        checkpoint_id: Some(checkpoint.checkpoint_id.clone()),
        receipt,
        verdict: verdict.clone(),
        signs,
    });
    while attempts.len() > MAX_RETAINED_ATTEMPTS {
        attempts.remove(0);
        retention_gap += 1;
    }
    let interrupted = verdict == ObligationVerdict::Interrupted;
    Ok(ObligationRecord {
        schema_version: OBLIGATION_SCHEMA_VERSION,
        obligation_id: obligation_id(&basis),
        basis,
        form_id: form.expanded_form_id.as_str().into(),
        plan_id: plan.plan_id.as_str().into(),
        attempts,
        retention_gap,
        checkpoint: interrupted.then_some(checkpoint),
        terminal_verdict: (!interrupted).then_some(verdict),
    })
}

fn validate_prior(
    prior: &ObligationRecord,
    basis: &ObligationBasis,
) -> Result<(), ObligationRefusal> {
    if prior.schema_version != OBLIGATION_SCHEMA_VERSION {
        return Err(ObligationRefusal::CorruptCheckpoint);
    }
    if &prior.basis != basis {
        return Err(if prior.basis.source_commit != basis.source_commit {
            ObligationRefusal::StaleCommit
        } else if prior.basis.command != basis.command || prior.basis.tool != basis.tool {
            ObligationRefusal::ChangedCommand
        } else if prior.basis.profile != basis.profile
            || prior.basis.proof_class != basis.proof_class
        {
            ObligationRefusal::ChangedProfile
        } else {
            ObligationRefusal::IncompatibleArtifact
        });
    }
    if prior.obligation_id != obligation_id(basis)
        || prior.attempts.is_empty()
        || prior.attempts.len() > MAX_RETAINED_ATTEMPTS
    {
        return Err(ObligationRefusal::CorruptCheckpoint);
    }
    match prior.attempts.last().map(|attempt| &attempt.verdict) {
        Some(ObligationVerdict::Interrupted) => prior
            .checkpoint
            .as_ref()
            .ok_or(ObligationRefusal::CorruptCheckpoint)?
            .validate(basis),
        Some(ObligationVerdict::Failed) if prior.checkpoint.is_none() => Ok(()),
        _ => Err(ObligationRefusal::CorruptCheckpoint),
    }
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    encoded: &[u8],
) -> Result<Scheduler, ObligationRefusal> {
    let mut values = HostedValueStore::new(2, VALUE_BYTES, VALUE_BYTES * 2)
        .map_err(|_| ObligationRefusal::StepFailed)?;
    let value = values
        .store(encoded)
        .map_err(|_| ObligationRefusal::StepFailed)?;
    let mut operations = Vec::new();
    for node in &lowered.nodes {
        let placement = &fragment.placements[usize::from(node.node.0)];
        operations.push(match placement.kind_id.as_str() {
            SOURCE_KIND => ObligationOperation::Source {
                value,
                emitted: false,
            },
            EXECUTE_KIND => ObligationOperation::Execute { pending: false },
            _ => return Err(ObligationRefusal::StepFailed),
        });
    }
    let drivers = operations
        .into_iter()
        .map(OperationDriver::new)
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| ObligationRefusal::StepFailed)?;
    let mut routes = FixedRoutes::<ROUTE_SLOTS, 1>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| ObligationRefusal::StepFailed)?;
    }
    routes.seal().map_err(|_| ObligationRefusal::StepFailed)?;
    let mut bindings = FixedHostOperationBindings::<NODES>::new(1);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| ObligationRefusal::StepFailed)?;
    }
    bindings.seal().map_err(|_| ObligationRefusal::StepFailed)?;
    let sign_bytes = u32::try_from(MAX_SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>())
        .map_err(|_| ObligationRefusal::StepFailed)?;
    let signs = HostedSignLog::new(MAX_SIGNS as u16, sign_bytes)
        .map_err(|_| ObligationRefusal::StepFailed)?;
    Scheduler::new_with_host_operations(
        lowered
            .node_specs
            .clone()
            .try_into()
            .map_err(|_| ObligationRefusal::StepFailed)?,
        [lowered.cords[0].spec],
        routes,
        bindings,
        drivers,
        values,
        signs,
    )
    .map_err(|_| ObligationRefusal::StepFailed)
}

fn completed() -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Completed,
        output: None,
        failure: None,
    }
}

fn host_failed() -> HostOperationOutcome {
    HostOperationOutcome {
        disposition: HostOperationDisposition::Failed,
        output: None,
        failure: Some(Failure {
            code: FailureCode::HostOperationFailed,
            detail: 1,
        }),
    }
}

fn failed(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}

impl From<SchedulerError> for ObligationRefusal {
    fn from(_: SchedulerError) -> Self {
        Self::StepFailed
    }
}
