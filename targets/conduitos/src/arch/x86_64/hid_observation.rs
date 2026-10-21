//! Fixed Root-only HID observations for source-pinned instrumented comparisons.
//!
//! Poll calls are observed executions, not the configured maximum. Raw TSC
//! ticks have no inferred frequency. Atomic counter overhead is part of both
//! instrumented comparison arms. Records stop after a finite sample budget;
//! this is not evidence of semantic Play acceptance, IRQ delivery or CPU time.
use core::{
    fmt::Write,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

use super::{hid::REPORT_BUFFERS, serial, usb::UsbDevice};

const SAMPLE_LIMIT: u64 = 16;
static SUBMISSIONS: AtomicU64 = AtomicU64::new(0);
static POLLS: AtomicU64 = AtomicU64::new(0);
static EMPTY_POLLS: AtomicU64 = AtomicU64::new(0);
static COMPLETIONS: AtomicU64 = AtomicU64::new(0);
static ACCEPTED: AtomicU64 = AtomicU64::new(0);
static OVERFLOWED: AtomicBool = AtomicBool::new(false);
static SEQUENCES: [AtomicU64; REPORT_BUFFERS] =
    [const { AtomicU64::new(u64::MAX) }; REPORT_BUFFERS];
static SUBMITTED_AT: [AtomicU64; REPORT_BUFFERS] = [const { AtomicU64::new(0) }; REPORT_BUFFERS];
static COMPLETED_AT: [AtomicU64; REPORT_BUFFERS] = [const { AtomicU64::new(0) }; REPORT_BUFFERS];

fn increment(counter: &AtomicU64) -> u64 {
    let previous = counter.fetch_add(1, Ordering::Relaxed);
    if previous == u64::MAX {
        OVERFLOWED.store(true, Ordering::Relaxed);
    }
    previous
}

pub(super) fn submitted(start: usize, end: usize) {
    let ticks = crate::native_observation::counter_ticks().unwrap_or(0);
    for sequence in start..end {
        let slot = sequence % REPORT_BUFFERS;
        SUBMITTED_AT[slot].store(ticks, Ordering::Relaxed);
        COMPLETED_AT[slot].store(0, Ordering::Relaxed);
        SEQUENCES[slot].store(sequence as u64, Ordering::Relaxed);
        increment(&SUBMISSIONS);
    }
}

pub(super) fn poll() {
    increment(&POLLS);
}
pub(super) fn empty_poll() {
    increment(&EMPTY_POLLS);
}
pub(super) fn completed(sequence: usize) {
    let slot = sequence % REPORT_BUFFERS;
    if SEQUENCES[slot].load(Ordering::Relaxed) == sequence as u64 {
        COMPLETED_AT[slot].store(
            crate::native_observation::counter_ticks().unwrap_or(0),
            Ordering::Relaxed,
        );
    }
    increment(&COMPLETIONS);
}

/// Called only after the existing boot-report parser and transition validator.
/// This label intentionally stops before portable queue/Play admission.
pub(super) fn accepted(sequence: usize, device: &UsbDevice) {
    let accepted_ticks = crate::native_observation::counter_ticks().unwrap_or(0);
    let sample = increment(&ACCEPTED);
    if sample >= SAMPLE_LIMIT {
        if sample == SAMPLE_LIMIT {
            let _ = serial::append_boot_diagnostic(
                b"CONDUIT_HID_WAIT_OBSERVATION_END {\"reason\":\"sample-limit\",\"sample_limit\":16}\n",
            );
        }
        return;
    }
    let slot = sequence % REPORT_BUFFERS;
    let exact_sequence = SEQUENCES[slot].load(Ordering::Relaxed) == sequence as u64;
    let submitted_ticks = SUBMITTED_AT[slot].load(Ordering::Relaxed);
    let completed_ticks = COMPLETED_AT[slot].load(Ordering::Relaxed);
    let mut record = crate::sign_format::FixedText::new();
    let formatted = writeln!(
        record,
        "CONDUIT_HID_WAIT_OBSERVATION {{\"schema\":\"conduit.conduitos/hid-wait-observation@1\",\"route\":\"bounded-poll\",\"clock\":\"x86-tsc-lfence\",\"frequency_hz\":null,\"stage\":\"validated-boot-report\",\"sequence\":{},\"root_port\":{},\"slot\":{},\"attachment_epoch\":{},\"exact_observed_sequence\":{},\"counter_reversed\":{},\"submitted_ticks\":{},\"completed_ticks\":{},\"validated_ticks\":{},\"poll_calls\":{},\"empty_poll_calls\":{},\"submitted_reports\":{},\"completed_reports\":{},\"validated_reports\":{},\"counter_overflow\":{},\"irq_ticks\":null,\"wake_ticks\":null,\"cpu_time\":null,\"sample_limit\":{}}}",
        sequence,
        device.root_port,
        device.slot,
        device.attachment_epoch,
        exact_sequence,
        completed_ticks < submitted_ticks || accepted_ticks < completed_ticks,
        submitted_ticks,
        completed_ticks,
        accepted_ticks,
        POLLS.load(Ordering::Relaxed),
        EMPTY_POLLS.load(Ordering::Relaxed),
        SUBMISSIONS.load(Ordering::Relaxed),
        COMPLETIONS.load(Ordering::Relaxed),
        sample + 1,
        OVERFLOWED.load(Ordering::Relaxed),
        SAMPLE_LIMIT
    );
    if formatted.is_ok() {
        let _ = serial::append_boot_diagnostic(record.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn counter_wrap_is_retained_as_invalid_comparison_evidence() {
        let counter = AtomicU64::new(u64::MAX);
        assert_eq!(increment(&counter), u64::MAX);
        assert_eq!(counter.load(Ordering::Relaxed), 0);
        assert!(OVERFLOWED.load(Ordering::Relaxed));
    }
}
