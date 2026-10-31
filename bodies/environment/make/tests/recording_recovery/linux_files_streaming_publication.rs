//! File/kernel backing exists before the first source fact. Placement, grants,
//! producer authentication and producer clock are explicit CPU fixtures.
use super::linux_files::directory;
use super::linux_files_publication::plan;
use super::*;
use conduit_environment_linux::{FilePublicationRefusal, PreparedFileRecordingPublication};
use conduit_environment_runtime::publication::{PublicationChannel, PublicationSnapshot};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};

#[test]
fn linux_files_streaming_publication_prepares_before_samples_and_commits_original_requests() {
    execute(false);
    execute(true);
}
fn execute(cancel: bool) {
    let contract = ObservationContract::prepare().unwrap();
    let mut selected = reference();
    selected.extent.bytes =
        PreparedRecordingSnapshot::content_bytes(context(), &contract, 2).unwrap() as u64;
    let mut generation = PreparedRecordingBatch::prepare(
        context(),
        &contract,
        2,
        core::slice::from_ref(&selected),
        1,
        4096,
    )
    .unwrap()
    .into_generations()
    .pop();
    let (plan, lowered) = plan::plan(&selected);
    let fragment = &plan.fragments[0];
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let foreign = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 2);
    let root = directory::Root::prepare();
    let directory = root.directory();
    let mut owner = PreparedFileRecordingPublication::prepare_streaming(
        &directory,
        fragment,
        &lowered,
        &active,
        &mut generation,
    )
    .unwrap();
    assert!(generation.is_none());
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
    let mut input = PreparedRecordingInput::prepare(&contract).unwrap();
    let mut decoder =
        conduit_environment_runtime::observation::PreparedSourceFrameDecoder::prepare(&contract)
            .unwrap();
    let value = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
    let first = PublicationSnapshot {
        channel: PublicationChannel::Reading,
        sequence: 1,
        canonical: value,
    };
    let second = PublicationSnapshot {
        sequence: 2,
        ..first
    };
    let mut expected =
        PreparedRecordingSnapshot::prepare(context(), &contract, 2, &selected).unwrap();
    expected
        .append_publication(&mut input, first, 60_000)
        .unwrap();
    expected
        .append_publication(&mut input, second, 120_000)
        .unwrap();
    let expected = expected.seal().unwrap().to_owned();
    let ((), memory) = allocation_probe::observe(|| {
        assert!(matches!(
            owner.append_publication(&foreign, &mut input, first, 60_000),
            Err(FilePublicationRefusal::Scope)
        ));
        assert!(matches!(
            owner.admit(&active),
            Err(FilePublicationRefusal::Content(
                RecordingSnapshotRefusal::Incomplete
            ))
        ));
        owner
            .append_publication(&active, &mut input, first, 60_000)
            .unwrap();
        assert_eq!(
            owner
                .append_publication(&active, &mut input, first, 60_000)
                .unwrap(),
            ChunkAppend::Duplicate
        );
        assert!(matches!(
            owner.admit(&active),
            Err(FilePublicationRefusal::Content(
                RecordingSnapshotRefusal::Incomplete
            ))
        ));
        assert!(owner.service_next(&active).unwrap().is_none());
        assert_eq!(owner.observer().committed(), None);
        if cancel {
            owner.cancel(&active).unwrap();
            assert!(matches!(
                owner.append_publication(&active, &mut input, second, 120_000),
                Err(FilePublicationRefusal::Shape)
            ));
            assert!(matches!(
                owner.admit(&active),
                Err(FilePublicationRefusal::Shape)
            ));
            return;
        }
        // Refusal cannot replace the first sample or consume the second slot.
        assert!(owner
            .append_publication(&active, &mut input, second, 59_999)
            .is_err());
        owner
            .append_publication(&active, &mut input, second, 120_000)
            .unwrap();
        assert!(matches!(
            owner.admit(&active).unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        ));
        assert!(matches!(
            owner.append_publication(&active, &mut input, second, 120_000),
            Err(FilePublicationRefusal::Shape)
        ));
        let mut effects = 0;
        let mut delivered = 0;
        let mut drained = false;
        for _ in 0..40 {
            if let Some(completion) = owner.service_next(&active).unwrap() {
                effects += 1;
                assert_eq!(
                    completion.disposition,
                    conduit_kernel::HostCallDisposition::Completed
                );
                assert!(completion.refusal.is_none());
                assert_eq!(
                    owner.observer().committed(),
                    None,
                    "file effect precedes Back commit"
                );
            }
            if owner.take_reference(&active).unwrap().is_some() {
                delivered += 1;
            }
            if owner.step(&active).unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!((effects, delivered), (1, 1));
        assert_eq!(
            owner.observer().committed(),
            Some(RecordedRange {
                first_sequence: 1,
                last_sequence: 2,
                samples: 2
            })
        );
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    if cancel {
        assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
        assert_eq!(owner.observer().committed(), None);
        return;
    }
    // Fresh file owner and independent current read authority see exact source
    // samples and producer times, rather than staging or recorder receipt times.
    let reopened = root.directory();
    let (mut read, _) = super::linux_files::publisher_fixture::placement(
        "fixture/recorder/read-boot",
        false,
        expected.len(),
    );
    read.host_id = fragment.host_id.clone();
    read.authority[0].host_id = read.host_id.clone();
    let resource = read.resources[0].content.as_mut().unwrap();
    resource.owner_host = read.host_id.clone();
    resource.contract.identity = selected.identity;
    resource.contract.version = selected.lifetime.version;
    let mut reader =
        conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(&reopened, &read, &selected)
            .unwrap();
    assert_eq!(
        reader.read(&read.authority[0]).unwrap(),
        expected.as_bytes()
    );
}

#[test]
fn linux_files_streaming_preflight_refuses_nonempty_generation_without_transfer_or_file_effect() {
    let contract = ObservationContract::prepare().unwrap();
    let mut selected = reference();
    selected.extent.bytes =
        PreparedRecordingSnapshot::content_bytes(context(), &contract, 2).unwrap() as u64;
    let mut generation = PreparedRecordingBatch::prepare(
        context(),
        &contract,
        2,
        core::slice::from_ref(&selected),
        1,
        4096,
    )
    .unwrap()
    .into_generations()
    .pop();
    generation
        .as_mut()
        .unwrap()
        .encoder
        .append(1, 60_000, RecordingDisposition::CaptureTimeout, None)
        .unwrap();
    let (plan, lowered) = plan::plan(&selected);
    let fragment = &plan.fragments[0];
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let root = directory::Root::prepare();
    let directory = root.directory();
    assert!(matches!(
        PreparedFileRecordingPublication::prepare_streaming(
            &directory,
            fragment,
            &lowered,
            &active,
            &mut generation
        ),
        Err(FilePublicationRefusal::Shape)
    ));
    assert_eq!(generation.as_ref().unwrap().encoder.len(), 1);
    assert_eq!(generation.as_ref().unwrap().observer.committed(), None);
    assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
}
