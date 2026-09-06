//! Local multi-partition composition of the existing kernel and Host effects.
use super::{
    kernel_preparation::KernelTables, preparation, simple_presentation_host, InstalledOperation,
    InstalledScheduler, MAX_CORDS, MAX_NODES, MAX_QUEUE_SLOTS, PENDING_REQUESTS,
};
use crate::{hosted_keyboard::HostedKeyboardAdapter, RunControl, TimerAdapter};
use conduit_core::{CancellationReason, FailureReason, PlanFragment, TerminalDisposition};
use conduit_kernel::{
    scheduler::{HostOperationRequest, OperationDriver, SchedulerStatus},
    HostOperationDisposition, HostOperationOutcome, HostedSignLog, HostedValueStore, KernelEvent,
};
use conduit_plan_lowering::{
    fragment_set::{lower_local_fragment_set, FragmentSetBounds},
    lowering::{KernelIdentityMap, LoweredHostOperation, FIXED_KERNEL_STORAGE_PROFILE},
};
use std::{io::Write, time::Duration};

pub(crate) struct BodyKernel {
    scheduler: InstalledScheduler,
    partitions: Vec<KernelIdentityMap>,
    operations: Vec<LoweredHostOperation>,
    requests: Vec<HostOperationRequest>,
}

pub(crate) struct BodyKernelResult {
    pub terminal: TerminalDisposition,
    pub failure: Option<String>,
    pub partitions: Vec<KernelIdentityMap>,
    pub requests: Vec<HostOperationRequest>,
    pub events: Vec<KernelEvent>,
}

fn keyboard(contract: &conduit_core::HostOperationContractId) -> bool {
    contract.as_str() == conduit_std_offers::NEXT_KEY_EVENT_HOST_OPERATION_CONTRACT
}
fn timer(contract: &conduit_core::HostOperationContractId) -> bool {
    contract.as_str() == conduit_core::WAIT_HOST_OPERATION_CONTRACT
}
fn presentation(operation: &LoweredHostOperation) -> bool {
    operation.target_kind.as_ref().is_some_and(|target| {
        matches!(
            target.as_str(),
            "presentation/stdout-text"
                | conduit_std_offers::TICK_PRESENTATION_TARGET
                | conduit_std_offers::COUNT_PRESENTATION_TARGET
                | conduit_std_offers::BOOL_PRESENTATION_TARGET
        ) && operation.contract_id
            == conduit_core::present_host_operation_requirement(
                target.clone(),
                operation.binding.maximum_input_bytes,
            )
            .contract_id
    })
}

impl BodyKernel {
    pub(crate) fn prepare(fragments: &[&PlanFragment], has_keyboard: bool) -> Result<Self, String> {
        let lowered = lower_local_fragment_set(
            fragments,
            FIXED_KERNEL_STORAGE_PROFILE,
            FragmentSetBounds {
                fragments: conduit_body::MAX_BODY_FORMS as u16,
                nodes: MAX_NODES as u16,
                cords: MAX_CORDS as u16,
                queue_slots: MAX_QUEUE_SLOTS as u16,
                value_bytes: 16 * 1024 * 1024,
                sign_items: 4096,
                sign_bytes: 4 * 1024 * 1024,
            },
        )
        .map_err(|error| format!("Body fragment lowering: {error:?}"))?;
        for operation in lowered
            .partitions
            .iter()
            .flat_map(|part| &part.host_operations)
        {
            if keyboard(&operation.contract_id) {
                if !has_keyboard {
                    return Err("Body keyboard has no admitted adapter".into());
                }
            } else if !timer(&operation.contract_id) && !presentation(operation) {
                return Err(format!(
                    "Body Host operation is unsupported: {}",
                    operation.contract_id.as_str()
                ));
            }
        }
        let mut items = 0_u16;
        let mut bytes = 0_u32;
        let mut maximum = 1_u32;
        let mut sign_items = 32_u16;
        let mut request_capacity = 0_usize;
        for placement in fragments.iter().flat_map(|part| &part.placements) {
            let budget = preparation::operation_budget(placement)?;
            items = items
                .checked_add(budget.value_items)
                .ok_or("Body value item overflow")?;
            bytes = bytes
                .checked_add(budget.value_bytes)
                .filter(|bytes| *bytes <= 16 * 1024 * 1024)
                .ok_or("Body value byte capacity exceeded")?;
            maximum = maximum.max(budget.maximum_value_bytes);
            sign_items = sign_items
                .checked_add(budget.sign_items)
                .ok_or("Body Sign overflow")?;
            request_capacity = request_capacity
                .checked_add(budget.host_requests)
                .ok_or("Body request overflow")?;
        }
        let mut values = HostedValueStore::new(items.max(1), maximum, bytes.max(1))
            .map_err(|error| format!("Body value store: {error:?}"))?;
        let mut drivers =
            core::array::from_fn(|_| OperationDriver::new(InstalledOperation::inactive()).unwrap());
        for (fragment, part) in fragments.iter().zip(&lowered.partitions) {
            for node in &part.nodes {
                drivers[usize::from(node.node.0)] =
                    OperationDriver::new(preparation::prepare_ordinary_operation(
                        fragment,
                        &node.placement_id,
                        &mut values,
                    )?)
                    .map_err(|error| format!("Body operation preparation: {error:?}"))?;
            }
        }
        let tables = KernelTables::prepare(&lowered.partitions.iter().collect::<Vec<_>>())?;
        let signs = HostedSignLog::new(
            sign_items,
            u32::from(sign_items) * core::mem::size_of::<KernelEvent>() as u32,
        )
        .map_err(|error| format!("Body Sign store: {error:?}"))?;
        Ok(Self {
            scheduler: tables.install(drivers, values, signs)?,
            operations: lowered
                .partitions
                .iter()
                .flat_map(|part| part.host_operations.clone())
                .collect(),
            partitions: lowered
                .partitions
                .into_iter()
                .map(|part| part.identity)
                .collect(),
            requests: Vec::with_capacity(request_capacity),
        })
    }

