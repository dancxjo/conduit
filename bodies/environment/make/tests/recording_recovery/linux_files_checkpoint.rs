//! Actual chunk kernel/file writes -> catalogue checkpoint -> Body State ->
//! fresh-Boot discovery and batch kernel/file recovery. Host/Body admission,
//! producer authentication and current grants remain explicit CPU fixtures.
use super::*;
use conduit_environment_linux::{
    FileBatchSoftware, PreparedFileBatchRecovery, PreparedFileRecordingPublication,
};
use conduit_environment_make::PreparedRecordingCatalogCheckpoint;
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_std_host as host;
#[path = "../support/catalog_host.rs"]
mod catalog_host;
#[path = "checkpoint_plan.rs"]
mod checkpoint_plan;
use super::linux_files::{directory, publisher_fixture};

#[test]
fn linux_files_checkpoint_restarts_the_night_from_its_actual_durable_seed() {
    let files = directory::Root::prepare();
    let state = tempfile::tempdir().unwrap();
    let body: conduit_body::BodyId = serde_json::from_str("\"fixture/checkpoint-night\"").unwrap();
    let directory = files.directory();
    let (references, observers) = write_chunks(&directory);
    let catalog =
        PreparedRecordingCatalog::prepare_committed(&references, &observers, 16, 16 * 4096)
            .unwrap();
    let mut selected = reference();
    selected.identity = c::ResourceSemanticIdentity::from_digest([99; 32]);
    selected.lifetime.version = c::ResourceVersionIdentity::from_digest([201; 32]);
    selected.extent.bytes = catalog.content().len() as u64;
    let checkpoint = PreparedRecordingCatalogCheckpoint::prepare_after_publications(
        selected.clone(),
        &references,
        &observers,
        16,
        16 * 4096,
    )
    .unwrap();
    assert_eq!(checkpoint.content(), catalog.content());
    run_checkpoint(&directory, &body, state.path(), &checkpoint, false);
    let seed_file = std::fs::read_dir(state.path())
        .unwrap()
        .map(|entry| entry.unwrap().path().join("current"))
        .find(|path| path.is_file())
        .unwrap();
    let seed_bytes = std::fs::read(&seed_file).unwrap();
    // An occupied next catalogue is a prior-effect fixture, not an ack for
    // this request. Its refusal must preserve the last committed Body seed.
    let mut later = selected.clone();
    later.lifetime.version = c::ResourceVersionIdentity::from_digest([202; 32]);
    let later = PreparedRecordingCatalogCheckpoint::prepare_after_publications(
        later,
        &references,
        &observers,
        16,
        16 * 4096,
    )
    .unwrap();
    run_checkpoint(&directory, &body, state.path(), &later, true);
    assert_eq!(std::fs::read(&seed_file).unwrap(), seed_bytes);
    drop(directory);
    let (mut reader, plan, _) = catalog_host::planned(false, "fixture/night-restart-boot");
    let recovered = conduit_environment_linux::recover_durable_catalog_seed(
        &mut reader,
        &body,
        state.path(),
        plan.fragments[0].clone(),
        &mut catalog_host::NoWait,
    )
    .unwrap()
    .unwrap();
    assert_eq!(recovered, selected);
    assert_eq!(std::fs::read(&seed_file).unwrap(), seed_bytes);
    let directory = files.directory();
    let decoded = read_catalog(&directory, &recovered);
    assert_eq!(decoded, references);
    recover_chunks(&directory, &decoded);
}
fn write_chunks(
    directory: &conduit_linux_snapshot_store::SnapshotDirectory,
) -> (Vec<c::BoundedResourceRef>, Vec<RecordingCommitObserver>) {
    let contract = ObservationContract::prepare().unwrap();
    let mut quantities =
        conduit_environment_runtime::observation::PreparedObservationQuantityForms::prepare(
            &contract,
        )
        .unwrap();
    let observation = quantities.unpack(&[198, 0, 78, 2]).unwrap();
    let mut references = Vec::new();
    let mut observers = Vec::new();
    for index in 0..15 {
        let mut selected = reference();
        selected.lifetime.version =
            c::ResourceVersionIdentity::from_digest([(index + 1) as u8; 32]);
        selected.extent.bytes =
            PreparedRecordingSnapshot::content_bytes(context(), &contract, 64).unwrap() as u64;
        let mut generation = Some(
            PreparedRecordingBatch::prepare(
                context(),
                &contract,
                64,
                core::slice::from_ref(&selected),
                1,
                4096,
            )
            .unwrap()
            .into_generations()
            .pop()
            .unwrap(),
        );
        for offset in 1..=64 {
            let sequence = (index * 64 + offset) as u64;
            let failed = sequence.is_multiple_of(17);
            generation
                .as_mut()
                .unwrap()
                .encoder
                .append(
                    sequence,
                    sequence * 60_000,
                    if failed {
                        RecordingDisposition::Checksum
                    } else {
                        RecordingDisposition::Available
                    },
                    (!failed).then_some(observation),
                )
                .unwrap();
        }
        let (plan, lowered) = super::linux_files_publication::plan::plan(&selected);
        let fragment = &plan.fragments[0];
        let active =
            c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
        let mut writer = PreparedFileRecordingPublication::prepare(
            directory,
            fragment,
            &lowered,
            &active,
            &mut generation,
        )
        .unwrap();
        let mut drained = false;
        let mut delivered = false;
        let (_, memory) = allocation_probe::observe(|| {
            assert!(matches!(
                writer.admit(&active).unwrap(),
                RemoteIngressOutcome::Accepted { sequence: 0 }
            ));
            for _ in 0..40 {
                if let Some(completion) = writer.service_next(&active).unwrap() {
                    assert!(completion.refusal.is_none());
                }
                if writer.take_reference(&active).unwrap().is_some() {
                    delivered = true;
                }
                if writer.step(&active).unwrap() == SchedulerStatus::Drained {
                    drained = true;
                    break;
                }
            }
        });
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert!(drained && delivered);
        observers.push(writer.observer().clone());
        references.push(selected);
        writer.cancel(&active).unwrap();
    }
    (references, observers)
}
fn run_checkpoint(
    directory: &conduit_linux_snapshot_store::SnapshotDirectory,
    body: &conduit_body::BodyId,
    state: &std::path::Path,
    checkpoint: &PreparedRecordingCatalogCheckpoint,
    occupied: bool,
) {
    let (mut host, plan, expected) = checkpoint_plan::planned_checkpoint(
        checkpoint.source(),
        checkpoint.reference(),
        if occupied {
            "fixture/catalog-occupied-boot"
        } else {
            "fixture/catalog-publish-boot"
        },
    );
    if occupied {
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND
            })
            .unwrap();
        conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(
            directory,
            placement,
            checkpoint.reference(),
        )
        .unwrap()
        .publish(&placement.authority[0], checkpoint.content().as_bytes())
        .unwrap();
    }
    let control = host::RunControl::default();
    let mut current = catalog_host::WriteCurrent {
        expected,
        control: control.clone(),
    };
    let result = host.run_body_durable_snapshot_plot_to(
        body,
        state,
        directory,
        plan.fragments[0].clone(),
        host::ExternalForeRun {
            inputs: &[host::ExternalForeInput {
                front_port_id: "catalog".into(),
                track: c::ConnectionTrack::Payload,
                bytes: checkpoint.content().as_bytes().to_vec(),
            }],
            output: &mut current,
            sequential: false,
            stream: None,
        },
        &mut std::io::sink(),
        &mut catalog_host::NoWait,
        &control,
    );
    if occupied {
        assert!(result.is_err());
    } else {
        let report = result.unwrap();
        assert_eq!(report.control_receipts.len(), 1);
    }
}
fn read_catalog(
    directory: &conduit_linux_snapshot_store::SnapshotDirectory,
    selected: &c::BoundedResourceRef,
) -> Vec<c::BoundedResourceRef> {
    let (mut placement, _) = publisher_fixture::placement(
        "fixture/catalog-reader-new-boot",
        false,
        selected.extent.bytes as usize,
    );
    placement.host_id = "fixture/seed-host".into();
    placement.authority[0].host_id = placement.host_id.clone();
    let resource = placement.resources[0].content.as_mut().unwrap();
    resource.owner_host = placement.host_id.clone();
    resource.contract.identity = selected.identity;
    resource.contract.version = selected.lifetime.version;
    let mut reader = conduit_environment_linux::PreparedFileCatalogRecovery::prepare(
        directory, &placement, selected,
    )
    .unwrap();
    let catalog = reader.read(&placement.authority[0]).unwrap();
    (0..catalog.len())
        .map(|index| {
            c::BoundedResourceRef::decode(catalog.encoded_reference(index).unwrap()).unwrap()
        })
        .collect()
}
fn recover_chunks(
    directory: &conduit_linux_snapshot_store::SnapshotDirectory,
    references: &[c::BoundedResourceRef],
) {
    let mut fixture = super::batch_plan::fixture(references);
    let plan = PreparedRecordingRecovery::prepare_batch(
        &fixture.source,
        &fixture.host,
        &fixture.choices,
        &bases(),
        &fixture.grants,
    )
    .unwrap();
    let fragment = &plan.plan().fragments[0];
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let mut recovery = PreparedFileBatchRecovery::<60, 59>::prepare(
        directory,
        fragment,
        plan.lowered(),
        &active,
        FileBatchSoftware {
            generations: &mut fixture.generations,
            singletons: &mut fixture.singleton,
            concats: &mut fixture.concat,
            recovery: &mut fixture.recovery,
        },
    )
    .unwrap();
    let mut drained = false;
    let (_, memory) = allocation_probe::observe(|| {
        for _ in 0..4000 {
            if let Some(completion) = recovery.service_next(&active).unwrap() {
                assert!(completion.refusal.is_none());
            }
            if recovery.step(&active).unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
    });
    assert!(drained);
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(
        recovery.history(),
        RecoveredHistory {
            chunks: 15,
            samples: 960,
            first_sequence: Some(1),
            last_sequence: Some(960)
        }
    );
    recovery.cancel(&active).unwrap();
}
