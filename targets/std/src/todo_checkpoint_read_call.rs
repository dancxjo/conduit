//! One exact ReadPublished Todo generation through the admitted Host Call.
use crate::todo_durable_resource::{CheckpointIdentity, Refusal, SelectedTodoResidence};
use conduit_core::{AuthorityBinding, PlannedGear};
use conduit_kernel::{
    scheduler::{HostCallRequest, StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, NodeId, PortId,
    RequestId, ValueRef,
};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use conduit_todo_plot::{TodoState, STATE_MAX_BYTES};
use std::path::Path;

pub struct TodoCheckpointReadBack {
    empty_input: ValueRef,
    pending: bool,
    done: bool,
}
impl TodoCheckpointReadBack {
    pub const fn new(empty_input: ValueRef) -> Self {
        Self {
            empty_input,
            pending: false,
            done: false,
        }
    }
}
impl<const PORTS: usize> StepBack<PORTS> for TodoCheckpointReadBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.done {
            return StepOutcome::Complete;
        }
        if self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != RequestId(0) {
                return fail(1);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            let Some(output) = outcome.output else {
                return fail(2);
            };
            let Some(bytes) = inputs.host_output() else {
                return fail(3);
            };
            if outcome.disposition != HostCallDisposition::Completed
                || output.admitted_bytes != STATE_MAX_BYTES as u32
                || output.value.byte_len as usize != bytes.len()
                || TodoState::decode_info(bytes).is_err()
            {
                return fail(4);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            // The completed Host Call already stored this admitted value. Reuse
            // its ValueRef: canonical derived outputs are limited to 100 bytes,
            // while a valid Todo checkpoint can be much larger.
            if io.consume_host_completion().is_err() || io.send(PortId(0), output.value).is_err() {
                return fail(6);
            }
            self.done = true;
            self.pending = false;
            return StepOutcome::Progress;
        }
        let Ok(input) = BoundedValueRef::new(self.empty_input, 0) else {
            return fail(7);
        };
        if io
            .request_host_call(RequestId(0), HostCallId(0), input)
            .is_err()
        {
            return fail(8);
        }
        self.pending = true;
        StepOutcome::Progress
    }
    fn cancel(&mut self) {
        self.pending = false;
        self.done = true;
    }
}
const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::HostCallFailed,
        detail,
    })
}

pub struct TodoCheckpointReadHost {
    residence: SelectedTodoResidence,
    grant: AuthorityBinding,
    node: NodeId,
    call: HostCallId,
}
impl TodoCheckpointReadHost {
    pub fn prepare(
        root: &Path,
        placement: &PlannedGear,
        lowered: &KernelIdentityMap,
        identity: CheckpointIdentity,
    ) -> Result<Self, Refusal> {
        if placement.implementation_id.as_str()
            != conduit_std_offers::TODO_CHECKPOINT_READ_IMPLEMENTATION
            || placement.kind_id.as_str() != conduit_todo_plot::TODO_CHECKPOINT_READ_KIND
        {
            return Err(Refusal::InvalidBinding);
        }
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
        Ok(Self {
            residence,
            grant: placement.authority[0].clone(),
            node,
            call,
        })
    }
    pub fn read(&self, request: HostCallRequest, input: &[u8]) -> Result<Vec<u8>, Refusal> {
        if request.node != self.node
            || request.call != self.call
            || request.request != RequestId(0)
            || request.input.admitted_bytes != 0
            || request.input.value.byte_len != 0
            || !input.is_empty()
        {
            return Err(Refusal::InvalidBinding);
        }
        let state = self.residence.recover(&self.grant)?;
        let bytes = state.encode_info().map_err(|_| Refusal::Corrupt)?;
        if bytes.len() > STATE_MAX_BYTES {
            return Err(Refusal::Corrupt);
        }
        Ok(bytes)
    }
}

pub const fn read_failure_detail(refusal: &Refusal) -> u16 {
    match refusal {
        Refusal::Missing => 1,
        Refusal::Corrupt => 2,
        Refusal::StaleRevision => 3,
        Refusal::WrongAuthority | Refusal::WrongAccess => 4,
        Refusal::InvalidBinding => 5,
        Refusal::InvalidState => 6,
        Refusal::UnknownOutcome => 7,
        Refusal::Storage => 8,
        Refusal::MigrationRequired => 9,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_revision_outcomes_remain_distinct() {
        let missing = read_failure_detail(&Refusal::Missing);
        let stale = read_failure_detail(&Refusal::StaleRevision);
        let corrupt = read_failure_detail(&Refusal::Corrupt);
        assert_ne!(missing, stale);
        assert_ne!(missing, corrupt);
        assert_ne!(stale, corrupt);
    }
}
