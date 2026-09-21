//! Fixed one-Back source profile for tiny assigned fragments.
//!
//! This is a specialization of the same [`StepOperation`] protocol used by the
//! full scheduler. It is valid only for a fragment containing one source, one
//! Host Call, one output Port, and no local or remote Cords. Any other
//! shape must be refused before construction.

use crate::scheduler::{HostCallRequest, StepInputBytes, StepIo, StepOperation, StepOutcome};
use crate::{
    BoundedValueRef, HostCallId, HostCallOutcome, KernelEventKind, NodeId, PortId,
    RemoteLifecycleIdentity, RequestId, SignError, SignSink, StorageError, ValueRef,
};

/// Exact three-Sign storage required by the single-source execution profile.
/// This implements the ordinary Sign contract without remote capacity, which
/// an admitted zero-Cord fragment cannot use.
pub struct SingleSourceSignLog {
    entries: [Option<crate::KernelEvent>; 3],
    len: u8,
}

impl SingleSourceSignLog {
    pub const fn new() -> Self {
        Self {
            entries: [None; 3],
            len: 0,
        }
    }
}

impl Default for SingleSourceSignLog {
    fn default() -> Self {
        Self::new()
    }
}

impl SignSink for SingleSourceSignLog {
    fn item_capacity(&self) -> u16 {
        3
    }

    fn byte_capacity(&self) -> u32 {
        (3 * core::mem::size_of::<crate::KernelEvent>()) as u32
    }

    fn len(&self) -> u16 {
        u16::from(self.len)
    }

    fn used_bytes(&self) -> u32 {
        u32::from(self.len) * core::mem::size_of::<crate::KernelEvent>() as u32
    }

    fn record(
        &mut self,
        node: NodeId,
        port: Option<PortId>,
        request: Option<RequestId>,
        kind: KernelEventKind,
    ) -> Result<crate::KernelEvent, SignError> {
        let index = usize::from(self.len);
        if index == self.entries.len() {
            return Err(SignError::ItemCapacityExceeded);
        }
        let event = crate::KernelEvent {
            sequence: u32::from(self.len),
            node,
            port,
            request,
            kind,
        };
        self.entries[index] = Some(event);
        self.len += 1;
        Ok(event)
    }

    fn record_remote(
        &mut self,
        _node: NodeId,
        _port: PortId,
        _kind: KernelEventKind,
        _remote: RemoteLifecycleIdentity,
    ) -> Result<crate::KernelEvent, SignError> {
        Err(SignError::RemoteItemCapacityExceeded)
    }

    fn ensure_remote_capacity(&self, additional: u16) -> Result<(), SignError> {
        if additional == 0 {
            Ok(())
        } else {
            Err(SignError::RemoteItemCapacityExceeded)
        }
    }
}

/// Exact empty-input and one-output value storage for this profile.
pub struct SingleSourceValues<const MAX_OUTPUT_BYTES: usize> {
    bytes: [u8; MAX_OUTPUT_BYTES],
    stored: bool,
}

impl<const MAX_OUTPUT_BYTES: usize> SingleSourceValues<MAX_OUTPUT_BYTES> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_OUTPUT_BYTES],
            stored: false,
        }
    }

    pub const fn empty(&self) -> ValueRef {
        ValueRef {
            slot: 0,
            generation: 1,
            byte_len: 0,
        }
    }

    pub fn store_output(&mut self, bytes: &[u8]) -> Result<ValueRef, StorageError> {
        if self.stored {
            return Err(StorageError::ItemCapacityExceeded);
        }
        if bytes.is_empty() || bytes.len() > MAX_OUTPUT_BYTES || bytes.len() > usize::from(u16::MAX)
        {
            return Err(StorageError::ValueTooLarge);
        }
        self.bytes[..bytes.len()].copy_from_slice(bytes);
        self.stored = true;
        Ok(ValueRef {
            slot: 1,
            generation: 1,
            byte_len: bytes.len() as u32,
        })
    }
}

impl<const MAX_OUTPUT_BYTES: usize> Default for SingleSourceValues<MAX_OUTPUT_BYTES> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SingleSourceOutput {
    pub port: PortId,
    pub value: BoundedValueRef,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SingleSourceRefusal {
    InvalidBound,
    InvalidStart,
    WrongRequest,
    InvalidCompletion,
    BackFailed(u16),
    SignCapacity,
    AlreadyStarted,
    AlreadyTerminal,
}

pub struct SingleSourceExecutor<B, E> {
    back: B,
    signs: E,
    node: NodeId,
    call_id: HostCallId,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
    maximum_step_work: u16,
    request: Option<RequestId>,
    terminal: bool,
}

impl<B: StepOperation<1>, E: SignSink> SingleSourceExecutor<B, E> {
    pub fn new(
        back: B,
        signs: E,
        node: NodeId,
        call_id: HostCallId,
        maximum_input_bytes: u32,
        maximum_output_bytes: u32,
        maximum_step_work: u16,
    ) -> Result<Self, SingleSourceRefusal> {
        if maximum_step_work < 3 || maximum_output_bytes == 0 {
            return Err(SingleSourceRefusal::InvalidBound);
        }
        Ok(Self {
            back,
            signs,
            node,
            call_id,
            maximum_input_bytes,
            maximum_output_bytes,
            maximum_step_work,
            request: None,
            terminal: false,
        })
    }

