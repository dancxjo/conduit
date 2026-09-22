//! Allocation-independent execution and equality proof for both comparison Plans.

use alloc::vec::Vec;
use conduit_kernel::scheduler::SchedulerStatus;

use crate::{
    composition::{MachineRunError, MachineRunReceipt},
    machine::{IdleBase, InterruptBase, MonotonicClockBase, SerialBase},
    tour_comparison_kernel::{ComparisonKernel, ComparisonNodeKind},
    tour_comparison_plan::PreparedComparisonPlans,
};

const MAXIMUM_KERNEL_STEPS: u32 = 256;
const UNIT_MILLIS: u16 = 40;

pub(crate) struct ComparisonScratch {
    output: Vec<u8>,
    auxiliary: Vec<u8>,
    direct: Vec<u8>,
    recursive: Vec<u8>,
}

impl ComparisonScratch {
    pub(crate) fn prepared() -> Self {
        Self {
            output: Vec::with_capacity(conduit_text::MAXIMUM_MORSE_PATTERN_BYTES),
            auxiliary: Vec::with_capacity(conduit_text::MAXIMUM_MORSE_PATTERN_BYTES),
            direct: Vec::with_capacity(conduit_text::MAXIMUM_MORSE_PATTERN_BYTES),
            recursive: Vec::with_capacity(conduit_text::MAXIMUM_MORSE_PATTERN_BYTES),
        }
    }
}

pub(crate) fn run<C, S, I, D>(
    prepared: &mut PreparedComparisonPlans,
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
    let started = clock.now();
    let disabled_state = interrupts.disable();
    if interrupts.is_enabled() {
        return Err(MachineRunError::InterruptBaseFailure);
    }
    interrupts.restore(disabled_state);
    let _ = interrupts.disable();
    let direct = run_one(
        &mut prepared.direct_kernel,
        &mut prepared.scratch,
        serial,
        idle,
        true,
    )?;
    let recursive = run_one(
        &mut prepared.recursive_kernel,
        &mut prepared.scratch,
        serial,
        idle,
        false,
    )?;
    if prepared.scratch.direct.is_empty() || prepared.scratch.direct != prepared.scratch.recursive {
        return Err(MachineRunError::KernelFailure);
    }
    Ok(MachineRunReceipt {
        logical_operations: 10,
        decisions: direct.decisions + recursive.decisions,
        kernel_signs: direct.signs.saturating_add(recursive.signs),
        timer_irq_wakes: 0,
        idle_entries: idle.idle_count(),
        serial_presentations: serial.presentation_count(),
        clock_monotonic: clock.now() >= started,
        pending_host_calls: 0,
        overlap_witness: false,
        timer_pending_during_text_progress: false,
        physical_parallelism: false,
    })
}

struct RunStats {
    decisions: u32,
    signs: u16,
}

fn run_one<S: SerialBase, D: IdleBase>(
    kernel: &mut ComparisonKernel,
    scratch: &mut ComparisonScratch,
    serial: &mut S,
    _idle: &mut D,
    direct: bool,
) -> Result<RunStats, MachineRunError> {
    for _ in 0..MAXIMUM_KERNEL_STEPS {
        while let Some(request) = kernel.next_host_request() {
            let kind = kernel.request_kind(&request);
            if kind == ComparisonNodeKind::Indicator {
                let value = kernel
                    .host_value(request.input.value)
                    .map_err(|_| MachineRunError::KernelFailure)?;
                let observed = if direct {
                    &mut scratch.direct
                } else {
                    &mut scratch.recursive
                };
                observed.clear();
                observed.extend_from_slice(value);
                serial
                    .present(value)
                    .map_err(|_| MachineRunError::SerialBaseFailure)?;
                kernel
                    .complete_presentation(request)
                    .map_err(|_| MachineRunError::KernelFailure)?;
            } else {
                let maximum = output_bound(kind);
                {
                    let input = kernel
                        .host_value(request.input.value)
                        .map_err(|_| MachineRunError::KernelFailure)?;
                    transform(kind, input, scratch)?;
                }
                kernel
                    .complete_value(request, &scratch.output, maximum)
                    .map_err(|_| MachineRunError::KernelFailure)?;
            }
        }
        match kernel.step().map_err(|_| MachineRunError::KernelFailure)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => {
                if kernel.pending_host_calls() == 0 {
                    return Err(MachineRunError::FalseIdle);
                }
                // Comparison Host Calls are local and synchronous; let
                // the next ownership-loop iteration service them directly.
                continue;
            }
            SchedulerStatus::Drained => {
                return Ok(RunStats {
                    decisions: kernel.decisions(),
                    signs: kernel.sign_count(),
                });
            }
            SchedulerStatus::Cancelled => return Err(MachineRunError::KernelFailure),
        }
    }
    Err(MachineRunError::StepLimitExceeded)
}

