//! Explicit Back lifecycle fixtures: no real storage, installed Body or live Host.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod common;
use common::{basis, chunk, context};
use conduit_core as c;
use conduit_environment_history::{
    CommittedHistoryRefusal as R, HistoryFaceRefusal, PreparedCommittedHistoryFace, HISTORY_SUBJECT,
};
use conduit_environment_runtime::{
    observation::ObservationContract,
    recording::{
        PreparedRecordingSnapshot, RecordingCommitObserver, RecordingContext, RecordingSnapshotBack,
    },
};
use conduit_face::{FaceFragment, FacePropertyValue as V};
use conduit_kernel::{
    self as k,
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
};

struct Publication {
    back: RecordingSnapshotBack,
    observer: RecordingCommitObserver,
    content: String,
    reference: Vec<u8>,
}
fn generation(version: u8, events: &[(u64, u64, bool)]) -> Publication {
    publication_for(version, context(), chunk(events), events.len())
}
fn publication_for(
    version: u8,
    producer: RecordingContext<'_>,
    content: String,
    samples: usize,
) -> Publication {
    let contract = ObservationContract::prepare().unwrap();
    let reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([11; 32]),
        content_profile: c::kind_id("text/json-utf8@1"),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: PreparedRecordingSnapshot::content_bytes(
                RecordingContext {
                    host: producer.host,
                    boot: producer.boot,
                    plan: producer.plan,
                    play: producer.play,
                },
                &contract,
                samples,
            )
            .unwrap() as u64,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([version; 32]),
            expires_at: None,
        },
    };
    let (back, observer) =
        RecordingSnapshotBack::prepare(producer, &contract, samples, &reference).unwrap();
    Publication {
        back,
        observer,
        content,
        reference: reference.encode().unwrap(),
    }
}
impl Publication {
    fn request(&mut self) {
        let value = k::ValueRef {
            slot: 0,
            generation: 1,
            byte_len: self.content.len() as u32,
        };
        let mut io = StepIo::test_frame([Some(value)], [false], [Some(512)], None, 16);
        assert_eq!(
            self.back.step(
                &mut io,
                &StepInputBytes::test_frame([Some(self.content.as_bytes())], None)
            ),
            StepOutcome::Progress
        );
    }
    fn commit_step(&mut self) {
        <RecordingSnapshotBack as StepBack<1>>::step_committed(&mut self.back);
    }
    fn complete_stage(&mut self, disposition: k::HostCallDisposition) -> StepOutcome {
        let success = disposition == k::HostCallDisposition::Completed;
        let output = success.then(|| {
            k::BoundedValueRef::new(
                k::ValueRef {
                    slot: 1,
                    generation: 1,
                    byte_len: self.reference.len() as u32,
                },
                512,
            )
            .unwrap()
        });
        let outcome = k::HostCallOutcome {
            disposition,
            output,
            failure: (!success).then_some(k::Failure {
                code: k::FailureCode::InvalidInput,
                detail: 73,
            }),
        };
        let mut io = StepIo::test_frame(
            [None],
            [false],
            [Some(512)],
            Some((k::RequestId(0), outcome)),
            16,
        );
        self.back.step(
            &mut io,
            &StepInputBytes::test_frame([None], success.then_some(self.reference.as_slice())),
        )
    }
    fn publish(&mut self) {
        self.request();
        self.commit_step();
        assert_eq!(
            self.complete_stage(k::HostCallDisposition::Completed),
            StepOutcome::Progress
        );
        self.commit_step();
        assert!(self
            .observer
            .committed_content_matches(self.content.as_bytes()));
    }
}
fn property<'a>(f: &'a FaceFragment, subject: &str, name: &str) -> Option<&'a V> {
    f.properties
        .iter()
        .find(|p| p.subject == subject && p.name == name)
        .map(|p| &p.value)
}

#[test]
fn request_and_host_completion_cannot_advance_history_before_sink_commit() {
    let mut publication = generation(1, &[(1, 100, true)]);
    let observers = [publication.observer.clone()];
    let contract = ObservationContract::prepare().unwrap();
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    assert_eq!(
        history.next_reference(),
        Some(publication.reference.as_slice())
    );
    let initial = history.contribution(basis()).unwrap();
    assert!(property(&initial, HISTORY_SUBJECT, "last-publication-reference").is_none());
    assert_eq!(history.read_next(&publication.content), Err(R::Uncommitted));
    publication.request();
    assert!(!publication
        .observer
        .committed_content_matches(publication.content.as_bytes()));
    assert_eq!(history.read_next(&publication.content), Err(R::Uncommitted));
    publication.commit_step();
    assert_eq!(history.read_next(&publication.content), Err(R::Uncommitted));
    assert_eq!(
        publication.complete_stage(k::HostCallDisposition::Completed),
        StepOutcome::Progress
    );
    assert_eq!(history.read_next(&publication.content), Err(R::Uncommitted));
    assert_eq!(history.recovered().samples, 0);
    assert_eq!(history.contribution(basis()).unwrap(), initial);
    publication.commit_step();
    history.read_next(&publication.content).unwrap();
    assert_eq!(history.recovered().samples, 1);
    assert_eq!(history.next_reference(), None);
}