    pub fn start(&mut self) -> Result<HostCallRequest, SingleSourceRefusal> {
        if self.terminal {
            return Err(SingleSourceRefusal::AlreadyTerminal);
        }
        if self.request.is_some() {
            return Err(SingleSourceRefusal::AlreadyStarted);
        }
        let mut io = StepIo::single_source(self.maximum_output_bytes, self.maximum_step_work, None);
        let input_bytes = StepInputBytes::single_source();
        if self.back.step(&mut io, &input_bytes) != StepOutcome::Progress {
            return Err(SingleSourceRefusal::InvalidStart);
        }
        let Some((request, call, input)) = io.single_source_start_request() else {
            return Err(SingleSourceRefusal::InvalidStart);
        };
        if call != self.call_id || input.value.byte_len > self.maximum_input_bytes {
            return Err(SingleSourceRefusal::InvalidStart);
        }
        self.request = Some(request);
        self.signs
            .record(
                self.node,
                None,
                Some(request),
                KernelEventKind::HostCallRequested,
            )
            .map_err(|_| SingleSourceRefusal::SignCapacity)?;
        Ok(HostCallRequest {
            node: self.node,
            request,
            call,
            input,
        })
    }

    pub fn complete(
        &mut self,
        request: RequestId,
        outcome: HostCallOutcome,
    ) -> Result<SingleSourceOutput, SingleSourceRefusal> {
        if self.terminal {
            return Err(SingleSourceRefusal::AlreadyTerminal);
        }
        if self.request != Some(request) {
            return Err(SingleSourceRefusal::WrongRequest);
        }
        if outcome
            .output
            .is_some_and(|output| output.value.byte_len > self.maximum_output_bytes)
        {
            return Err(SingleSourceRefusal::InvalidCompletion);
        }
        self.signs
            .record(
                self.node,
                None,
                Some(request),
                KernelEventKind::HostCallCompleted,
            )
            .map_err(|_| SingleSourceRefusal::SignCapacity)?;
        let mut io = StepIo::single_source(
            self.maximum_output_bytes,
            self.maximum_step_work,
            Some((request, outcome)),
        );
        let input_bytes = StepInputBytes::single_source();
        let outcome = self.back.step(&mut io, &input_bytes);
        if let StepOutcome::Fail(failure) = outcome {
            self.terminal = true;
            return Err(SingleSourceRefusal::BackFailed(failure.detail));
        }
        if outcome != StepOutcome::Complete {
            return Err(SingleSourceRefusal::InvalidCompletion);
        }
        let Some(value) = io.single_source_completion_output() else {
            return Err(SingleSourceRefusal::InvalidCompletion);
        };
        let port = PortId(0);
        let bounded = BoundedValueRef::new(value, self.maximum_output_bytes)
            .map_err(|_| SingleSourceRefusal::InvalidCompletion)?;
        self.back.step_committed();
        self.terminal = true;
        self.signs
            .record(self.node, Some(port), None, KernelEventKind::BackCompleted)
            .map_err(|_| SingleSourceRefusal::SignCapacity)?;
        Ok(SingleSourceOutput {
            port,
            value: bounded,
        })
    }

    pub fn signs(&self) -> &E {
        &self.signs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Failure, FailureCode, HostCallDisposition};

    #[derive(Clone, Copy)]
    struct Source;

    impl StepOperation<1> for Source {
        fn step(
            &mut self,
            io: &mut StepIo<1>,
            _input_bytes: &StepInputBytes<'_, 1>,
        ) -> StepOutcome {
            if let Some((RequestId(4), outcome)) = io.host_completion() {
                let HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                } = outcome
                else {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::InvalidLifecycle,
                        detail: 9,
                    });
                };
                if io.consume_host_completion().is_err()
                    || io.send(PortId(0), output.value).is_err()
                {
                    return StepOutcome::Fail(Failure {
                        code: FailureCode::InvalidLifecycle,
                        detail: 9,
                    });
                }
                return StepOutcome::Complete;
            }
            let input = BoundedValueRef::new(
                ValueRef {
                    slot: 0,
                    generation: 1,
                    byte_len: 0,
                },
                0,
            )
            .unwrap();
            if io
                .request_host_call(RequestId(4), HostCallId(2), input)
                .is_err()
            {
                StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 9,
                })
            } else {
                StepOutcome::Progress
            }
        }
    }

    #[test]
    fn exact_single_source_uses_the_shared_step_and_sign_contracts() {
        let signs = SingleSourceSignLog::new();
        let mut executor =
            SingleSourceExecutor::new(Source, signs, NodeId(0), HostCallId(2), 0, 1, 3).unwrap();
        assert_eq!(executor.start().unwrap().request, RequestId(4));
        let value = BoundedValueRef::new(
            ValueRef {
                slot: 1,
                generation: 1,
                byte_len: 1,
            },
            1,
        )
        .unwrap();
        let output = executor
            .complete(
                RequestId(4),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(value),
                    failure: None,
                },
            )
            .unwrap();
        assert_eq!(output.port, PortId(0));
        assert_eq!(executor.signs().len(), 3);
    }
}
