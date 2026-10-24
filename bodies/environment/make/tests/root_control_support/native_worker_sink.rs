//! Exact target worker with bounded in-memory recorder pressure; no durable ACK.
use super::*;
use conduit_environment_runtime::{
    observation::ObservationContract,
    publication::PublicationSink,
    recording::{
        PreparedRecordingChunk, PreparedRecordingInput, PreparedRecordingReader, RecordingContext,
        MAXIMUM_CHUNK_SAMPLES,
    },
};
use core::cell::RefCell;
struct Sink<'a> {
    input: PreparedRecordingInput,
    chunks: Vec<PreparedRecordingChunk>,
    ready: &'a Cell<bool>,
    fail: bool,
    pending: Cell<Option<(u64, usize, usize)>>,
    accepted: u64,
    wake: &'a RefCell<Option<Waker>>,
}
impl PublicationSink for Sink<'_> {
    fn poll_publication(
        &mut self,
        snapshot: PublicationSnapshot<'_>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), k::Failure>> {
        let identity = (
            snapshot.sequence,
            snapshot.canonical.as_ptr() as usize,
            snapshot.canonical.len(),
        );
        if let Some(pending) = self.pending.get() {
            assert_eq!(identity, pending);
        }
        if !self.ready.get() {
            self.pending.set(Some(identity));
            *self.wake.borrow_mut() = Some(cx.waker().clone());
            return Poll::Pending;
        }
        if self.fail {
            return Poll::Ready(Err(k::Failure {
                code: k::FailureCode::StorageExhausted,
                detail: 501,
            }));
        }
        assert_eq!(snapshot.sequence, self.accepted + 1);
        // Explicit CPU producer clock fixture, independent of polling/ACK time.
        let chunk = &mut self.chunks[self.accepted as usize / MAXIMUM_CHUNK_SAMPLES];
        self.input
            .append(chunk, snapshot, snapshot.sequence * 100)
            .unwrap();
        self.accepted = snapshot.sequence;
        self.pending.set(None);
        Poll::Ready(Ok(()))
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn check<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    mut execution: Box<Execution<P, W>>,
    pulse: &Cell<usize>,
    timer: &Cell<usize>,
    samples: u64,
    behavior: u8,
    retirement: impl FnOnce(crate::native_worker::RetiredProviders<P, W>),
) {
    let scope = execution.native.pulse.inspection().scope.clone();
    let context = RecordingContext {
        host: &scope.host_id,
        boot: &scope.boot_id,
        plan: &scope.plan_id,
        play: &scope.active_play_id,
    };
    let contract = ObservationContract::prepare().unwrap();
    let ready = Cell::new(false);
    let cancel = Cell::new(false);
    let wake = RefCell::new(None);
    let mut sink = Sink {
        input: PreparedRecordingInput::prepare(&contract).unwrap(),
        // All content buffers for this finite CPU workset exist before play.
        // They are staging content, not minted storage generations or receipts.
        chunks: (0..samples.div_ceil(MAXIMUM_CHUNK_SAMPLES as u64))
            .map(|_| PreparedRecordingChunk::prepare(context, &contract).unwrap())
            .collect(),
        ready: &ready,
        fail: behavior == 17,
        pending: Cell::new(None),
        accepted: 0,
        wake: &wake,
    };
    let stop = || {
        core::future::poll_fn(|_| {
            if cancel.get() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
    };
    let future_bytes = execution.sink_future_bytes(stop(), &mut sink);
    assert!(future_bytes > 0);
    assert_eq!((pulse.get(), timer.get()), (0, 0));
    let (outcome, memory) = allocation_probe::observe(|| {
        let mut future = core::pin::pin!(execution.run_with_sink(stop(), &mut sink));
        // No wall-clock polling: these are explicit cooperative CPU polls.
        for _ in 0..500 {
            assert!(future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
        }
        let held = (pulse.get(), timer.get());
        for _ in 0..20 {
            assert!(future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
            assert_eq!((pulse.get(), timer.get()), held);
        }
        assert!(wake.borrow().is_some());
        if behavior == 18 {
            return None;
        }
        if behavior == 16 {
            cancel.set(true);
        } else {
            ready.set(true);
        }
        // Readiness is owned separately from the sink's mutable future borrow.
        wake.borrow_mut().take().unwrap().wake();
        for _ in 0..samples * 2000 {
            if let Poll::Ready(outcome) = future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                return Some(outcome);
            }
        }
        panic!("bounded recorder-pressure worker did not finish");
    });
    let outcome = match outcome {
        Some(outcome) => outcome,
        None => {
            let (outcome, fenced_memory) = allocation_probe::observe(|| {
                run(execution.run_with_sink(core::future::ready(()), &mut sink))
            });
            assert_eq!(
                (fenced_memory.allocations, fenced_memory.reallocations),
                (0, 0)
            );
            outcome
        }
    };
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(outcome.cleanup, Ok(()));
    match behavior {
        16 => {
            assert_eq!(outcome.exit, Exit::Cancelled);
            assert_eq!(sink.accepted, 0);
            assert!(sink.chunks.iter().all(PreparedRecordingChunk::is_empty));
        }
        18 => {
            assert_eq!(
                outcome.exit,
                Exit::Failed(k::Failure {
                    code: k::FailureCode::InvalidLifecycle,
                    detail: 27
                })
            );
            assert_eq!(sink.accepted, 0);
            assert!(sink.chunks.iter().all(PreparedRecordingChunk::is_empty));
        }
        17 => {
            assert_eq!(
                outcome.exit,
                Exit::Failed(k::Failure {
                    code: k::FailureCode::StorageExhausted,
                    detail: 501
                })
            );
            assert_eq!(sink.accepted, 0);
            assert!(sink.chunks.iter().all(PreparedRecordingChunk::is_empty));
        }
        _ => {
            assert_eq!(outcome.exit, Exit::Drained);
            assert_eq!(sink.accepted, samples);
            assert_eq!(outcome.progress.publications, samples);
            let mut reader = PreparedRecordingReader::prepare(context, &contract).unwrap();
            let mut expected = 1;
            for chunk in &mut sink.chunks {
                reader.read(chunk.seal().unwrap()).unwrap();
                for n in 0..reader.len() {
                    let sample = reader.sample(n).unwrap();
                    assert_eq!(
                        (sample.sequence, sample.sampled_at_boot_ms),
                        (expected, expected * 100)
                    );
                    assert_eq!(sample.gap_before, None);
                    assert!(sample.observation.is_some());
                    expected += 1;
                }
            }
            assert_eq!(expected, samples + 1);
        }
    }
    for inspection in [
        execution.native.pulse.inspection(),
        execution.native.wait.inspection(),
    ] {
        assert_eq!(inspection.lifecycle, c::CapabilityLifecycle::Revoked);
        assert_eq!(inspection.in_flight_operations, 0);
    }
    eprintln!("native_publication_sink_cpu behavior={behavior} samples={samples} accepted={} future_bytes={future_bytes} play_allocations=0 durable_ack=false", sink.accepted);
    retirement(super::retire(execution));
}
