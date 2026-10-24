//! Actual target worker and file publication kernels. The cross-Host Line,
//! producer clock and store placement/grants remain explicit CPU fixtures.
use super::*;
#[cfg(feature = "installed-recording-proof")]
#[path = "native_catalog_checkpoint.rs"]
mod checkpoint;
use conduit_environment_linux::PreparedFileRecordingPublication;
use conduit_environment_runtime::{publication::PublicationSink, recording::*};
use conduit_resource_snapshot::{PUBLISH_OPERATION, READ_OPERATION};
#[path = "../../../../../targets/std/resource-snapshot/tests/support/directory.rs"]
mod directory;
#[path = "../../../../../mechanisms/io/resource-snapshot/tests/support/fixture.rs"]
#[allow(dead_code)]
mod file_fixture;
#[path = "../common/recording_fixture.rs"]
#[allow(dead_code)]
mod fixture;
struct Sink<'a> {
    input: PreparedRecordingInput,
    owners: Vec<PreparedFileRecordingPublication<'a>>,
    active: Vec<c::ActivePlayIdentity>,
    ready: &'a Cell<bool>,
    stop: &'a Cell<bool>,
    accepted: usize,
    committed: usize,
    limit: usize,
    per_generation: usize,
    period_ms: u64,
    cancel_after: bool,
    failure: Option<k::Failure>,
    refusal: Option<conduit_linux_snapshot_store::FileOperationRefusal>,
    held: Option<(u64, usize, usize)>,
    wake: &'a core::cell::RefCell<Option<Waker>>,
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
        if let Some(held) = self.held {
            assert_eq!(held, identity);
        }
        if !self.ready.get() {
            self.held = Some(identity);
            *self.wake.borrow_mut() = Some(cx.waker().clone());
            return Poll::Pending;
        }
        assert_eq!(snapshot.sequence, self.accepted as u64 + 1);
        let generation = self.accepted / self.per_generation;
        let first = generation * self.per_generation + 1;
        let last = ((generation + 1) * self.per_generation).min(self.limit);
        let owner = &mut self.owners[generation];
        let active = &self.active[generation];
        // Explicit producer-clock fixture; never the poll or file receipt time.
        owner
            .append_publication(
                active,
                &mut self.input,
                snapshot,
                snapshot.sequence * self.period_ms,
            )
            .unwrap();
        if snapshot.sequence != last as u64 {
            assert_eq!(owner.observer().committed(), None);
            self.accepted += 1;
            self.held = None;
            return Poll::Ready(Ok(())); // staging, not a durable ACK
        }
        owner.admit(active).unwrap();
        let mut drained = false;
        let mut delivered = false;
        let mut effects = 0;
        // Bounded cooperative driving of an already admitted ordinary kernel.
        for _ in 0..40 {
            if let Some(effect) = owner.service_next(active).unwrap() {
                if let Some(failure) = effect.failure {
                    assert_eq!(effect.disposition, k::HostCallDisposition::Failed);
                    assert!(effect.refusal.is_some());
                    assert_eq!(owner.observer().committed(), None);
                    assert!(
                        matches!(owner.step(active), Err(conduit_environment_linux::FilePublicationRefusal::Kernel(k::scheduler::SchedulerError::BackFailed(actual))) if actual == failure)
                    );
                    self.failure = Some(failure);
                    self.refusal = effect.refusal;
                    return Poll::Ready(Err(failure));
                }
                assert_eq!(effect.disposition, k::HostCallDisposition::Completed);
                assert!(effect.refusal.is_none());
                assert_eq!(owner.observer().committed(), None);
                effects += 1;
            }
            let mut expected_reference = [0u8; 512];
            let length = owner.observer().encoded_reference().len();
            expected_reference[..length].copy_from_slice(owner.observer().encoded_reference());
            if let Some(reference) = owner.take_reference(active).unwrap() {
                assert!(!delivered);
                assert_eq!(reference, &expected_reference[..length]);
                delivered = true;
            }
            if owner.step(active).unwrap() == k::scheduler::SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained && delivered);
        assert_eq!(effects, 1);
        assert_eq!(
            owner.observer().committed(),
            Some(RecordedRange {
                first_sequence: first as u64,
                last_sequence: last as u64,
                samples: last - first + 1
            })
        );
        self.accepted += 1;
        self.committed = self.accepted;
        self.held = None;
        if self.cancel_after && self.accepted == self.limit {
            self.stop.set(true);
        }
        Poll::Ready(Ok(()))
    }
}
pub(super) fn check<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    mut execution: Box<Execution<P, W>>,
    pulse: &Cell<usize>,
    timer: &Cell<usize>,
    samples: u64,
    overnight: bool,
    refuse: bool,
    retirement: impl FnOnce(crate::native_worker::RetiredProviders<P, W>),
) {
    let scope = execution.native.pulse.inspection().scope.clone();
    let context = RecordingContext {
        host: &scope.host_id,
        boot: &scope.boot_id,
        plan: &scope.plan_id,
        play: &scope.active_play_id,
    };
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let root = directory::Root::prepare();
    let directory = root.directory();
    let ready = Cell::new(false);
    let stop = Cell::new(false);
    let wake = core::cell::RefCell::new(None);
    let limit = if overnight { samples as usize } else { 2 };
    let per_generation = if overnight { MAXIMUM_CHUNK_SAMPLES } else { 1 };
    let generations = limit.div_ceil(per_generation);
    assert!(generations <= MAXIMUM_CATALOG_GENERATIONS);
    let period_ms = if overnight { 60_000 } else { 100 };
    let mut sink = Sink {
        input: PreparedRecordingInput::prepare(&contract).unwrap(),
        owners: Vec::with_capacity(generations),
        active: Vec::with_capacity(generations),
        ready: &ready,
        stop: &stop,
        accepted: 0,
        committed: 0,
        limit,
        per_generation,
        period_ms,
        cancel_after: !overnight,
        failure: None,
        refusal: None,
        held: None,
        wake: &wake,
    };
    let mut references = Vec::with_capacity(generations);
    for index in 0..generations {
        let chunk_samples = per_generation.min(limit - index * per_generation);
        let mut reference = fixture::reference();
        reference.extent.bytes =
            PreparedRecordingSnapshot::content_bytes(context, &contract, chunk_samples).unwrap()
                as u64;
        reference.lifetime.version =
            c::ResourceVersionIdentity::from_digest([(index + 1) as u8; 32]);
        let mut generation = PreparedRecordingBatch::prepare(
            context,
            &contract,
            chunk_samples as u64,
            core::slice::from_ref(&reference),
            1,
            4096,
        )
        .unwrap()
        .into_generations()
        .pop();
        let (plan, lowered) = fixture::plan(&reference);
        let fragment = &plan.fragments[0];
        let active =
            c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
        if refuse && index == 1 {
            // A real pre-existing immutable generation, not an injected failure.
            let mut occupied =
                PreparedRecordingSnapshot::prepare(context, &contract, 1, &reference).unwrap();
            occupied
                .append(2, 200, RecordingDisposition::CaptureTimeout, None)
                .unwrap();
            conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(
                &directory,
                &fragment.placements[0],
                &reference,
            )
            .unwrap()
            .publish(
                &fragment.placements[0].authority[0],
                occupied.seal().unwrap().as_bytes(),
            )
            .unwrap();
        }
        sink.owners.push(
            PreparedFileRecordingPublication::prepare_streaming(
                &directory,
                fragment,
                &lowered,
                &active,
                &mut generation,
            )
            .unwrap(),
        );
        assert!(generation.is_none());
        sink.active.push(active);
        references.push(reference);
    }
    let (outcome, memory) = allocation_probe::observe(|| {
        let stopping = core::future::poll_fn(|_| {
            if stop.get() {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        });
        let mut future = core::pin::pin!(execution.run_with_sink(stopping, &mut sink));
        for _ in 0..500 {
            assert!(future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
        }
        assert!(wake.borrow().is_some());
        let held = (pulse.get(), timer.get());
        for _ in 0..20 {
            assert!(future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending());
            assert_eq!((pulse.get(), timer.get()), held);
        }
        ready.set(true);
        wake.borrow_mut().take().unwrap().wake();
        for _ in 0..limit * 2000 {
            if let Poll::Ready(result) = future
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
            {
                return result;
            }
        }
        panic!("bounded native/file worker did not retire");
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    if refuse {
        let failure = sink.failure.expect("original storage failure");
        assert_eq!(outcome.exit, Exit::Failed(failure));
        assert_eq!(outcome.cleanup, Ok(()));
        assert_eq!(
            (outcome.progress.publications, sink.accepted, sink.committed),
            (1, 1, 1)
        );
        assert!(sink.owners[0].observer().committed().is_some());
        assert_eq!(sink.owners[1].observer().committed(), None);
        let observers: Vec<_> = sink.owners.iter().map(|o| o.observer().clone()).collect();
        assert!(matches!(
            PreparedRecordingCatalog::prepare_committed(&references, &observers, 3, 12288),
            Err(RecordingCatalogRefusal::UncommittedGeneration(1))
        ));
        assert!(matches!(
            sink.refusal,
            Some(conduit_linux_snapshot_store::FileOperationRefusal::Storage(
                conduit_linux_snapshot_store::FileSnapshotRefusal::PublishedNameExists
            ))
        ));
        // Two immutable publications plus the refused write's retained candidate.
        assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 3);
        let reopened = root.directory();
        let mut history = PreparedRecordingReader::prepare(context, &contract).unwrap();
        for (index, reference) in references.iter().enumerate() {
            let (mut read, _) = file_fixture::placement(
                "fixture/refusal-read-boot",
                false,
                reference.extent.bytes as usize,
            );
            read.host_id = "fixture/recorder".into();
            read.authority[0].host_id = read.host_id.clone();
            let resource = read.resources[0].content.as_mut().unwrap();
            resource.owner_host = read.host_id.clone();
            resource.contract.identity = reference.identity;
            resource.contract.version = reference.lifetime.version;
            let mut reader = conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(
                &reopened, &read, reference,
            )
            .unwrap();
            history
                .read(core::str::from_utf8(reader.read(&read.authority[0]).unwrap()).unwrap())
                .unwrap();
            let sample = history.sample(0).unwrap();
            assert_eq!(
                (sample.sequence, sample.sampled_at_boot_ms),
                (index as u64 + 1, (index as u64 + 1) * 100)
            );
            assert_eq!(
                sample.disposition,
                if index == 0 {
                    RecordingDisposition::Available
                } else {
                    RecordingDisposition::CaptureTimeout
                }
            );
        }
        // No catalogue can make the refused generation part of this history.
        eprintln!("native_file_refusal_cpu accepted=1 committed=1 original_failure={failure:?} catalogue_refused=true cleanup=true");
        retirement(super::retire(execution));
        return;
    }
    assert_eq!(
        outcome.exit,
        if overnight {
            Exit::Drained
        } else {
            Exit::Cancelled
        }
    );
    assert_eq!(outcome.cleanup, Ok(()));
    assert_eq!(outcome.progress.publications, limit as u64);
    assert_eq!(sink.accepted, limit);
    assert_eq!(sink.committed, limit);
    let observers: Vec<_> = sink.owners.iter().map(|o| o.observer().clone()).collect();
    let catalog = PreparedRecordingCatalog::prepare_committed(
        &references,
        &observers,
        generations + 1,
        ((generations + 1) * MAXIMUM_CHUNK_BYTES) as u64,
    )
    .unwrap();
    assert!(!catalog.content().is_empty());
    // Files persist independently of the producer worker and its staging owner.
    drop(sink);
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), generations);
    let reopened = root.directory();
    let mut history = PreparedRecordingReader::prepare(context, &contract).unwrap();
    let mut quantities =
        conduit_environment_runtime::observation::PreparedObservationQuantityForms::prepare(
            &contract,
        )
        .unwrap();
    for (index, reference) in references.iter().enumerate() {
        let (mut read, _) = file_fixture::placement(
            "fixture/recorder/reopened-boot",
            false,
            reference.extent.bytes as usize,
        );
        read.host_id = "fixture/recorder".into();
        read.authority[0].host_id = read.host_id.clone();
        let resource = read.resources[0].content.as_mut().unwrap();
        resource.owner_host = read.host_id.clone();
        resource.contract.identity = reference.identity;
        resource.contract.version = reference.lifetime.version;
        let mut reader = conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(
            &reopened, &read, reference,
        )
        .unwrap();
        let bytes = reader.read(&read.authority[0]).unwrap();
        history.read(core::str::from_utf8(bytes).unwrap()).unwrap();
        let expected = per_generation.min(limit - index * per_generation);
        assert_eq!(history.len(), expected);
        for offset in 0..expected {
            let sample = history.sample(offset).unwrap();
            let sequence = (index * per_generation + offset + 1) as u64;
            assert_eq!(
                (sample.sequence, sample.sampled_at_boot_ms),
                (sequence, sequence * period_ms)
            );
            assert_eq!(
                quantities.pack(sample.observation.unwrap()).unwrap(),
                [198, 0, 48, 2]
            );
        }
    }
    #[cfg(feature = "installed-recording-proof")]
    checkpoint::check(&reopened, &references, &observers);
    eprintln!("native_file_sink_cpu accepted={limit} committed={limit} generations={generations} native_play_allocations=0 kernel_commit_observed=true line_fixture=true");
    retirement(super::retire(execution));
}
