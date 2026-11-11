//! Prepared execution only: two serial Host Calls sharing one service node.
//! This owns no media, grants, or admission proof. The caller must fund the
//! fixed read response backing and bind this exact shape before installation.
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedFiniteTerminalEmission, AssignedNormalCloseTransduction,
        AssignedTerminalTransduction, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

#[derive(Clone, Copy, Debug)]
pub struct RecordServiceOperation {
    pub input: PortId,
    pub output: PortId,
    pub call: HostCallId,
    pub maximum_input_bytes: u32,
    pub maximum_output_bytes: u32,
}
#[derive(Clone, Copy, Debug)]
pub struct RecordServiceShape {
    pub append: RecordServiceOperation,
    pub read: RecordServiceOperation,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordServiceBackRefusal {
    InvalidShape,
    InsufficientReadBacking,
}
#[derive(Clone, Copy)]
enum Commit {
    Request {
        request: RequestId,
        operation: usize,
    },
    Read {
        len: usize,
    },
    Append,
    Drain,
}

pub struct RecordServiceBack<const PORTS: usize, const READ_BYTES: usize> {
    operations: [RecordServiceOperation; 2],
    pending: Option<(RequestId, usize)>,
    next_request: u32,
    cursor: usize,
    read_response: [u8; READ_BYTES],
    read_len: Option<usize>,
    closed: [bool; 2],
    candidate: Option<Commit>,
    candidate_close: Option<usize>,
    cancelled: bool,
}
impl<const PORTS: usize, const READ_BYTES: usize> RecordServiceBack<PORTS, READ_BYTES> {
    /// PORTS is the selected node's input/output table width, not a capacity
    /// default. Distinct ports and calls are required; call ordering is supplied
    /// by lowering rather than assumed here.
    pub fn new(shape: RecordServiceShape) -> Result<Self, RecordServiceBackRefusal> {
        let operations = [shape.append, shape.read];
        if operations.iter().any(|op| {
            usize::from(op.input.0) >= PORTS
                || usize::from(op.output.0) >= PORTS
                || op.maximum_input_bytes == 0
                || op.maximum_output_bytes == 0
        }) || shape.append.input == shape.read.input
            || shape.append.output == shape.read.output
            || shape.append.call == shape.read.call
            || shape
                .append
                .maximum_output_bytes
                .checked_add(shape.read.maximum_output_bytes)
                .is_none()
        {
            return Err(RecordServiceBackRefusal::InvalidShape);
        }
        if usize::try_from(shape.read.maximum_output_bytes)
            .ok()
            .filter(|n| *n <= READ_BYTES)
            .is_none()
        {
            return Err(RecordServiceBackRefusal::InsufficientReadBacking);
        }
        Ok(Self {
            operations,
            pending: None,
            next_request: 0,
            cursor: 0,
            read_response: [0; READ_BYTES],
            read_len: None,
            closed: [false; 2],
            candidate: None,
            candidate_close: None,
            cancelled: false,
        })
    }
    // Consume at most one exposed terminal truth in the same transaction as a
    // response/request. A last close may enter the finite flush phase while the
    // single retained read response awaits its output; it never cancels a call.
    fn stage_close(&mut self, io: &mut StepIo<PORTS>) -> Result<bool, ()> {
        for operation in 0..2 {
            let input = self.operations[operation].input;
            if !self.closed[operation] && io.input_closed(input) {
                io.consume_closed(input).map_err(|_| ())?;
                self.candidate_close = Some(operation);
                return Ok(self.closed[1 - operation]);
            }
        }
        Ok(self.closed == [true; 2])
    }
}
impl<const READ_BYTES: usize, const PORTS: usize> StepBack<PORTS>
    for RecordServiceBack<PORTS, READ_BYTES>
{
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        let mut profiles = [None; PORTS];
        for op in &self.operations {
            profiles[usize::from(op.input.0)] = Some(AssignedTerminalTransduction {
                input: op.input,
                output: op.output,
                normal_close: AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(
                    AssignedFiniteTerminalEmission {
                        maximum_items: 2,
                        maximum_bytes: self.operations[0].maximum_output_bytes
                            + self.operations[1].maximum_output_bytes,
                    },
                ),
                abnormal: AssignedAbnormalTransduction::NotAccepted,
                cancellation: AssignedCancellationTransduction::NotCancellable,
            });
        }
        profiles
    }
    // Cancellation is terminal for the service. Transport pressure or loss of
    // a remote observer must not be translated into cancellation of local Play.
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        // Rejected Steps must not publish candidate state on a later attempt.
        self.candidate = None;
        self.candidate_close = None;
        if self.cancelled {
            return fail(FailureCode::Cancelled);
        }
        if let Some((request, outcome)) = io.host_completion() {
            let Some((expected, operation)) = self.pending else {
                return fail(FailureCode::InvalidLifecycle);
            };
            if request != expected {
                return fail(FailureCode::InvalidLifecycle);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    let op = self.operations[operation];
                    if output.value.byte_len > op.maximum_output_bytes {
                        return fail(FailureCode::InvalidInput);
                    }
                    if operation == 1 {
                        if self.read_len.is_some() {
                            return fail(FailureCode::InvalidLifecycle);
                        }
                        let Some(data) = bytes.host_output() else {
                            return fail(FailureCode::InvalidInput);
                        };
                        if data.len() != output.value.byte_len as usize || data.len() > READ_BYTES {
                            return fail(FailureCode::InvalidInput);
                        }
                        self.read_response[..data.len()].copy_from_slice(data);
                        if io.consume_host_completion().is_err() {
                            return fail(FailureCode::InvalidLifecycle);
                        }
                        self.candidate = Some(Commit::Read { len: data.len() });
                    } else {
                        if !io.output_ready_for(op.output, output.value.byte_len) {
                            return StepOutcome::Await;
                        }
                        if io.consume_host_completion().is_err()
                            || io.send(op.output, output.value).is_err()
                        {
                            return fail(FailureCode::InvalidLifecycle);
                        }
                        self.candidate = Some(Commit::Append);
                    }
                    let all_closed = match self.stage_close(io) {
                        Ok(all) => all,
                        Err(()) => return fail(FailureCode::InvalidLifecycle),
                    };
                    return if operation == 0 && all_closed && self.read_len.is_none() {
                        StepOutcome::Complete
                    } else {
                        StepOutcome::Progress
                    };
                }
                (HostCallDisposition::Cancelled, _, _) => return fail(FailureCode::Cancelled),
                (_, _, Some(failure)) => return StepOutcome::Fail(failure),
                _ => return fail(FailureCode::InvalidLifecycle),
            }
        }
        if self.pending.is_some() {
            return StepOutcome::Await;
        }
        if let Some(len) = self.read_len {
            if io.output_ready_for(self.operations[1].output, len as u32) {
                if io
                    .send_prepared(self.operations[1].output, len as u32)
                    .is_err()
                {
                    return fail(FailureCode::InvalidLifecycle);
                }
                self.candidate = Some(Commit::Drain);
                return match self.stage_close(io) {
                    Ok(true) => StepOutcome::Complete,
                    Ok(false) => StepOutcome::Progress,
                    Err(()) => fail(FailureCode::InvalidLifecycle),
                };
            }
        }
        for offset in 0..2 {
            let operation = (self.cursor + offset) % 2;
            let op = self.operations[operation];
            if self.closed[operation] || (operation == 1 && self.read_len.is_some()) {
                continue;
            }
            let Some(value) = io.input(op.input) else {
                continue;
            };
            let Ok(input) = BoundedValueRef::new(value, op.maximum_input_bytes) else {
                return fail(FailureCode::InvalidInput);
            };
            if self.next_request == u32::MAX {
                return fail(FailureCode::StorageExhausted);
            }
            let request = RequestId(self.next_request);
            if io.consume(op.input).is_err()
                || io.request_host_call(request, op.call, input).is_err()
            {
                return fail(FailureCode::InvalidLifecycle);
            }
            self.candidate = Some(Commit::Request { request, operation });
            if self.stage_close(io).is_err() {
                return fail(FailureCode::InvalidLifecycle);
            }
            return StepOutcome::Progress;
        }
        let all_closed = match self.stage_close(io) {
            Ok(all) => all,
            Err(()) => return fail(FailureCode::InvalidLifecycle),
        };
        if all_closed && self.read_len.is_none() {
            StepOutcome::Complete
        } else if self.candidate_close.is_some() {
            StepOutcome::Progress
        } else {
            StepOutcome::Await
        }
    }

    fn step_committed(&mut self) {
        match self.candidate.take() {
            Some(Commit::Request { request, operation }) => {
                self.pending = Some((request, operation));
                self.next_request = request.0 + 1;
                self.cursor = 1 - operation;
            }
            Some(Commit::Read { len }) => {
                self.pending = None;
                self.read_len = Some(len);
            }
            Some(Commit::Append) => self.pending = None,
            Some(Commit::Drain) => self.read_len = None,
            None => {}
        }
        if let Some(operation) = self.candidate_close.take() {
            self.closed[operation] = true;
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port == self.operations[1].output {
            self.read_len.map(|len| &self.read_response[..len])
        } else {
            None
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.pending = None;
        self.read_len = None;
        self.candidate = None;
        self.candidate_close = None;
    }
}
fn fail(code: FailureCode) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail: 0 })
}
#[cfg(test)]
#[path = "service_back/tests.rs"]
mod tests;
