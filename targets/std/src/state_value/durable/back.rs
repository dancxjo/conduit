use super::{
    decode_receipt, digest, DurableStateBinding, DurableStateRefusal, PROTOCOL_VERSION,
    RECOVERY_ABSENT, RECOVERY_HEADER_BYTES, RECOVERY_PRESENT,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};

enum Phase {
    RecoveringMetadata,
    RecoveringValue {
        generation: u64,
        digest: [u8; 32],
    },
    Current,
    Committing {
        request: RequestId,
        generation: u64,
        digest: [u8; 32],
    },
    Terminal,
}

/// Installed Step driver for the transactional boundary. Recovery request 0
/// obtains bounded generation/digest metadata; request 1 binds that exact
/// metadata to the recovered hosted value. Commit requests begin at 2.
pub struct DurableStateBack {
    binding: DurableStateBinding,
    initial: Option<ValueRef>,
    recovery_metadata_probe: ValueRef,
    generation: u64,
    next_request: u32,
    pending_value: Option<ValueRef>,
    phase: Phase,
}

impl DurableStateBack {
    pub fn new(
        binding: DurableStateBinding,
        initial: ValueRef,
        recovery_metadata_probe: ValueRef,
    ) -> Result<Self, DurableStateRefusal> {
        binding.validate()?;
        if initial.byte_len > binding.maximum_value_bytes || recovery_metadata_probe.byte_len != 1 {
            return Err(DurableStateRefusal::ValueTooLarge);
        }
        Ok(Self {
            binding,
            initial: Some(initial),
            recovery_metadata_probe,
            generation: 0,
            next_request: 2,
            pending_value: None,
            phase: Phase::RecoveringMetadata,
        })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) const fn expects_recovered_value(&self) -> bool {
        matches!(self.phase, Phase::RecoveringValue { .. })
    }
}

impl<const PORTS: usize> StepBack<PORTS> for DurableStateBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self.phase {
            Phase::RecoveringMetadata => self.recover_metadata(io, bytes),
            Phase::RecoveringValue { generation, digest } => {
                self.recover_value(io, bytes, generation, digest)
            }
            Phase::Current => self.current(io, bytes),
            Phase::Committing {
                request,
                generation,
                digest,
            } => self.finish_commit(io, bytes, request, generation, digest),
            Phase::Terminal => fail(FailureCode::InvalidLifecycle, 12),
        }
    }

    fn cancel(&mut self) {
        self.pending_value = None;
        self.phase = Phase::Terminal;
    }

    fn retains_host_call_input(&self, _request: RequestId, value: ValueRef) -> bool {
        self.pending_value == Some(value)
    }
}

