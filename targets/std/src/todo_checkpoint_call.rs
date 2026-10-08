//! One planned Todo checkpoint generation through the ordinary Host Call boundary.
//!
//! The Back releases no state to its output until the selected std residence
//! has acknowledged durable publication. The next command needs a fresh Plan.
use crate::todo_durable_resource::{CheckpointIdentity, Refusal, SelectedTodoResidence};
use conduit_core::{AuthorityBinding, PlannedGear};
use conduit_kernel::{
    scheduler::{HostCallRequest, StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, HostCallOutcome,
    NodeId, PortId, RequestId, ValueRef,
};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use conduit_todo_plot::{TodoState, STATE_MAX_BYTES};
use std::path::Path;

pub struct TodoCheckpointBack {
    pending: Option<ValueRef>,
    sent: bool,
    closed: bool,
}

impl TodoCheckpointBack {
    pub const fn new() -> Self {
        Self {
            pending: None,
            sent: false,
            closed: false,
        }
    }
}
impl Default for TodoCheckpointBack {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for TodoCheckpointBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.closed {
            return StepOutcome::Complete;
        }
        if let Some(value) = self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != RequestId(0) {
                return fail(1);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed
                || outcome.output != BoundedValueRef::new(value, STATE_MAX_BYTES as u32).ok()
            {
                return fail(2);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err() || io.send(PortId(0), value).is_err() {
                return fail(3);
            }
            self.pending = None;
            self.sent = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.sent {
                return fail(4);
            }
            let Ok(input) = BoundedValueRef::new(value, STATE_MAX_BYTES as u32) else {
                return fail(5);
            };
            if io.consume(PortId(0)).is_err()
                || io
                    .request_host_call(RequestId(0), HostCallId(0), input)
                    .is_err()
            {
                return fail(6);
            }
            self.pending = Some(value);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return fail(7);
            }
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.pending = None;
        self.closed = true;
    }
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::HostCallFailed,
        detail,
    })
}

pub struct TodoCheckpointHost {
    residence: SelectedTodoResidence,
    grant: AuthorityBinding,
    node: NodeId,
    call: HostCallId,
}

impl TodoCheckpointHost {
    pub fn prepare(
        root: &Path,
        placement: &PlannedGear,
        lowered: &KernelIdentityMap,
        identity: CheckpointIdentity,
    ) -> Result<Self, Refusal> {
        let residence = SelectedTodoResidence::prepare(root, placement, identity)?;
        let node = lowered
            .node_for_placement(&placement.placement_id)
            .ok_or(Refusal::InvalidBinding)?;
        let call = lowered
            .host_call_for_contract(node, &placement.host_calls[0].contract_id)
            .ok_or(Refusal::InvalidBinding)?;
        if call != HostCallId(0) || lowered.plan_id.as_str().is_empty() {
            return Err(Refusal::InvalidBinding);
        }
        let grant = placement.authority[0].clone();
        Ok(Self {
            residence,
            grant,
            node,
            call,
        })
    }

    /// Completion is returned only after the provider has committed its
    /// immutable candidate and current selector. Unknown outcomes fail closed.
    pub fn perform(&self, request: HostCallRequest, input: &[u8]) -> HostCallOutcome {
        if request.node != self.node
            || request.call != self.call
            || request.input.admitted_bytes != STATE_MAX_BYTES as u32
            || request.input.value.byte_len as usize != input.len()
            || input.len() > STATE_MAX_BYTES
        {
            return denied();
        }
        let state = match TodoState::decode_info(input) {
            Ok(state) => state,
            Err(_) => return failed(),
        };
        completion_for_commit(request, self.residence.commit(&self.grant, &state))
    }
}

fn completion_for_commit(request: HostCallRequest, result: Result<(), Refusal>) -> HostCallOutcome {
    match result {
        Ok(()) => HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: Some(request.input),
            failure: None,
        },
        Err(_) => failed(),
    }
}

const fn denied() -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Denied,
        output: None,
        failure: Some(Failure {
            code: FailureCode::HostCallDenied,
            detail: 1,
        }),
    }
}
const fn failed() -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Failed,
        output: None,
        failure: Some(Failure {
            code: FailureCode::HostCallFailed,
            detail: 2,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_publication_outcome_never_acknowledges_state() {
        let request = HostCallRequest {
            node: NodeId(1),
            request: RequestId(0),
            call: HostCallId(0),
            input: BoundedValueRef::new(
                ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: 1,
                },
                STATE_MAX_BYTES as u32,
            )
            .unwrap(),
        };
        let outcome = completion_for_commit(request, Err(Refusal::UnknownOutcome));
        assert_eq!(outcome.disposition, HostCallDisposition::Failed);
        assert!(outcome.output.is_none());
    }
}
