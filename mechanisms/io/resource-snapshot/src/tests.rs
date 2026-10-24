use super::*;
use conduit_core::*;

#[test]
fn snapshot_owner_metadata_must_fit_the_prepared_record_envelope() {
    for length in [0, 129, 1024] {
        let (mut write, reference) = placement("boot/current", true, MAXIMUM_SNAPSHOT_BYTES);
        let oversized: HostId = "x".repeat(length).into();
        write.host_id = oversized.clone();
        write.authority[0].host_id = oversized.clone();
        write.resources[0].content.as_mut().unwrap().owner_host = oversized;
        assert!(matches!(
            PreparedSnapshotRecord::prepare(&write, &reference),
            Err(SnapshotRefusal::Reference(
                ResourceContentRefusal::InvalidContract
            ))
        ));
    }
}

#[path = "../tests/support/fixture.rs"]
mod fixture;
pub(crate) use fixture::placement;

#[test]
fn snapshot_record_preserves_generation_across_new_boot_but_requires_new_authority() {
    let (write, reference) = placement("boot/one", true, 2);
    let mut writer = PreparedSnapshotRecord::prepare(&write, &reference).unwrap();
    let bytes = writer
        .publication(&write.authority[0], b"[]")
        .unwrap()
        .to_vec();
    let (old_read, _) = placement("boot/one", false, 2);
    let (read, _) = placement("boot/two", false, 2);
    let reader = PreparedSnapshotRecord::prepare(&read, &reference).unwrap();
    assert_eq!(writer.storage_key(), reader.storage_key());
    assert_eq!(reader.restore(&read.authority[0], &bytes).unwrap(), b"[]");
    assert_eq!(
        reader.restore(&old_read.authority[0], &bytes),
        Err(SnapshotRefusal::StaleBoot)
    );
    let mut foreign = read.authority[0].clone();
    foreign.host_id = "foreign".into();
    assert_eq!(
        reader.restore(&foreign, &bytes),
        Err(SnapshotRefusal::ForeignHost)
    );
    let mut corrupt = bytes.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert_eq!(
        reader.restore(&read.authority[0], &corrupt),
        Err(SnapshotRefusal::CorruptRecord)
    );
    assert_eq!(
        reader.restore(&read.authority[0], &bytes[..bytes.len() - 1]),
        Err(SnapshotRefusal::CorruptRecord)
    );
}

#[test]
fn snapshot_binding_refuses_wrong_bounds_authority_and_lifetime() {
    let (mut write, reference) = placement("boot/one", true, 2);
    write.host_calls[0].maximum_input_bytes = 1;
    assert!(matches!(
        PreparedSnapshotRecord::prepare(&write, &reference),
        Err(SnapshotRefusal::InvalidBinding)
    ));
    write.host_calls[0].maximum_input_bytes = 4096;
    write.authority[0].capability_id = "wrong".into();
    assert!(matches!(
        PreparedSnapshotRecord::prepare(&write, &reference),
        Err(SnapshotRefusal::AuthorityDenied)
    ));
    let (mut write, _) = placement("boot/one", true, 2);
    write.resources[0]
        .content
        .as_mut()
        .unwrap()
        .contract
        .retention = ResourceRetention::Play;
    assert!(matches!(
        PreparedSnapshotRecord::prepare(&write, &reference),
        Err(SnapshotRefusal::UnsupportedLifetime)
    ));
}

#[test]
fn snapshot_generation_key_cannot_be_changed_by_claiming_a_different_extent() {
    let (one, first) = placement("boot/one", true, 2);
    let (two, second) = placement("boot/one", true, 3);
    let mut one = PreparedSnapshotRecord::prepare(&one, &first).unwrap();
    let two = PreparedSnapshotRecord::prepare(&two, &second).unwrap();
    assert_eq!(one.storage_key(), two.storage_key());
    let (placement, _) = placement("boot/one", true, 2);
    assert_eq!(
        one.publication(&placement.authority[0], b"[0]"),
        Err(SnapshotRefusal::ContentExtent)
    );
}

#[test]
fn snapshot_maximum_record_is_bounded_and_rejects_an_unrelated_authority_contract() {
    let (mut write, reference) = placement("boot/one", true, MAXIMUM_SNAPSHOT_BYTES);
    write.authority[0].contract_id = "authority/unrelated@1".into();
    assert!(matches!(
        PreparedSnapshotRecord::prepare(&write, &reference),
        Err(SnapshotRefusal::AuthorityDenied)
    ));
    write.authority[0].contract_id = AUTHORITY_CONTRACT.into();
    let mut writer = PreparedSnapshotRecord::prepare(&write, &reference).unwrap();
    let content = [42; MAXIMUM_SNAPSHOT_BYTES];
    let record = writer
        .publication(&write.authority[0], &content)
        .unwrap()
        .to_vec();
    assert!(record.len() <= MAXIMUM_RECORD_BYTES);
    let (read, _) = placement("boot/two", false, MAXIMUM_SNAPSHOT_BYTES);
    let reader = PreparedSnapshotRecord::prepare(&read, &reference).unwrap();
    assert_eq!(
        reader.restore(&read.authority[0], &record).unwrap(),
        &content
    );
}
