//! Checked Source, real Linux files and the production kernel. Producer
//! authentication, Host/resource admission and placement are CPU fixtures.
use super::linux_files::directory;
use super::linux_files_publication::plan;
use super::*;
use conduit_environment_linux::{FilePublicationRefusal, PreparedFileRecordingPublication};
use conduit_environment_runtime::observed_record::ObservedRecord;
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};

#[test]
fn linux_files_observations_keep_exact_original_bytes_through_commit() {
    execute(false, RecordingTimestampBasis::ProducerObservation);
    execute(true, RecordingTimestampBasis::ProducerObservation);
    execute(false, RecordingTimestampBasis::Acquisition);
}
fn execute(cancel: bool, timestamp_basis: RecordingTimestampBasis) {
    let producer = || RecordingContext {
        timestamp_basis,
        ..context()
    };
    let contract = ObservationContract::prepare().unwrap();
    let mut selected = reference();
    selected.extent.bytes =
        PreparedRecordingSnapshot::content_bytes(producer(), &contract, 2).unwrap() as u64;
    let mut generation = PreparedRecordingBatch::prepare(
        producer(),
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
    let first = ObservedRecord {
        sequence: 1,
        observed_at_boot_ms: 60_000,
        disposition: RecordingDisposition::Available,
        quantities: Some((
            c::Quantity::new(21, c::Unit::Celsius),
            c::Quantity::new(61, c::Unit::Percent),
        )),
    };
    let second = ObservedRecord {
        sequence: 2,
        observed_at_boot_ms: 120_000,
        disposition: RecordingDisposition::Checksum,
        quantities: None,
    };
    let mut expected =
        PreparedRecordingSnapshot::prepare(producer(), &contract, 2, &selected).unwrap();
    if timestamp_basis == RecordingTimestampBasis::ProducerObservation {
        expected.append_observed(&first).unwrap();
        expected.append_observed(&second).unwrap();
    }
    let expected = (timestamp_basis == RecordingTimestampBasis::ProducerObservation)
        .then(|| expected.seal().unwrap().to_owned());
    let ((), memory) = allocation_probe::observe(|| {
        assert!(matches!(
            owner.append_observed(&foreign, &first),
            Err(FilePublicationRefusal::Scope)
        ));
        if timestamp_basis == RecordingTimestampBasis::Acquisition {
            assert!(matches!(
                owner.append_observed(&active, &first),
                Err(FilePublicationRefusal::Content(
                    RecordingSnapshotRefusal::Recording(RecordingRefusal::ClockBasis)
                ))
            ));
            assert!(owner.admit(&active).is_err());
            return;
        }
        owner.append_observed(&active, &first).unwrap();
        assert_eq!(
            owner.append_observed(&active, &first).unwrap(),
            ChunkAppend::Duplicate
        );
        let changed = ObservedRecord {
            observed_at_boot_ms: 60_001,
            ..first
        };
        assert!(owner.append_observed(&active, &changed).is_err());
        assert!(owner.admit(&active).is_err());
        if cancel {
            owner.cancel(&active).unwrap();
            assert!(matches!(
                owner.append_observed(&active, &second),
                Err(FilePublicationRefusal::Shape)
            ));
            return;
        }
        owner.append_observed(&active, &second).unwrap();
        assert!(matches!(
            owner.admit(&active).unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 0 }
        ));
        assert!(matches!(
            owner.append_observed(&active, &second),
            Err(FilePublicationRefusal::Shape)
        ));
        let mut effects = 0;
        let mut drained = false;
        for _ in 0..40 {
            if let Some(completion) = owner.service_next(&active).unwrap() {
                effects += 1;
                assert_eq!(
                    completion.disposition,
                    conduit_kernel::HostCallDisposition::Completed
                );
                assert!(completion.refusal.is_none());
                assert!(owner.observer().committed().is_none());
            }
            let _ = owner.take_reference(&active).unwrap();
            if owner.step(&active).unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!(effects, 1);
        assert!(owner
            .observer()
            .committed_content_matches(expected.as_ref().unwrap().as_bytes()));
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    if cancel || timestamp_basis == RecordingTimestampBasis::Acquisition {
        assert_eq!(std::fs::read_dir(&root.path).unwrap().count(), 0);
        assert!(owner.observer().committed().is_none());
        return;
    }
    let expected = expected.unwrap();
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
    let restored = reader.read(&read.authority[0]).unwrap();
    assert_eq!(restored, expected.as_bytes());
    assert!(owner.observer().committed_content_matches(restored));
}
