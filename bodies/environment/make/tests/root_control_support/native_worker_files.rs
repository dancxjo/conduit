//! Actual target worker and file publication kernels. The cross-Host Line,
//! producer clock and store placement/grants remain explicit CPU fixtures.
use super::*;
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
        let owner = &mut self.owners[self.accepted];
        let active = &self.active[self.accepted];
        // Explicit producer-clock fixture; never the poll or file receipt time.
        owner
            .append_publication(active, &mut self.input, snapshot, snapshot.sequence * 100)
            .unwrap();
        owner.admit(active).unwrap();
        let mut drained = false;
        let mut delivered = false;
        let mut effects = 0;
        // Bounded cooperative driving of an already admitted ordinary kernel.
        for _ in 0..40 {
            if let Some(effect) = owner.service_next(active).unwrap() {
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
                first_sequence: snapshot.sequence,
                last_sequence: snapshot.sequence,
                samples: 1
            })
        );
        self.accepted += 1;
        self.held = None;
        if self.accepted == self.owners.len() {
            self.stop.set(true);
        }
        Poll::Ready(Ok(()))
    }
}
pub(super) fn check<P: PulseReadProvider, W: w::MonotonicWaitProvider>(
    mut execution: Box<Execution<P, W>>,
    pulse: &Cell<usize>,
    timer: &Cell<usize>,
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
    let mut sink = Sink {
        input: PreparedRecordingInput::prepare(&contract).unwrap(),
        owners: Vec::with_capacity(2),
        active: Vec::with_capacity(2),
        ready: &ready,
        stop: &stop,
        accepted: 0,
        held: None,
        wake: &wake,
    };
    let mut references = Vec::with_capacity(2);
    for index in 0..2 {
        let mut reference = fixture::reference();
        reference.extent.bytes =
            PreparedRecordingSnapshot::content_bytes(context, &contract, 1).unwrap() as u64;
        reference.lifetime.version = c::ResourceVersionIdentity::from_digest([index + 1; 32]);
        let mut generation = PreparedRecordingBatch::prepare(
            context,
            &contract,
            1,
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
        for _ in 0..10000 {
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
    assert_eq!(outcome.exit, Exit::Cancelled);
    assert_eq!(outcome.cleanup, Ok(()));
    assert_eq!(outcome.progress.publications, 2);
    assert_eq!(sink.accepted, 2);
    let observers: Vec<_> = sink.owners.iter().map(|o| o.observer().clone()).collect();
    let catalog =
        PreparedRecordingCatalog::prepare_committed(&references, &observers, 3, 8192).unwrap();
    assert!(!catalog.content().is_empty());
    // Files persist independently of the producer worker and its staging owner.
    drop(sink);
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 2);
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
        assert_eq!(history.len(), 1);
        let sample = history.sample(0).unwrap();
        assert_eq!(
            (sample.sequence, sample.sampled_at_boot_ms),
            (index as u64 + 1, (index as u64 + 1) * 100)
        );
        // The retained CPU transcript is 19.8°C and 56% RH; both quantities
        // survive the canonical publication and checked recording Form.
        assert_eq!(
            quantities.pack(sample.observation.unwrap()).unwrap(),
            [198, 0, 48, 2]
        );
    }
    eprintln!("native_file_sink_cpu accepted=2 generations=2 play_allocations=0 kernel_commit_observed=true line_fixture=true");
    retirement(super::retire(execution));
}
