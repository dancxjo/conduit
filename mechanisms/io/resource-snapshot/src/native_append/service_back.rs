//! Prepared execution only: two serial Host Calls sharing one service node.
//! This owns no media, grants, or admission proof. The caller must fund the
//! fixed read response backing and bind this exact shape before installation.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
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
    InvalidDeclaredReadCancellation,
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
    Close(usize),
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
    cancelled: bool,
    declared_read_cancellation: bool,
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
            cancelled: false,
            declared_read_cancellation: false,
        })
    }
    /// Opt in only for a selected read operation whose output is the declared
    /// resource read-result type. Its binder/provider must prove that semantic
    /// mapping independently; numeric shape alone does not establish a Kind.
    /// The genuine kernel Cancelled outcome remains Cancelled in its Signs.
    pub fn new_with_declared_read_cancellation(
        shape: RecordServiceShape,
    ) -> Result<Self, RecordServiceBackRefusal> {
        let mut back = Self::new(shape)?;
        let cancelled = crate::read_result::cancelled();
        if crate::read_result::decode(cancelled) != Ok(crate::read_result::ReadResult::Cancelled)
            || cancelled.len() > READ_BYTES
            || cancelled.len() as u64 > u64::from(shape.read.maximum_output_bytes)
        {
            return Err(RecordServiceBackRefusal::InvalidDeclaredReadCancellation);
        }
        back.declared_read_cancellation = true;
        Ok(back)
    }
}
impl<const READ_BYTES: usize, const PORTS: usize> StepBack<PORTS>
    for RecordServiceBack<PORTS, READ_BYTES>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        // Rejected Steps must not publish candidate state on a later attempt.
        self.candidate = None;
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
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Cancelled, None, None)
                    if operation == 1 && self.declared_read_cancellation =>
                {
                    if self.read_len.is_some() {
                        return fail(FailureCode::InvalidLifecycle);
                    }
                    let data = crate::read_result::cancelled();
                    self.read_response[..data.len()].copy_from_slice(data);
                    if io.consume_host_completion().is_err() {
                        return fail(FailureCode::InvalidLifecycle);
                    }
                    self.candidate = Some(Commit::Read { len: data.len() });
                    return StepOutcome::Progress;
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
                return StepOutcome::Progress;
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
            return StepOutcome::Progress;
        }
        for operation in 0..2 {
            let op = self.operations[operation];
            if !self.closed[operation] && io.input_closed(op.input) {
                if io.consume_closed(op.input).is_err() {
                    return fail(FailureCode::InvalidLifecycle);
                }
                self.candidate = Some(Commit::Close(operation));
                return StepOutcome::Progress;
            }
        }
        if self.closed == [true; 2] && self.read_len.is_none() {
            StepOutcome::Complete
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
            Some(Commit::Close(operation)) => self.closed[operation] = true,
            None => {}
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
    }
}
fn fail(code: FailureCode) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail: 0 })
}
#[cfg(test)]
#[path = "service_back/tests.rs"]
mod tests;