#[test]
fn committed_same_range_and_cardinality_cannot_substitute_other_content() {
    let mut publication = generation(2, &[(1, 100, true), (2, 200, true)]);
    publication.publish();
    let observers = [publication.observer.clone()];
    let contract = ObservationContract::prepare().unwrap();
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    let changed = chunk(&[(1, 101, true), (2, 200, true)]);
    assert_eq!(changed.len(), publication.content.len());
    assert!(!publication
        .observer
        .committed_content_matches(changed.as_bytes()));
    let initial = history.contribution(basis()).unwrap();
    for invalid in [changed.as_str(), "{}"] {
        assert_eq!(history.read_next(invalid), Err(R::ContentMismatch));
        assert_eq!(history.contribution(basis()).unwrap(), initial);
        assert_eq!(
            history.next_reference(),
            Some(publication.reference.as_slice())
        );
    }
    history.read_next(&publication.content).unwrap();
    assert_eq!(history.read_next(&publication.content), Err(R::Capacity));
    assert_eq!(history.recovered().samples, 2);
}

#[test]
fn failed_denied_cancelled_and_cancelled_staged_success_remain_uncommitted() {
    for disposition in [
        k::HostCallDisposition::Failed,
        k::HostCallDisposition::Denied,
        k::HostCallDisposition::Cancelled,
    ] {
        let mut publication = generation(3, &[(1, 100, false)]);
        publication.request();
        publication.commit_step();
        assert!(matches!(
            publication.complete_stage(disposition),
            StepOutcome::Fail(_)
        ));
        publication.commit_step();
        assert_eq!(publication.observer.committed(), None);
        let observers = [publication.observer.clone()];
        let contract = ObservationContract::prepare().unwrap();
        let mut history =
            PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
        assert_eq!(history.read_next(&publication.content), Err(R::Uncommitted));
        assert_eq!(history.recovered().samples, 0);
    }
    let mut publication = generation(4, &[(1, 100, true)]);
    publication.request();
    publication.commit_step();
    assert_eq!(
        publication.complete_stage(k::HostCallDisposition::Completed),
        StepOutcome::Progress
    );
    <RecordingSnapshotBack as StepBack<1>>::cancel(&mut publication.back);
    publication.commit_step();
    assert_eq!(publication.observer.committed(), None);
    assert!(!publication
        .observer
        .committed_content_matches(publication.content.as_bytes()));
}

#[test]
fn references_are_preselected_and_a_different_valid_commit_cannot_advance_them() {
    let mut selected = generation(5, &[(1, 100, true)]);
    let mut other = generation(6, &[(1, 110, true)]);
    other.publish();
    let observers = [selected.observer.clone()];
    let contract = ObservationContract::prepare().unwrap();
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    assert_eq!(history.read_next(&other.content), Err(R::Uncommitted));
    selected.publish();
    assert_eq!(history.read_next(&other.content), Err(R::ContentMismatch));
    assert_eq!(
        history.next_reference(),
        Some(selected.reference.as_slice())
    );
    history.read_next(&selected.content).unwrap();
    assert_ne!(selected.reference, other.reference);
}

#[test]
fn empty_duplicate_and_replayed_generation_are_refused() {
    let contract = ObservationContract::prepare().unwrap();
    assert!(matches!(
        PreparedCommittedHistoryFace::prepare(context(), &contract, &[]),
        Err(R::Empty)
    ));
    let bounded = generation(7, &[(1, 100, true)]);
    let oversized = vec![bounded.observer.clone(); 65];
    assert!(matches!(
        PreparedCommittedHistoryFace::prepare(context(), &contract, &oversized),
        Err(R::Capacity)
    ));
    let different_extent = generation(7, &[(1, 100, true), (2, 200, false)]);
    assert_ne!(bounded.reference, different_extent.reference);
    let same_generation = [bounded.observer.clone(), different_extent.observer.clone()];
    assert!(matches!(
        PreparedCommittedHistoryFace::prepare(context(), &contract, &same_generation),
        Err(R::DuplicateGeneration)
    ));
    let mut publication = generation(7, &[(1, 100, true)]);
    publication.publish();
    let duplicates = [publication.observer.clone(), publication.observer.clone()];
    assert!(matches!(
        PreparedCommittedHistoryFace::prepare(context(), &contract, &duplicates),
        Err(R::DuplicateGeneration)
    ));
    // Separate Back instances cannot alias one immutable generation either.
    let alias = generation(7, &[(1, 100, true)]);
    let duplicates = [publication.observer.clone(), alias.observer.clone()];
    assert!(matches!(
        PreparedCommittedHistoryFace::prepare(context(), &contract, &duplicates),
        Err(R::DuplicateGeneration)
    ));
    let mut replay = generation(8, &[(1, 100, true)]);
    replay.publish();
    let observers = [publication.observer.clone(), replay.observer.clone()];
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    history.read_next(&publication.content).unwrap();
    let accepted = history.contribution(basis()).unwrap();
    assert!(matches!(
        history.read_next(&replay.content),
        Err(R::History(HistoryFaceRefusal::Recording(_)))
    ));
    assert_eq!(history.contribution(basis()).unwrap(), accepted);
    assert_eq!(history.next_reference(), Some(replay.reference.as_slice()));
}

