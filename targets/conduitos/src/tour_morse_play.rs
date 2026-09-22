//! Allocation-independent ownership loop for the shared native fan-out Tour Play.

use alloc::vec::Vec;
use conduit_kernel::scheduler::SchedulerStatus;

use crate::{
    composition::{MachineRunError, MachineRunReceipt},
    machine::{IdleBase, InterruptBase, MonotonicClockBase, SerialBase},
    tour_morse_plan::PreparedTourMorsePlay,
};

const MAXIMUM_KERNEL_STEPS: u32 = 192;

pub struct MorseScratch {
    first: Vec<u8>,
    second: Vec<u8>,
    expected: Vec<u8>,
}

impl MorseScratch {
    pub fn prepared() -> Result<Self, MachineRunError> {
        let mut scratch = Self {
            first: Vec::with_capacity(conduit_text::MAXIMUM_MORSE_PATTERN_BYTES),
            second: Vec::with_capacity(conduit_text::MAXIMUM_MORSE_PATTERN_BYTES),
            expected: Vec::with_capacity(conduit_text::MAXIMUM_MORSE_PATTERN_BYTES),
        };
        scratch.encode(b"sos", 80)?;
        scratch.expected.extend_from_slice(&scratch.first);
        Ok(scratch)
    }

    fn encode(&mut self, input: &[u8], unit_millis: u16) -> Result<&[u8], MachineRunError> {
        let text = core::str::from_utf8(input).map_err(|_| MachineRunError::TextMalformedUtf8)?;
        self.first.clear();
        conduit_text::morse_characters_from_text_into(text, &mut self.first)
            .map_err(|_| MachineRunError::KernelFailure)?;
        self.second.clear();
        conduit_text::morse_lookup_characters_into(&self.first, &mut self.second)
            .map_err(|_| MachineRunError::KernelFailure)?;
        self.first.clear();
        conduit_text::morse_intersperse_gaps_into(&self.second, &mut self.first)
            .map_err(|_| MachineRunError::KernelFailure)?;
        self.second.clear();
        conduit_text::morse_flatten_groups_into(&self.first, &mut self.second)
            .map_err(|_| MachineRunError::KernelFailure)?;
        self.first.clear();
        conduit_text::morse_symbols_to_pattern_into(&self.second, unit_millis, &mut self.first)
            .map_err(|_| MachineRunError::KernelFailure)?;
        Ok(&self.first)
    }

    fn is_expected(&self, pattern: &[u8]) -> bool {
        pattern == self.expected
    }
}

pub fn run<C, S, I, D>(
    prepared: &mut PreparedTourMorsePlay,
    clock: &mut C,
    serial: &mut S,
    interrupts: &mut I,
    idle: &mut D,
) -> Result<MachineRunReceipt, MachineRunError>
where
    C: MonotonicClockBase,
    S: SerialBase,
    I: InterruptBase,
    D: IdleBase,
{
    let PreparedTourMorsePlay {
        kernel, scratch, ..
    } = prepared;
    let started = clock.now();
    let disabled_state = interrupts.disable();
    if interrupts.is_enabled() {
        return Err(MachineRunError::InterruptBaseFailure);
    }
    interrupts.restore(disabled_state);
    let _ = interrupts.disable();
    for _ in 0..MAXIMUM_KERNEL_STEPS {
        while let Some(request) = kernel.next_host_request() {
            if kernel.is_upper_request(&request) {
                let output = {
                    let input = kernel
                        .host_value(request.input.value)
                        .map_err(|_| MachineRunError::KernelFailure)?;
                    crate::text_upper::uppercase(input).map_err(|error| match error {
                        crate::text_upper::UppercaseError::MalformedUtf8 => {
                            MachineRunError::TextMalformedUtf8
                        }
                        crate::text_upper::UppercaseError::OutputOverflow => {
                            MachineRunError::TextOutputOverflow
                        }
                    })?
                };
                kernel
                    .complete_upper(request, output.as_bytes())
                    .map_err(|_| MachineRunError::KernelFailure)?;
            } else if kernel.is_morse_request(&request) {
                let pattern = {
                    let input = kernel
                        .host_value(request.input.value)
                        .map_err(|_| MachineRunError::KernelFailure)?;
                    scratch.encode(input, 80)?
                };
                kernel
                    .complete_morse(request, pattern)
                    .map_err(|_| MachineRunError::KernelFailure)?;
            } else if kernel.is_text_presentation_request(&request)
                || kernel.is_indicator_request(&request)
            {
                let text = kernel.is_text_presentation_request(&request);
                let value = kernel
                    .host_value(request.input.value)
                    .map_err(|_| MachineRunError::KernelFailure)?;
                if (text && value != b"SOS") || (!text && !scratch.is_expected(value)) {
                    return Err(MachineRunError::SerialBaseFailure);
                }
                serial
                    .present(value)
                    .map_err(|_| MachineRunError::SerialBaseFailure)?;
                kernel
                    .complete_presentation(request)
                    .map_err(|_| MachineRunError::KernelFailure)?;
            } else {
                return Err(MachineRunError::UnexpectedHostCall);
            }
        }
        match kernel.step().map_err(|_| MachineRunError::KernelFailure)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => {
                if kernel.pending_host_calls() == 0 {
                    return Err(MachineRunError::FalseIdle);
                }
                // Every Host Call in this runner is synchronously
                // serviceable at the top of the loop.
                continue;
            }
            SchedulerStatus::Drained => {
                return Ok(MachineRunReceipt {
                    logical_operations: 5,
                    decisions: kernel.decisions(),
                    kernel_signs: kernel.sign_count(),
                    timer_irq_wakes: 0,
                    idle_entries: idle.idle_count(),
                    serial_presentations: serial.presentation_count(),
                    clock_monotonic: clock.now() >= started,
                    pending_host_calls: kernel.pending_host_calls() as u8,
                    overlap_witness: false,
                    timer_pending_during_text_progress: false,
                    physical_parallelism: false,
                });
            }
            SchedulerStatus::Cancelled => return Err(MachineRunError::KernelFailure),
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}
