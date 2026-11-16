use super::*;
use conduit_resource_snapshot::read_result::{Encoder, ReadResult};
use std::{
    cell::Cell,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
#[derive(Clone, Copy)]
enum Outcome {
    Delivered,
    Pressure,
    Unknown,
    NoCallback,
}
struct Lease {
    frames: u32,
    timeout: Duration,
    bytes: Vec<u8>,
    outcome: Outcome,
    calls: Arc<AtomicUsize>,
    acks: Arc<AtomicUsize>,
}
impl Lease {
    fn new(value: ReadResult<'_>, frames: u32) -> Self {
        Self {
            frames,
            timeout: Duration::from_secs(1),
            bytes: Encoder::new().unwrap().encode(value).unwrap().to_vec(),
            outcome: Outcome::Delivered,
            calls: Arc::new(AtomicUsize::new(0)),
            acks: Arc::new(AtomicUsize::new(0)),
        }
    }
}
// SAFETY: in-memory test correspondence only; no actual native registration,
// descriptor, grant, membership, Start or durable-commit evidence is created.
unsafe impl OriginalRecordReadFore for Lease {
    fn remaining_frames(&self) -> u32 {
        self.frames
    }
    fn maximum_timeout(&self) -> Duration {
        self.timeout
    }
    fn exchange(
        &mut self,
        _: &[u8],
        _: &mut [u8],
        timeout: Duration,
        consume: impl FnOnce(&[u8]) -> Result<(), ()>,
    ) -> Result<(), WorkerFailure> {
        assert!(!timeout.is_zero());
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.frames -= 6;
        match self.outcome {
            Outcome::Pressure => return Err(WorkerFailure::Provider),
            Outcome::Unknown => return Err(WorkerFailure::TimedOut),
            Outcome::NoCallback => return Ok(()),
            Outcome::Delivered => {}
        }
        consume(&self.bytes).map_err(|_| WorkerFailure::Provider)?;
        self.acks.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn budget(count: u32) -> RecordReadBudget {
    RecordReadBudget {
        maximum_requests: count,
        timeout_per_request: Duration::from_millis(100),
    }
}
fn worker_budget() -> NativeWorkerBudget {
    NativeWorkerBudget {
        requested_stack_bytes: 2 * 1024 * 1024,
        maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 4096,
        capture_timeout: Duration::from_secs(1),
        cancellation_timeout: Duration::from_secs(1),
    }
}
fn prepared(lease: Lease, count: u32) -> ForeRecordReadWorker<Lease> {
    match ForeRecordReadWorker::prepare(lease, budget(count), worker_budget()) {
        Ok(w) => w,
        Err(_) => panic!("test preparation"),
    }
}
#[test]
fn finite_lifetime_and_original_frame_ceiling_survive_child_calls() {
    let mut worker = prepared(Lease::new(ReadResult::End(9), 12), 2);
    let mut out = [0; READ_RESULT_BYTES];
    let input = conduit_core::encode_count(9);
    for remaining in [1, 0] {
        let n = worker
            .perform(&input, &mut out, Duration::from_secs(1), || false)
            .unwrap();
        assert!(matches!(
            decode_for_index(9, &out[..n]),
            Ok(ReadResult::End(9))
        ));
        assert_eq!(worker.remaining_requests(), remaining);
    }
    assert_eq!(
        worker.perform(&input, &mut out, Duration::from_secs(1), || false),
        Err(WorkerFailure::Profile)
    );
    assert_eq!(worker.original.calls.load(Ordering::SeqCst), 2);
    assert!(worker.is_fenced());
}
#[test]
fn prepare_refuses_zero_overflow_and_unfunded_total_without_consuming_lease() {
    for (count, frames, reason) in [
        (0, 12, PreparationRefusal::ZeroRequests),
        (u32::MAX, u32::MAX, PreparationRefusal::FrameBudget),
        (3, 12, PreparationRefusal::FrameBudget),
    ] {
        let lease = Lease::new(ReadResult::End(0), frames);
        let calls = lease.calls.clone();
        match ForeRecordReadWorker::prepare(lease, budget(count), worker_budget()) {
            Err((_, r)) => assert_eq!(r, reason),
            Ok(_) => panic!("invalid budget accepted"),
        };
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn wrong_index_and_corruption_are_refused_before_transport_ack() {
    for malformed in [false, true] {
        let mut lease = Lease::new(
            ReadResult::Recorded {
                index: 3,
                captured_end: 8,
                record: &[1],
            },
            6,
        );
        if malformed {
            lease.bytes.clear();
            lease.bytes.extend_from_slice(b"wrong schema");
        }
        let mut worker = prepared(lease, 1);
        let mut out = [0; READ_RESULT_BYTES];
        assert_eq!(
            worker.perform(
                &conduit_core::encode_count(4),
                &mut out,
                Duration::from_secs(1),
                || false
            ),
            Err(WorkerFailure::Provider)
        );
        assert_eq!(worker.original.acks.load(Ordering::SeqCst), 0);
        assert!(worker.is_fenced());
    }
}
#[test]
fn ambiguous_delivered_pressure_and_unknown_fence_without_retry_or_ack() {
    for outcome in [Outcome::NoCallback, Outcome::Pressure, Outcome::Unknown] {
        let mut lease = Lease::new(ReadResult::End(0), 12);
        lease.outcome = outcome;
        let mut worker = prepared(lease, 2);
        let mut out = [0; READ_RESULT_BYTES];
        assert!(worker
            .perform(
                &conduit_core::encode_count(0),
                &mut out,
                Duration::from_secs(1),
                || false
            )
            .is_err());
        assert_eq!(worker.original.acks.load(Ordering::SeqCst), 0);
        assert_eq!(
            worker.perform(
                &conduit_core::encode_count(0),
                &mut out,
                Duration::from_secs(1),
                || false
            ),
            Err(WorkerFailure::Lost)
        );
        assert_eq!(worker.original.calls.load(Ordering::SeqCst), 1);
    }
}
#[test]
fn canonical_busy_remains_an_application_result_not_transport_pressure() {
    let mut worker = prepared(Lease::new(ReadResult::Busy, 6), 1);
    let mut out = [0; READ_RESULT_BYTES];
    let n = worker
        .perform(
            &conduit_core::encode_count(0),
            &mut out,
            Duration::from_secs(1),
            || false,
        )
        .unwrap();
    assert!(matches!(
        decode_for_index(0, &out[..n]),
        Ok(ReadResult::Busy)
    ));
    assert!(!worker.is_fenced());
}
#[test]
fn cancellation_preserves_lease_and_never_retries_even_after_delivery() {
    let mut before = prepared(Lease::new(ReadResult::End(0), 6), 1);
    let mut out = [0; READ_RESULT_BYTES];
    assert_eq!(
        before.perform(
            &conduit_core::encode_count(0),
            &mut out,
            Duration::from_secs(1),
            || true
        ),
        Err(WorkerFailure::Cancelled)
    );
    assert_eq!(before.original.calls.load(Ordering::SeqCst), 0);
    let mut after = prepared(Lease::new(ReadResult::End(0), 6), 1);
    let checks = Cell::new(0);
    assert_eq!(
        after.perform(
            &conduit_core::encode_count(0),
            &mut out,
            Duration::from_secs(1),
            || {
                let n = checks.get();
                checks.set(n + 1);
                n != 0
            }
        ),
        Err(WorkerFailure::Cancelled)
    );
    assert_eq!(after.original.acks.load(Ordering::SeqCst), 1);
    assert!(after.is_fenced());
}

#[test]
fn timeout_is_selected_by_original_and_worker_budgets_not_diagnostic_constant() {
    let mut lease = Lease::new(ReadResult::End(0), 6);
    lease.timeout = Duration::from_secs(3);
    let read = RecordReadBudget {
        maximum_requests: 1,
        timeout_per_request: Duration::from_secs(2),
    };
    let mut worker = worker_budget();
    worker.capture_timeout = Duration::from_secs(2);
    worker.cancellation_timeout = Duration::from_secs(2);
    assert!(ForeRecordReadWorker::prepare(lease, read, worker).is_ok());
    let mut worker = worker_budget();
    worker.capture_timeout = Duration::from_secs(1);
    worker.cancellation_timeout = Duration::from_millis(1);
    assert!(matches!(
        ForeRecordReadWorker::prepare(Lease::new(ReadResult::End(0), 6), budget(1), worker),
        Err((_, PreparationRefusal::Timeout))
    ));
}

#[test]
fn changed_original_budget_and_invalid_input_do_not_start_io() {
    let mut worker = prepared(Lease::new(ReadResult::End(0), 12), 2);
    worker.original.frames = 5;
    let mut out = [0; READ_RESULT_BYTES];
    assert_eq!(
        worker.perform(
            &conduit_core::encode_count(0),
            &mut out,
            Duration::from_secs(1),
            || false
        ),
        Err(WorkerFailure::Profile)
    );
    assert_eq!(worker.original.calls.load(Ordering::SeqCst), 0);
    for input in [vec![0; 7], conduit_core::encode_count(u64::MAX).to_vec()] {
        let mut worker = prepared(Lease::new(ReadResult::End(0), 6), 1);
        assert_eq!(
            worker.perform(&input, &mut out, Duration::from_secs(1), || false),
            Err(WorkerFailure::InvalidOutput)
        );
        assert_eq!(worker.original.calls.load(Ordering::SeqCst), 0);
        assert!(worker.is_fenced());
    }
}
