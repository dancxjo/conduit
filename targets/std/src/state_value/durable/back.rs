use super::{
    decode_receipt, digest, DurableStateBinding, DurableStateRefusal, PROTOCOL_VERSION,
    RECOVERY_ABSENT, RECOVERY_HEADER_BYTES, RECOVERY_PRESENT,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostCallDisposition, HostCallId, PortId,
    RequestId, ValueRef,
};

enum Phase {
    Recovering,
    Current,
    Committing {
        request: RequestId,
        generation: u64,
        digest: [u8; 32],
    },
    Terminal,
}

/// Ordinary Step driver for the transactional boundary. Call 0 recovers and
/// call 1 commits. This is groundwork, not an installed or advertised Back.
pub struct DurableStateBack {
    binding: DurableStateBinding,
    initial: CanonicalValue,
    recovery_probe: ValueRef,
    generation: u64,
    next_request: u32,
    phase: Phase,
}

impl DurableStateBack {
    pub fn new(
        binding: DurableStateBinding,
        initial: &[u8],
        recovery_probe: ValueRef,
    ) -> Result<Self, DurableStateRefusal> {
        binding.validate()?;
        if initial.len() > binding.maximum_value_bytes as usize {
            return Err(DurableStateRefusal::ValueTooLarge);
        }
        Ok(Self {
            binding,
            initial: CanonicalValue::new(initial)
                .map_err(|_| DurableStateRefusal::ValueTooLarge)?,
            recovery_probe,
            generation: 0,
            next_request: 1,
            phase: Phase::Recovering,
        })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

impl<const PORTS: usize> StepBack<PORTS> for DurableStateBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self.phase {
            Phase::Recovering => self.recover(io, bytes),
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
        self.phase = Phase::Terminal;
    }
}

impl DurableStateBack {
    fn recover<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        let Some((request, outcome)) = io.host_completion() else {
            let input = BoundedValueRef::new(self.recovery_probe, self.recovery_probe.byte_len)
                .expect("probe bound equals prepared probe");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("prepared recovery Host Call");
            return StepOutcome::Progress;
        };
        if request != RequestId(0) {
            return fail(FailureCode::InvalidLifecycle, 1);
        }
        let Some(response) = bytes.host_output() else {
            return host_failure(outcome, 2);
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (generation, value) = match decode_recovery(response, &self.binding) {
            Ok(value) => value.unwrap_or((0, self.initial)),
            Err(_) => return fail(FailureCode::InvalidInput, 3),
        };
        io.consume_host_completion()
            .expect("observed recovery receipt");
        io.send_canonical(PortId(0), value)
            .expect("ready State output");
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
            io.request_host_call(
                request,
                HostCallId(1),
                BoundedValueRef::new(value, self.binding.maximum_value_bytes)
                    .expect("checked durable State value bound"),
            )
            .expect("prepared commit Host Call");
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
        let Some(value) = io.input(PortId(0)) else {
            return fail(FailureCode::InvalidLifecycle, 11);
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.consume_host_completion()
            .expect("observed exact commit receipt");
        io.consume(PortId(0)).expect("commit-receipted State input");
        io.send(PortId(0), value)
            .expect("ready committed State output");
        self.generation = generation;
        self.phase = Phase::Current;
        StepOutcome::Progress
    }
}

fn decode_recovery(
    bytes: &[u8],
    binding: &DurableStateBinding,
) -> Result<Option<(u64, CanonicalValue)>, DurableStateRefusal> {
    if bytes == [PROTOCOL_VERSION, RECOVERY_ABSENT] {
        return Ok(None);
    }
    if bytes.len() < RECOVERY_HEADER_BYTES
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
    let value = &bytes[42..];
    if value.len() > binding.maximum_value_bytes as usize || digest(value) != expected {
        return Err(DurableStateRefusal::Corrupt);
    }
    Ok(Some((
        generation,
        CanonicalValue::new(value).map_err(|_| DurableStateRefusal::ValueTooLarge)?,
    )))
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
