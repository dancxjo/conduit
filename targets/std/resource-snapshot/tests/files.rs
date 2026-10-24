#![cfg(target_os = "linux")]
use conduit_core::{self as c, AuthorityBinding};
use conduit_linux_snapshot_store::{FileSnapshotRefusal as E, PreparedFileSnapshot};
use conduit_resource_snapshot::{
    PreparedSnapshotRecord, SnapshotRefusal, MAXIMUM_RECORD_BYTES, PUBLISH_OPERATION,
    READ_OPERATION,
};
use std::{fs, path::PathBuf};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "../../../../mechanisms/io/resource-snapshot/tests/support/fixture.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "support/directory.rs"]
mod directory;
use directory::Root;
fn published_path(
    root: &Root,
    placement: &c::PlannedGear,
    reference: &c::BoundedResourceRef,
) -> PathBuf {
    let record = PreparedSnapshotRecord::prepare(placement, reference).unwrap();
    root.path
        .join(record.storage_key().strip_prefix("resource/").unwrap())
}
fn authority(placement: &c::PlannedGear) -> &AuthorityBinding {
    &placement.authority[0]
}
#[test]
fn publishes_and_reopens_an_exact_generation_under_new_boot_authority_without_play_allocations() {
    let root = Root::prepare();
    let (write, reference) = fixture::placement("boot/one", true, 2);
    {
        let directory = root.directory();
        let mut operation = PreparedFileSnapshot::prepare(&directory, &write, &reference).unwrap();
        let (result, memory) =
            allocation_probe::observe(|| operation.publish(authority(&write), b"[]"));
        result.unwrap();
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
        assert!(matches!(
            operation.publish(authority(&write), b"[]"),
            Err(E::AlreadyAttempted)
        ));
    }
    let (read, _) = fixture::placement("boot/two", false, 2);
    let directory = root.directory();
    let mut operation = PreparedFileSnapshot::prepare(&directory, &read, &reference).unwrap();
    let (_, memory) = allocation_probe::observe(|| {
        assert_eq!(operation.read(authority(&read)).unwrap(), b"[]");
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(fs::read_dir(&root.path).unwrap().count(), 1);
}
#[test]
fn wrong_authority_and_extent_refuse_before_filesystem_work_and_preserve_preparation() {
    let root = Root::prepare();
    let directory = root.directory();
    let (write, reference) = fixture::placement("boot/current", true, 2);
    let mut operation = PreparedFileSnapshot::prepare(&directory, &write, &reference).unwrap();
    let mut stale = authority(&write).clone();
    stale.boot_id = "boot/old".into();
    assert!(matches!(
        operation.publish(&stale, b"[]"),
        Err(E::Binding(SnapshotRefusal::StaleBoot))
    ));
    assert!(matches!(
        operation.publish(authority(&write), b"[0]"),
        Err(E::Binding(SnapshotRefusal::ContentExtent))
    ));
    assert_eq!(fs::read_dir(&root.path).unwrap().count(), 0);
    operation.publish(authority(&write), b"[]").unwrap();
    let (read, _) = fixture::placement("boot/current", false, 2);
    let mut reader = PreparedFileSnapshot::prepare(&directory, &read, &reference).unwrap();
    let mut stale = authority(&read).clone();
    stale.boot_id = "boot/old".into();
    assert!(matches!(
        reader.read(&stale),
        Err(E::Binding(SnapshotRefusal::StaleBoot))
    ));
    assert_eq!(reader.read(authority(&read)).unwrap(), b"[]");
}
#[test]
fn a_second_publication_never_replaces_the_committed_generation() {
    let root = Root::prepare();
    let directory = root.directory();
    let (write, reference) = fixture::placement("boot/current", true, 2);
    PreparedFileSnapshot::prepare(&directory, &write, &reference)
        .unwrap()
        .publish(authority(&write), b"[]")
        .unwrap();
    let path = published_path(&root, &write, &reference);
    let before = fs::read(&path).unwrap();
    let mut duplicate = PreparedFileSnapshot::prepare(&directory, &write, &reference).unwrap();
    assert!(matches!(
        duplicate.publish(authority(&write), b"{}"),
        Err(E::PublishedNameExists)
    ));
    assert_eq!(fs::read(path).unwrap(), before);
    assert!(matches!(
        duplicate.publish(authority(&write), b"{}"),
        Err(E::AlreadyAttempted)
    ));
}
#[test]
fn missing_symlink_oversized_and_corrupt_records_cannot_be_restored() {
    use std::os::unix::fs::symlink;
    let root = Root::prepare();
    let directory = root.directory();
    let (read, reference) = fixture::placement("boot/current", false, 2);
    let path = published_path(&root, &read, &reference);
    let mut operation = PreparedFileSnapshot::prepare(&directory, &read, &reference).unwrap();
    assert!(matches!(operation.read(authority(&read)), Err(E::Missing)));
    symlink("/etc/passwd", &path).unwrap();
    assert!(operation.read(authority(&read)).is_err());
    fs::remove_file(&path).unwrap();
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let name = CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: this is a terminated name inside the test's private directory.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(matches!(
        operation.read(authority(&read)),
        Err(E::InvalidFile)
    ));
    fs::remove_file(&path).unwrap();
    fs::write(&path, vec![0; MAXIMUM_RECORD_BYTES + 1]).unwrap();
    assert!(matches!(
        operation.read(authority(&read)),
        Err(E::InvalidFile)
    ));
    fs::write(&path, b"bad record").unwrap();
    assert!(matches!(
        operation.read(authority(&read)),
        Err(E::Binding(SnapshotRefusal::CorruptRecord))
    ));
}
#[test]
fn abandoned_candidate_is_explicit_and_never_silently_retried_or_deleted() {
    let root = Root::prepare();
    let directory = root.directory();
    let (write, reference) = fixture::placement("boot/current", true, 2);
    let path = published_path(&root, &write, &reference);
    let pending = root.path.join(format!(
        ".pending-{}",
        path.file_name().unwrap().to_str().unwrap()
    ));
    fs::write(&pending, b"abandoned").unwrap();
    let mut operation = PreparedFileSnapshot::prepare(&directory, &write, &reference).unwrap();
    assert!(matches!(
        operation.publish(authority(&write), b"[]"),
        Err(E::CandidateNameExists)
    ));
    assert!(matches!(
        operation.publish(authority(&write), b"[]"),
        Err(E::AlreadyAttempted)
    ));
    assert_eq!(fs::read(pending).unwrap(), b"abandoned");
    assert!(!path.exists());
}
#[test]
fn retained_directory_custody_survives_path_rename() {
    let mut root = Root::prepare();
    let directory = root.directory();
    let moved = root.path.with_extension("moved");
    fs::rename(&root.path, &moved).unwrap();
    root.path = moved;
    let (write, reference) = fixture::placement("boot/current", true, 2);
    PreparedFileSnapshot::prepare(&directory, &write, &reference)
        .unwrap()
        .publish(authority(&write), b"[]")
        .unwrap();
    assert!(published_path(&root, &write, &reference).exists());
}