fn transform(
    kind: ComparisonNodeKind,
    input: &[u8],
    scratch: &mut ComparisonScratch,
) -> Result<(), MachineRunError> {
    scratch.output.clear();
    match kind {
        ComparisonNodeKind::DirectMorse => {
            let text =
                core::str::from_utf8(input).map_err(|_| MachineRunError::TextMalformedUtf8)?;
            conduit_text::morse_characters_from_text_into(text, &mut scratch.output)
                .map_err(|_| MachineRunError::KernelFailure)?;
            scratch.auxiliary.clear();
            conduit_text::morse_lookup_characters_into(&scratch.output, &mut scratch.auxiliary)
                .map_err(|_| MachineRunError::KernelFailure)?;
            scratch.output.clear();
            conduit_text::morse_intersperse_gaps_into(&scratch.auxiliary, &mut scratch.output)
                .map_err(|_| MachineRunError::KernelFailure)?;
            scratch.auxiliary.clear();
            conduit_text::morse_flatten_groups_into(&scratch.output, &mut scratch.auxiliary)
                .map_err(|_| MachineRunError::KernelFailure)?;
            scratch.output.clear();
            conduit_text::morse_symbols_to_pattern_into(
                &scratch.auxiliary,
                UNIT_MILLIS,
                &mut scratch.output,
            )
            .map_err(|_| MachineRunError::KernelFailure)
        }
        ComparisonNodeKind::Characters => {
            let text =
                core::str::from_utf8(input).map_err(|_| MachineRunError::TextMalformedUtf8)?;
            conduit_text::morse_characters_from_text_into(text, &mut scratch.output)
                .map_err(|_| MachineRunError::KernelFailure)
        }
        ComparisonNodeKind::Lookup => {
            conduit_text::morse_lookup_characters_into(input, &mut scratch.output)
                .map_err(|_| MachineRunError::KernelFailure)
        }
        ComparisonNodeKind::Intersperse => {
            conduit_text::morse_intersperse_gaps_into(input, &mut scratch.output)
                .map_err(|_| MachineRunError::KernelFailure)
        }
        ComparisonNodeKind::Flatten => {
            conduit_text::morse_flatten_groups_into(input, &mut scratch.output)
                .map_err(|_| MachineRunError::KernelFailure)
        }
        ComparisonNodeKind::SymbolsToPattern => {
            conduit_text::morse_symbols_to_pattern_into(input, UNIT_MILLIS, &mut scratch.output)
                .map_err(|_| MachineRunError::KernelFailure)
        }
        ComparisonNodeKind::Literal | ComparisonNodeKind::Indicator => {
            Err(MachineRunError::UnexpectedHostCall)
        }
    }
}

const fn output_bound(kind: ComparisonNodeKind) -> u32 {
    match kind {
        ComparisonNodeKind::DirectMorse | ComparisonNodeKind::SymbolsToPattern => {
            conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32
        }
        ComparisonNodeKind::Characters => conduit_text::MAXIMUM_MORSE_CHARACTERS_BYTES as u32,
        ComparisonNodeKind::Lookup => conduit_text::MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES as u32,
        ComparisonNodeKind::Intersperse => conduit_text::MAXIMUM_MORSE_GAPPED_GROUPS_BYTES as u32,
        ComparisonNodeKind::Flatten => conduit_text::MAXIMUM_MORSE_SYMBOLS_BYTES as u32,
        ComparisonNodeKind::Literal | ComparisonNodeKind::Indicator => 0,
    }
}
