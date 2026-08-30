//! Fixed one-operation source profile for tiny assigned fragments.
//!
//! This is a specialization of the same [`Operation`] protocol used by the
//! full scheduler. It is valid only for a fragment containing one source, one
//! host operation, one output Port, and no local or remote Cords. Any other
//! shape must be refused before construction.

use crate::scheduler::HostOperationRequest;
use crate::{
    BoundedValueRef, HostOperationId, HostOperationOutcome, KernelEventKind, NodeId, Operation,
    OperationAction, OperationInput, PortId, RemoteLifecycleIdentity, RequestId, SignError,
    SignSink, StorageError, ValueRef,
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
    OperationFailed(u16),
    SignCapacity,
    AlreadyStarted,
    AlreadyTerminal,
}

pub struct SingleSourceExecutor<O, E> {
    operation: O,
    signs: E,
    node: NodeId,
    operation_id: HostOperationId,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
    maximum_step_work: u16,
    request: Option<RequestId>,
    terminal: bool,
}

impl<O: Operation, E: SignSink> SingleSourceExecutor<O, E> {
    pub fn new(
        operation: O,
        signs: E,
        node: NodeId,
        operation_id: HostOperationId,
        maximum_input_bytes: u32,
        maximum_output_bytes: u32,
        maximum_step_work: u16,
    ) -> Result<Self, SingleSourceRefusal> {
        if maximum_step_work < 3 || maximum_output_bytes == 0 {
            return Err(SingleSourceRefusal::InvalidBound);
        }
        Ok(Self {
            operation,
            signs,
            node,
            operation_id,
            maximum_input_bytes,
            maximum_output_bytes,
            maximum_step_work,
            request: None,
            terminal: false,
        })
    }

    pub fn start(&mut self) -> Result<HostOperationRequest, SingleSourceRefusal> {
        if self.terminal {
            return Err(SingleSourceRefusal::AlreadyTerminal);
        }
        if self.request.is_some() {
            return Err(SingleSourceRefusal::AlreadyStarted);
        }
        let OperationAction::RequestHostOperation {
            request,
            operation,
            input,
        } = self.operation.start()
        else {
            return Err(SingleSourceRefusal::InvalidStart);
        };
        if operation != self.operation_id || input.value.byte_len > self.maximum_input_bytes {
            return Err(SingleSourceRefusal::InvalidStart);
        }
        self.request = Some(request);
        self.signs
            .record(
                self.node,
                None,
                Some(request),
                KernelEventKind::HostOperationRequested,
            )
            .map_err(|_| SingleSourceRefusal::SignCapacity)?;
        Ok(HostOperationRequest {
            node: self.node,
            request,
            operation,
            input,
        })
    }

    pub fn complete(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
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
                KernelEventKind::HostOperationCompleted,
            )
            .map_err(|_| SingleSourceRefusal::SignCapacity)?;
        let first = self
            .operation
            .resume(OperationInput::HostOperationCompleted { request, outcome });
        let OperationAction::Emit { port, value } = first else {
            return match first {
                OperationAction::Fail(failure) => {
                    self.terminal = true;
                    Err(SingleSourceRefusal::OperationFailed(failure.detail))
                }
                _ => Err(SingleSourceRefusal::InvalidCompletion),
            };
        };
        let bounded = BoundedValueRef::new(value, self.maximum_output_bytes)
            .map_err(|_| SingleSourceRefusal::InvalidCompletion)?;
        if !matches!(self.operation.advance(), OperationAction::Complete) {
            return Err(SingleSourceRefusal::InvalidCompletion);
        }
        let _ = self.maximum_step_work;
        self.terminal = true;
        self.signs
            .record(
                self.node,
                Some(port),
                None,
                KernelEventKind::OperationCompleted,
            )
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
    use crate::{Failure, FailureCode, HostOperationDisposition};

    #[derive(Clone, Copy)]
    struct Source;

    impl Operation for Source {
        fn start(&mut self) -> OperationAction {
            OperationAction::RequestHostOperation {
                request: RequestId(4),
                operation: HostOperationId(2),
                input: BoundedValueRef::new(
                    ValueRef {
                        slot: 0,
                        generation: 1,
                        byte_len: 0,
                    },
                    0,
                )
                .unwrap(),
            }
        }

        fn resume(&mut self, input: OperationInput) -> OperationAction {
            match input {
                OperationInput::HostOperationCompleted {
                    request: RequestId(4),
                    outcome:
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: Some(output),
                            failure: None,
                        },
                } => OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                },
                _ => OperationAction::Fail(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 9,
                }),
            }
        }

        fn advance(&mut self) -> OperationAction {
            OperationAction::Complete
        }

        fn cancel(&mut self) {}
    }

    #[test]
    fn exact_single_source_uses_the_shared_operation_and_sign_contracts() {
        let signs = SingleSourceSignLog::new();
        let mut executor =
            SingleSourceExecutor::new(Source, signs, NodeId(0), HostOperationId(2), 0, 1, 3)
                .unwrap();
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
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(value),
                    failure: None,
                },
            )
            .unwrap();
        assert_eq!(output.port, PortId(0));
        assert_eq!(executor.signs().len(), 3);
    }
}
