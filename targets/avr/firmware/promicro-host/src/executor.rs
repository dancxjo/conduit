//! Exact execution of the selected Create contact implementation.
//!
//! The operation state machine and scheduling are the ordinary Conduit kernel.
//! This module supplies only the implementation-specific Create effect and the
//! compact portable contact value selected by the assigned Plan.

use conduit_assigned_plan::{
    AssignedActivation, AssignedExecutionReceipt, AssignedTerminalDisposition,
};
use conduit_create_oi::{
    query_create1_group_zero, transition_oi_mode, CreateOiFailure, CreateOiModeRequest,
    CreateUartProvider,
};
use crate::assigned_receiver::ValidatedContactPlan;
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, NodeId, Operation, OperationAction, OperationInput, PortId, RequestId,
    SingleSourceExecutor, SingleSourceSignLog, SingleSourceValues, ValueRef,
};

const CONTACT_PORT: PortId = PortId(0);
const HOST_OPERATION: HostOperationId = HostOperationId(0);
const REQUEST: RequestId = RequestId(1);
const VALUE_BYTES: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionRefusal {
    ActivationMismatch,
    UnsupportedPlan,
    Kernel,
    Device(CreateOiFailure),
}

impl ExecutionRefusal {
    pub const fn detail(self) -> u16 {
        match self {
            Self::ActivationMismatch => 1,
            Self::UnsupportedPlan => 2,
            Self::Kernel => 3,
            Self::Device(CreateOiFailure::ProviderUnavailable) => 10,
            Self::Device(CreateOiFailure::WrongUartProfile { .. }) => 11,
            Self::Device(CreateOiFailure::WriteFailed) => 12,
            Self::Device(CreateOiFailure::ReadFailed) => 13,
            Self::Device(CreateOiFailure::Timeout) => 14,
            Self::Device(CreateOiFailure::DeviceNoResponse) => 15,
            Self::Device(CreateOiFailure::UnsupportedPacket(_)) => 16,
            Self::Device(CreateOiFailure::TruncatedFrame) => 17,
            Self::Device(CreateOiFailure::MalformedFrame) => 18,
            Self::Device(CreateOiFailure::SynchronizationLimit { .. }) => 19,
        }
    }
}

#[derive(Clone, Copy)]
struct ContactSource {
    empty: ValueRef,
    pending: bool,
    emitted: bool,
}

impl Operation for ContactSource {
    #[inline(never)]
    fn start(&mut self) -> OperationAction {
        self.pending = true;
        let input = match BoundedValueRef::new(self.empty, 0) {
            Ok(input) => input,
            Err(_) => return invalid(0),
        };
        OperationAction::RequestHostOperation {
            request: REQUEST,
            operation: HOST_OPERATION,
            input,
        }
    }

    #[inline(never)]
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if request == REQUEST
                    && self.pending
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                self.pending = false;
                match outcome.output {
                    Some(output) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: CONTACT_PORT,
                            value: output.value,
                        }
                    }
                    None => invalid(1),
                }
            }
            OperationInput::HostOperationCompleted { outcome, .. }
                if self.pending && outcome.disposition == HostOperationDisposition::Failed =>
            {
                self.pending = false;
                OperationAction::Fail(outcome.failure.unwrap_or(Failure {
                    code: FailureCode::HostOperationFailed,
                    detail: 2,
                }))
            }
            _ => invalid(3),
        }
    }

    #[inline(never)]
    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            self.emitted = false;
            OperationAction::Complete
        } else {
            invalid(4)
        }
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.emitted = false;
    }
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

pub fn execute_contact<'a, P: CreateUartProvider>(
    plan: ValidatedContactPlan,
    activation: AssignedActivation,
    provider: &mut P,
    deadline_tick: u64,
    value: &'a mut [u8; VALUE_BYTES],
) -> AssignedExecutionReceipt<'a> {
    let result = execute(plan, activation, provider, deadline_tick, value);
    let (disposition, detail, output) = match result {
        Ok(()) => (AssignedTerminalDisposition::Completed, 0, &value[..]),
        Err(refusal) => (
            match refusal {
                ExecutionRefusal::ActivationMismatch | ExecutionRefusal::UnsupportedPlan => {
                    AssignedTerminalDisposition::Refused
                }
                ExecutionRefusal::Kernel | ExecutionRefusal::Device(_) => {
                    AssignedTerminalDisposition::Failed
                }
            },
            refusal.detail(),
            &value[..0],
        ),
    };
    AssignedExecutionReceipt {
        activation,
        output_port: CONTACT_PORT.0,
        disposition,
        detail,
        value: output,
    }
}

#[inline(never)]
fn execute<P: CreateUartProvider>(
    plan: ValidatedContactPlan,
    activation: AssignedActivation,
    provider: &mut P,
    deadline_tick: u64,
    value: &mut [u8; VALUE_BYTES],
) -> Result<(), ExecutionRefusal> {
    if activation.plan != plan.assigned.plan
        || activation.fragment != plan.assigned.fragment
        || activation.host != plan.assigned.host
        || activation.boot != plan.assigned.boot
    {
        return Err(ExecutionRefusal::ActivationMismatch);
    }
    let mut values = SingleSourceValues::<VALUE_BYTES>::new();
    let empty = values.empty();
    let signs = SingleSourceSignLog::new();
    let source = ContactSource {
        empty,
        pending: false,
        emitted: false,
    };
    let mut kernel = SingleSourceExecutor::new(
        source,
        signs,
        NodeId(0),
        HOST_OPERATION,
        0,
        plan.maximum_output_bytes,
        plan.maximum_step_work,
    )
    .map_err(|_| ExecutionRefusal::Kernel)?;
    let request = kernel.start().map_err(|_| ExecutionRefusal::Kernel)?;
    if request.request != REQUEST || request.operation != HOST_OPERATION {
        return Err(ExecutionRefusal::Kernel);
    }
    let observed = transition_oi_mode(provider, CreateOiModeRequest::Full, deadline_tick)
        .map_err(|failure| ExecutionRefusal::Device(failure.failure))?;
    if observed as u8 != 3 {
        return Err(ExecutionRefusal::Device(CreateOiFailure::MalformedFrame));
    }
    let group =
        query_create1_group_zero(provider, deadline_tick).map_err(ExecutionRefusal::Device)?;
    // Compact serialization of robotics/contact-body-sectors@1. The target
    // does not define another observation type: Create left and right bumpers
    // map to the existing front-left/front-right bits.
    value[0] = u8::from(group.left_bumper_pressed) << 1
        | u8::from(group.right_bumper_pressed) << 2;
    let stored = values
        .store_output(value)
        .map_err(|_| ExecutionRefusal::Kernel)?;
    let completed = kernel
        .complete(
            REQUEST,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(stored, plan.maximum_output_bytes)
                        .map_err(|_| ExecutionRefusal::Kernel)?,
                ),
                failure: None,
            },
        )
        .map_err(|_| ExecutionRefusal::Kernel)?;
    if completed.port.0 != plan.output_port
        || completed.port != CONTACT_PORT
        || completed.value.value != stored
    {
        return Err(ExecutionRefusal::Kernel);
    }
    Ok(())
}
