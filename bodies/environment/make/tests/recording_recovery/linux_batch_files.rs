//! Ordinary planned overnight batch and actual immutable Linux files. Offers,
//! grants and historical producer authentication are explicitly CPU fixtures.
use super::linux_files::{directory, publisher_fixture};
use super::*;
use conduit_environment_linux::{
    FileBatchSoftware, FileRecoveryRefusal, PreparedFileBatchRecovery,
};
use conduit_kernel::scheduler::SchedulerStatus;

#[test]
fn linux_files_batch_recovers_one_overnight_and_maximum_batches_without_allocations() {
    execute::<4, 3>(1, false);
    execute::<60, 59>(15, false);
    execute::<64, 63>(16, false);
}
#[test]
fn linux_files_batch_missing_generation_cannot_fabricate_the_complete_range() {
    execute::<60, 59>(15, true);
}
fn execute<const N: usize, const C: usize>(count: usize, missing: bool) {
    let contract = ObservationContract::prepare().unwrap();
    let mut contents = Vec::with_capacity(count);
    let mut references = Vec::with_capacity(count);
    let mut quantities =
        conduit_environment_runtime::observation::PreparedObservationQuantityForms::prepare(
            &contract,
        )
        .unwrap();
    let observation = quantities.unpack(&[198, 0, 78, 2]).unwrap();
    for index in 0..count {
        let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
        for offset in 1..=64 {
            let sequence = (index * 64 + offset) as u64;
            let failed = sequence.is_multiple_of(17);
            chunk
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
        contents.push(chunk.seal().unwrap().to_owned());
        let mut reference = reference();
        reference.extent.bytes = contents[index].len() as u64;
        reference.lifetime.version =
            c::ResourceVersionIdentity::from_digest([(index + 1) as u8; 32]);
        references.push(reference);
    }
    let root = directory::Root::prepare();
    let published = root.directory();
    for (index, (reference, content)) in references.iter().zip(&contents).enumerate() {
        if missing && index == count / 2 {
            continue;
        }
        let (mut write, _) =
            publisher_fixture::placement("fixture/recorder/old-boot", true, content.len());
        write.host_id = "fixture/recorder".into();
        write.authority[0].host_id = write.host_id.clone();
        let resource = write.resources[0].content.as_mut().unwrap();
        resource.owner_host = write.host_id.clone();
        resource.contract.identity = reference.identity;
        resource.contract.version = reference.lifetime.version;
        conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(&published, &write, reference)
            .unwrap()
            .publish(&write.authority[0], content.as_bytes())
            .unwrap();
    }
    drop(published);
    // Reopen actual files; no retained publication object supplies read bytes.
    let directory = root.directory();
    let references = if missing {
        references
    } else {
        recover_catalog(&root, &references)
    };
    let mut fixture = super::batch_plan::fixture(&references);
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
    let mut driver = PreparedFileBatchRecovery::<N, C>::prepare(
        &directory,
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
    let foreign = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 2);
    let signs = driver.kernel().signs().events().count();
    assert!(matches!(
        driver.step(&foreign),
        Err(FileRecoveryRefusal::Scope)
    ));
    assert!(matches!(
        driver.service_next(&foreign),
        Err(FileRecoveryRefusal::Scope)
    ));
    assert_eq!(driver.kernel().signs().events().count(), signs);
    let mut requests = 0;
    let mut refusals = 0;
    let mut drained = false;
    let mut failed = false;
    let (_, memory) = allocation_probe::observe(|| {
        for _ in 0..4000 {
            if let Some(completion) = driver.service_next(&active).unwrap() {
                requests += 1;
                if completion.refusal.is_some() {
                    refusals += 1;
                }
            }
            match driver.step(&active) {
                Ok(SchedulerStatus::Drained) => {
                    drained = true;
                    break;
                }
                Err(FileRecoveryRefusal::Kernel(_)) => {
                    failed = true;
                    break;
                }
                Ok(_) => {}
                Err(error) => panic!("unexpected recovery refusal: {error:?}"),
            }
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    if missing {
        assert!(failed);
        assert!(!drained);
        assert_eq!(refusals, 1);
        assert!(driver.history().samples <= (count / 2 * 64) as u32);
        assert!(driver.history().last_sequence.unwrap_or(0) <= (count / 2 * 64) as u64);
    } else {
        assert!(drained);
        assert!(!failed);
        assert_eq!(requests, count);
        assert_eq!(refusals, 0);
        assert_eq!(
            driver.history(),
            RecoveredHistory {
                chunks: count as u16,
                samples: (count * 64) as u32,
                first_sequence: Some(1),
                last_sequence: Some((count * 64) as u64),
            }
        );
    }
    driver.cancel(&active).unwrap();
}

// Actual catalogue publication and a fresh selected reader precede ordinary
// batch Source preparation. The Root's starting catalogue reference and all
// storage/authority facts remain explicit fixtures.
fn recover_catalog(
    root: &directory::Root,
    references: &[c::BoundedResourceRef],
) -> Vec<c::BoundedResourceRef> {
    let catalog =
        PreparedRecordingCatalog::prepare(references, references.len() + 1, 72 * 1024).unwrap();
    let mut selected = reference();
    selected.identity = c::ResourceSemanticIdentity::from_digest([42; 32]);
    selected.extent.bytes = catalog.content().len() as u64;
    let checkpoint = conduit_environment_make::PreparedRecordingCatalogCheckpoint::prepare(
        selected.clone(),
        catalog,
    )
    .unwrap();
    let placement = |write| {
        let (mut placement, _) = publisher_fixture::placement(
            if write {
                "fixture/recorder/catalog-old-boot"
            } else {
                "fixture/recorder/catalog-new-boot"
            },
            write,
            checkpoint.content().len(),
        );
        let resource = placement.resources[0].content.as_mut().unwrap();
        resource.contract.identity = selected.identity;
        resource.contract.version = selected.lifetime.version;
        placement
    };
    let write = placement(true);
    let directory = root.directory();
    conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(&directory, &write, &selected)
        .unwrap()
        .publish(&write.authority[0], checkpoint.content().as_bytes())
        .unwrap();
    drop(directory);
    let directory = root.directory();
    let read = placement(false);
    let mut recovery = conduit_environment_linux::PreparedFileCatalogRecovery::prepare(
        &directory, &read, &selected,
    )
    .unwrap();
    let (result, memory) = allocation_probe::observe(|| recovery.read(&read.authority[0]));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let recovered = result.unwrap();
    let decoded: Vec<_> = (0..recovered.len())
        .map(|index| {
            c::BoundedResourceRef::decode(recovered.encoded_reference(index).unwrap()).unwrap()
        })
        .collect();
    assert_eq!(decoded, references);
    decoded
}

#[test]
fn linux_files_catalog_refusals_do_not_return_references() {
    use conduit_environment_linux::{FileCatalogRefusal, PreparedFileCatalogRecovery};
    use conduit_linux_snapshot_store::{FileSnapshotRefusal, PreparedFileSnapshot};
    for publish in [false, true] {
        let content = br#"{"fixture":"not-a-recording-catalog"}"#;
        let root = directory::Root::prepare();
        let directory = root.directory();
        let (write, selected) =
            publisher_fixture::placement("fixture/catalog/old", true, content.len());
        if publish {
            PreparedFileSnapshot::prepare(&directory, &write, &selected)
                .unwrap()
                .publish(&write.authority[0], content)
                .unwrap();
        }
        let (read, _) = publisher_fixture::placement("fixture/catalog/new", false, content.len());
        let mut recovery =
            PreparedFileCatalogRecovery::prepare(&directory, &read, &selected).unwrap();
        let mut foreign = read.authority[0].clone();
        foreign.boot_id = "fixture/catalog/foreign".into();
        let (result, memory) = allocation_probe::observe(|| recovery.read(&foreign));
        assert!(matches!(
            result,
            Err(FileCatalogRefusal::Storage(FileSnapshotRefusal::Binding(_)))
        ));
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        let (result, memory) = allocation_probe::observe(|| recovery.read(&read.authority[0]));
        if publish {
            assert!(matches!(
                result,
                Err(FileCatalogRefusal::Catalog(
                    RecordingCatalogRefusal::Encoding
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(FileCatalogRefusal::Storage(FileSnapshotRefusal::Missing))
            ));
        }
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
}