#[test]
fn foreign_producer_is_refused_despite_a_real_fixture_publication_commit() {
    let foreign_boot: c::BootId = "boot/else".into();
    let original = context();
    let foreign = RecordingContext {
        host: original.host,
        boot: &foreign_boot,
        plan: original.plan,
        play: original.play,
    };
    let content = chunk(&[(1, 100, true)]).replace("626f6f742f74657374", "626f6f742f656c7365");
    let mut publication = publication_for(9, foreign, content, 1);
    publication.publish();
    let observers = [publication.observer.clone()];
    let contract = ObservationContract::prepare().unwrap();
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    assert!(matches!(
        history.read_next(&publication.content),
        Err(R::History(HistoryFaceRefusal::Recording(_)))
    ));
    assert_eq!(history.recovered().samples, 0);
}

#[test]
fn two_committed_chunks_preserve_gap_and_exact_immutable_generation_metadata() {
    let mut first = generation(10, &[(1, 100, true), (2, 200, false)]);
    let mut second = generation(11, &[(5, 900, true)]);
    first.publish();
    second.publish();
    let observers = [first.observer.clone(), second.observer.clone()];
    let contract = ObservationContract::prepare().unwrap();
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    let (accepted, allocation) = allocation_probe::observe(|| history.read_next(&first.content));
    accepted.unwrap();
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    let first_view = history.contribution(basis()).unwrap();
    assert_eq!(
        property(&first_view, HISTORY_SUBJECT, "publication-evidence"),
        Some(&V::Text("kernel-publication-commit".into()))
    );
    assert_eq!(
        property(&first_view, HISTORY_SUBJECT, "last-publication-reference"),
        Some(&V::Content(first.reference.clone()))
    );
    assert_eq!(history.next_reference(), Some(second.reference.as_slice()));
    history.read_next(&second.content).unwrap();
    let second_view = history.contribution(basis()).unwrap();
    assert_eq!(history.recovered().samples, 3);
    assert_eq!(history.recovered().chunks, 2);
    assert_eq!(
        property(&second_view, HISTORY_SUBJECT, "last-publication-reference"),
        Some(&V::Content(second.reference.clone()))
    );
    assert_eq!(
        property(
            &second_view,
            "environment-history/sample/5",
            "missing-sequence-first"
        ),
        Some(&V::Count(3))
    );
    assert_eq!(
        property(
            &second_view,
            "environment-history/sample/5",
            "missing-sequence-last"
        ),
        Some(&V::Count(4))
    );
    assert_eq!(history.contribution(basis()).unwrap(), second_view);
    assert_eq!(
        property(&first_view, HISTORY_SUBJECT, "last-publication-reference"),
        Some(&V::Content(first.reference.clone()))
    );
    assert_eq!(history.next_reference(), None);
}

#[test]
fn presentation_window_cannot_replace_exact_full_chunk_publication() {
    let events = (1..=64).map(|n| (n, n * 1000, true)).collect::<Vec<_>>();
    let mut publication = generation(1, &events);
    let observers = [publication.observer.clone()];
    let contract = ObservationContract::prepare().unwrap();
    let mut history =
        PreparedCommittedHistoryFace::prepare(context(), &contract, &observers).unwrap();
    let window = conduit_environment_history::HistoryFaceWindow::prepare(8).unwrap();
    let before = history.contribution_window(basis(), window).unwrap();
    assert_eq!(history.read_next(&publication.content), Err(R::Uncommitted));
    publication.publish();
    assert_eq!(
        history.read_next(&chunk(&events[56..])),
        Err(R::ContentMismatch)
    );
    assert_eq!(
        history.contribution_window(basis(), window).unwrap(),
        before
    );
    history.read_next(&publication.content).unwrap();
    let tail = history.contribution_window(basis(), window).unwrap();
    assert_eq!(history.recovered().samples, 64);
    assert_eq!(
        property(&tail, HISTORY_SUBJECT, "visible-samples"),
        Some(&V::Count(8))
    );
    assert_eq!(
        property(&tail, HISTORY_SUBJECT, "samples-outside-view"),
        Some(&V::Count(56))
    );
    assert_eq!(
        property(&tail, HISTORY_SUBJECT, "publication-evidence"),
        Some(&V::Text("kernel-publication-commit".into()))
    );
    assert_eq!(history.contribution(basis()).unwrap().subjects.len(), 65);
}