impl DurableStateBack {
    fn recover_metadata<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        let Some((request, outcome)) = io.host_completion() else {
            let input = BoundedValueRef::new(
                self.recovery_metadata_probe,
                self.recovery_metadata_probe.byte_len,
            )
            .expect("probe bound equals prepared probe");
            io.request_host_call(RequestId(0), HostCallId(1), input)
                .expect("prepared recovery Host Call");
            return StepOutcome::Progress;
        };
        if request != RequestId(0) {
            return fail(FailureCode::InvalidLifecycle, 1);
        }
        if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
            return host_failure(outcome, 2);
        }
        let Some(metadata_value) = outcome.output else {
            return fail(FailureCode::InvalidInput, 2);
        };
        let Some(metadata) = bytes.host_output() else {
            return host_failure(outcome, 2);
        };
        match decode_recovery_metadata(metadata) {
            Ok(None) => {
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.consume_host_completion()
                    .expect("observed absent recovery receipt");
                let initial = self.initial.take().expect("prepared initial State value");
                io.send(PortId(0), initial)
                    .expect("ready initial State output");
                self.generation = 0;
                self.phase = Phase::Current;
                StepOutcome::Progress
            }
            Ok(Some((generation, digest))) => {
                io.consume_host_completion()
                    .expect("observed recovery metadata");
                io.request_host_call(RequestId(1), HostCallId(1), metadata_value)
                    .expect("prepared recovery value Host Call");
                self.phase = Phase::RecoveringValue { generation, digest };
                StepOutcome::Progress
            }
            Err(_) => fail(FailureCode::InvalidInput, 3),
        }
    }

    fn recover_value<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        bytes: &StepInputBytes<'_, PORTS>,
        generation: u64,
        expected_digest: [u8; 32],
    ) -> StepOutcome {
        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if request != RequestId(1) {
            return fail(FailureCode::InvalidLifecycle, 13);
        }
        if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
            return host_failure(outcome, 14);
        }
        let (Some(value), Some(value_bytes)) = (outcome.output, bytes.host_output()) else {
            return fail(FailureCode::InvalidInput, 15);
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if value.value.byte_len > self.binding.maximum_value_bytes
            || value.value.byte_len as usize != value_bytes.len()
            || digest(value_bytes) != expected_digest
        {
            return fail(FailureCode::InvalidInput, 16);
        }
        io.consume_host_completion()
            .expect("observed exact recovered value");
        io.discard(self.initial.take().expect("unused initial State value"))
            .expect("discard replaced initial State value");
        io.send(PortId(0), value.value)
            .expect("ready recovered State output");
        self.generation = generation;
        self.phase = Phase::Current;
        StepOutcome::Progress
    }

    fn current<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len > self.binding.maximum_value_bytes {
                return fail(FailureCode::StateCapacityExhausted, 4);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(value_bytes) = bytes.input(PortId(0)) else {
                return fail(FailureCode::InvalidInput, 4);
            };
            let Some(generation) = self.generation.checked_add(1) else {
                return fail(FailureCode::StorageExhausted, 5);
            };
            let request = RequestId(self.next_request);
            let Some(next_request) = self.next_request.checked_add(1) else {
                return fail(FailureCode::StorageExhausted, 6);
            };
            let owned = io
                .borrow_input_for_call(PortId(0))
                .expect("pin present durable State transition");
            io.request_host_call(
                request,
                HostCallId(0),
                BoundedValueRef::new(owned, self.binding.maximum_value_bytes)
                    .expect("checked durable State value bound"),
            )
            .expect("prepared commit Host Call");
            self.pending_value = Some(owned);
            self.next_request = next_request;
            self.phase = Phase::Committing {
                request,
                generation,
                digest: digest(value_bytes),
            };
            StepOutcome::Progress
        } else if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).expect("observed State close");
            self.phase = Phase::Terminal;
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }

    fn finish_commit<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        bytes: &StepInputBytes<'_, PORTS>,
        request: RequestId,
        generation: u64,
        expected_digest: [u8; 32],
    ) -> StepOutcome {
        let Some((actual_request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if actual_request != request {
            return fail(FailureCode::InvalidLifecycle, 7);
        }
        if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
            return host_failure(outcome, 8);
        }
        let Some(receipt) = bytes.host_output() else {
            return fail(FailureCode::InvalidInput, 9);
        };
        if decode_receipt(receipt) != Some((generation, expected_digest)) {
            return fail(FailureCode::InvalidInput, 10);
        }
        let Some(value) = self.pending_value else {
            return fail(FailureCode::InvalidLifecycle, 11);
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.consume_host_completion()
            .expect("observed exact commit receipt");
        io.consume(PortId(0))
            .expect("consume commit-receipted State transition");
        io.send(PortId(0), value)
            .expect("ready committed State output");
        self.pending_value = None;
        self.generation = generation;
        self.phase = Phase::Current;
        StepOutcome::Progress
    }
}

fn decode_recovery_metadata(bytes: &[u8]) -> Result<Option<(u64, [u8; 32])>, DurableStateRefusal> {
    if bytes == [PROTOCOL_VERSION, RECOVERY_ABSENT] {
        return Ok(None);
    }
    if bytes.len() != RECOVERY_HEADER_BYTES
        || bytes[0] != PROTOCOL_VERSION
        || bytes[1] != RECOVERY_PRESENT
    {
        return Err(DurableStateRefusal::InvalidReceipt);
    }
    let generation = u64::from_be_bytes(
        bytes[2..10]
            .try_into()
            .map_err(|_| DurableStateRefusal::InvalidReceipt)?,
    );
    if generation == 0 {
        return Err(DurableStateRefusal::InvalidReceipt);
    }
    let expected: [u8; 32] = bytes[10..42]
        .try_into()
        .map_err(|_| DurableStateRefusal::InvalidReceipt)?;
    Ok(Some((generation, expected)))
}

fn host_failure(outcome: conduit_kernel::HostCallOutcome, detail: u16) -> StepOutcome {
    match (outcome.disposition, outcome.failure) {
        (HostCallDisposition::Cancelled, _) => fail(FailureCode::Cancelled, detail),
        (HostCallDisposition::Failed, Some(failure)) => StepOutcome::Fail(failure),
        _ => fail(FailureCode::HostCallFailed, detail),
    }
}

const fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
