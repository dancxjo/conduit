//! Caller-selected resource identities are fixtures; no durable store is invoked.
use super::*;
use conduit_core as c;

fn references(samples: u64, contract: &ObservationContract) -> Vec<c::BoundedResourceRef> {
    (0..samples.div_ceil(64))
        .map(|index| {
            let size = (samples - index * 64).min(64) as usize;
            c::BoundedResourceRef {
                identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
                content_profile: c::kind_id("text/json-utf8@1"),
                access_class: "resource/snapshot@1".into(),
                extent: c::ResourceExtent {
                    bytes: PreparedRecordingSnapshot::content_bytes(
                        snapshot_context(),
                        contract,
                        size,
                    )
                    .unwrap() as u64,
                    items: Some(1),
                },
                lifetime: c::ResourceLifetime {
                    version: c::ResourceVersionIdentity::from_digest([(index + 1) as u8; 32]),
                    expires_at: None,
                },
            }
        })
        .collect()
}
#[test]
fn complete_overnight_batch_is_prepared_before_allocation_free_encoding() {
    let contract = ObservationContract::prepare().unwrap();
    let references = references(960, &contract);
    let batch = PreparedRecordingBatch::prepare(
        snapshot_context(),
        &contract,
        960,
        &references,
        64,
        1048576,
    )
    .unwrap();
    assert_eq!(batch.generations().len(), 15);
    assert_eq!(
        batch.content_bytes(),
        references.iter().map(|r| r.extent.bytes).sum::<u64>()
    );
    let mut generations = batch.into_generations();
    let mut reader = PreparedRecordingReader::prepare(snapshot_context(), &contract).unwrap();
    let (_, allocations) = allocation_probe::observe(|| {
        for (index, generation) in generations.iter_mut().enumerate() {
            assert_eq!(generation.samples, 64);
            for sample in 0..64 {
                let sequence = (index * 64 + sample + 1) as u64;
                generation
                    .encoder
                    .append(
                        sequence,
                        sequence * 60000,
                        RecordingDisposition::CaptureTimeout,
                        None,
                    )
                    .unwrap();
            }
            let content = generation.encoder.seal().unwrap();
            assert_eq!(content.len() as u64, references[index].extent.bytes);
            reader.read(content).unwrap();
            assert_eq!(reader.len(), 64);
            assert_eq!(
                reader.sample(63).unwrap().sequence,
                ((index + 1) * 64) as u64
            );
            assert_eq!(generation.observer.committed(), None);
        }
    });
    assert_eq!((allocations.allocations, allocations.reallocations), (0, 0));
}
#[test]
fn batch_requires_all_exact_unique_generations_within_resource_ceiling() {
    use RecordingBatchRefusal as E;
    let contract = ObservationContract::prepare().unwrap();
    let mut refs = references(65, &contract);
    let make = |samples, refs: &[c::BoundedResourceRef], slots, bytes| {
        PreparedRecordingBatch::prepare(snapshot_context(), &contract, samples, refs, slots, bytes)
    };
    let batch = make(65, &refs, 2, 8192).unwrap();
    assert_eq!(
        batch
            .generations()
            .iter()
            .map(|g| g.samples)
            .collect::<Vec<_>>(),
        [64, 1]
    );
    assert_eq!(make(0, &[], 2, 8192).err(), Some(E::Capacity));
    assert_eq!(make(u64::MAX, &refs, 64, 1048576).err(), Some(E::Capacity));
    assert_eq!(make(65, &refs, 1, 8192).err(), Some(E::Capacity));
    assert_eq!(make(65, &refs[..1], 2, 8192).err(), Some(E::Generations));
    assert_eq!(
        make(65, &refs, 2, batch.content_bytes() - 1).err(),
        Some(E::Capacity)
    );
    refs[1].lifetime.version = refs[0].lifetime.version;
    assert_eq!(make(65, &refs, 2, 8192).err(), Some(E::DuplicateGeneration));
    refs[1].lifetime.version = c::ResourceVersionIdentity::from_digest([2; 32]);
    refs[1].extent.bytes += 1;
    assert_eq!(
        make(65, &refs, 2, 8192).err(),
        Some(E::Snapshot(RecordingSnapshotRefusal::Extent))
    );
}