    pub(crate) fn run<W: Write, T: TimerAdapter>(
        mut self,
        output: &mut W,
        clock: &mut T,
        input: Option<&mut dyn HostedKeyboardAdapter>,
        control: &RunControl,
    ) -> BodyKernelResult {
        let mut keys = super::keyboard_input_host::KeyboardInputHost::new(input);
        let mut deadlines = super::deadline_host::InstalledDeadlineHost::<PENDING_REQUESTS>::new();
        let result = (|| -> Result<TerminalDisposition, String> {
            let mut cancelling = false;
            loop {
                if !cancelling && control.requested_stop().is_some() {
                    self.scheduler
                        .cancel()
                        .map_err(|error| format!("Body cancel: {error:?}"))?;
                    cancelling = true;
                }
                while let Some(cancellation) = self.scheduler.next_host_cancellation() {
                    let operation = self
                        .operations
                        .iter()
                        .find(|op| {
                            op.node == cancellation.node && op.operation == cancellation.operation
                        })
                        .ok_or("Body cancellation has no exact operation")?;
                    if keyboard(&operation.contract_id) {
                        keys.cancel();
                        self.scheduler
                            .complete_host_operation(
                                cancellation.node,
                                cancellation.request,
                                HostOperationOutcome {
                                    disposition: HostOperationDisposition::Cancelled,
                                    output: None,
                                    failure: None,
                                },
                            )
                            .map_err(|error| format!("Body keyboard cancellation: {error:?}"))?;
                    } else {
                        deadlines.cancel(cancellation, &mut self.scheduler)?;
                    }
                }
                while let Some(request) = self.scheduler.next_host_request() {
                    if self.requests.len() == self.requests.capacity() {
                        return Err("Body request capacity exceeded".into());
                    }
                    self.requests.push(request);
                    let operation = self
                        .operations
                        .iter()
                        .find(|op| op.node == request.node && op.operation == request.operation)
                        .ok_or("Body request has no exact partition operation")?;
                    let input = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|error| format!("Body request value: {error:?}"))?;
                    if keyboard(&operation.contract_id) {
                        keys.accept(request, input)?;
                        continue;
                    }
                    if timer(&operation.contract_id) {
                        let duration =
                            conduit_time::decode_tick(input).map_err(|error| error.to_string())?;
                        if let Some(now) = clock.monotonic_now_ms() {
                            deadlines.arm(request, duration, now)?;
                            continue;
                        }
                        clock.wait(Duration::from_millis(duration));
                    } else if !simple_presentation_host::present(
                        operation.target_kind.as_ref(),
                        input,
                        output,
                    )? {
                        return Err("Body presentation contract became unsupported".into());
                    }
                    self.scheduler
                        .complete_host_operation(
                            request.node,
                            request.request,
                            HostOperationOutcome {
                                disposition: HostOperationDisposition::Completed,
                                output: None,
                                failure: None,
                            },
                        )
                        .map_err(|error| format!("Body Host completion: {error:?}"))?;
                }
                match self
                    .scheduler
                    .step()
                    .map_err(|error| format!("Body kernel: {error:?}"))?
                {
                    SchedulerStatus::Complete => return Ok(TerminalDisposition::Completed),
                    SchedulerStatus::Cancelled => {
                        return Ok(TerminalDisposition::Cancelled {
                            reason: CancellationReason::OperatorRequested,
                        })
                    }
                    SchedulerStatus::Progress { .. } => {}
                    SchedulerStatus::Idle => {
                        if keys.poll(&mut self.scheduler)?
                            || deadlines.complete_next(&mut self.scheduler, clock)?
                        {
                            continue;
                        }
                        if !keys.is_pending() {
                            return Err("Body kernel has no admitted progress source".into());
                        }
                        std::thread::yield_now();
                    }
                }
            }
        })();
        let (terminal, failure) = match result {
            Ok(terminal) => (terminal, None),
            Err(error) => {
                keys.cancel();
                deadlines.clear();
                (
                    TerminalDisposition::Failed {
                        reason: FailureReason::RequiredBranchFailed,
                    },
                    Some(error),
                )
            }
        };
        BodyKernelResult {
            terminal,
            failure,
            partitions: self.partitions,
            requests: self.requests,
            events: self.scheduler.signs().events().collect(),
        }
    }
}
