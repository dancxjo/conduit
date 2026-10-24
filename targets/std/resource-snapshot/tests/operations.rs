#![cfg(target_os = "linux")]
//! Exact placement and numeric request fixtures; actual file effects. This is
//! not an admitted Body, started kernel, Line, capacity reservation or store offer.
use conduit_core::{ConfigurationEntry, ConfigurationValue};
use conduit_kernel::{
    scheduler::HostCallRequest, BoundedValueRef, FailureCode, HostCallId, NodeId, RequestId,
    ValueRef,
};
use conduit_linux_snapshot_store::{
    FileOperationRefusal as E, FileSnapshotRefusal, PreparedFileSnapshot,
    PreparedFileSnapshotOperation,
};
use conduit_resource_snapshot::{PUBLISH_OPERATION, READ_OPERATION};
use std::fs;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/directory.rs"]
mod directory;
#[path = "../../../../mechanisms/io/resource-snapshot/tests/support/fixture.rs"]
mod fixture;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn selected(write: bool, boot: &str) -> (conduit_core::PlannedGear, Vec<u8>) {
    let (mut placement, reference) = fixture::placement(boot, write, 2);
    let encoded = reference.encode().unwrap();
    let hex = encoded.iter().map(|b| format!("{b:02x}")).collect();
    placement.configuration = vec![ConfigurationEntry {
        key: "reference".into(),
        value: ConfigurationValue::Text(hex),
    }];
    (placement, encoded)
}
fn request(sequence: u32, bytes: usize, write: bool) -> HostCallRequest {
    HostCallRequest {
        node: NodeId(7),
        request: RequestId(sequence),
        call: HostCallId(0),
        input: BoundedValueRef::new(
            ValueRef {
                slot: 0,
                generation: 1,
                byte_len: bytes as u32,
            },
            if write { 4096 } else { 512 },
        )
        .unwrap(),
    }
}
fn lifecycle(result: Result<&[u8], E>) {
    assert!(matches!(result, Err(E::Operation(e)) if e.code==FailureCode::InvalidLifecycle));
}
#[test]
fn typed_file_effects_preserve_request_and_completion_without_repeating_io_or_allocating() {
    let root = directory::Root::prepare();
    let directory = root.directory();
    let (write, reference) = selected(true, "boot/old");
    let mut writer = PreparedFileSnapshotOperation::prepare(&directory, &write, NodeId(7)).unwrap();
    let publish = request(1, 2, true);
    let (_, memory) = allocation_probe::observe(|| {
        assert_eq!(writer.perform(publish, b"[]").unwrap(), reference);
        assert_eq!(writer.completion(publish).unwrap(), reference);
        lifecycle(writer.perform(publish, b"[]"));
        let foreign = request(2, 2, true);
        assert!(writer.finish(foreign).is_err());
        assert_eq!(writer.completion(publish).unwrap(), reference);
        writer.finish(publish).unwrap();
        lifecycle(writer.completion(publish));
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(fs::read_dir(&root.path).unwrap().count(), 1);
    let (read, _) = selected(false, "boot/new");
    let mut reader = PreparedFileSnapshotOperation::prepare(&directory, &read, NodeId(7)).unwrap();
    let read = request(1, reference.len(), false);
    let (_, memory) = allocation_probe::observe(|| {
        assert_eq!(reader.perform(read, &reference).unwrap(), b"[]");
        assert_eq!(reader.completion(read).unwrap(), b"[]");
        reader.finish(read).unwrap();
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn invalid_json_and_foreign_requests_cannot_reach_file_effects() {
    let root = directory::Root::prepare();
    let directory = root.directory();
    let (write, _) = selected(true, "boot/current");
    let mut operation =
        PreparedFileSnapshotOperation::prepare(&directory, &write, NodeId(7)).unwrap();
    let publish = request(1, 2, true);
    assert!(
        matches!(operation.perform(publish, b"xx"), Err(E::Operation(e)) if e.code==FailureCode::InvalidInput)
    );
    let mut foreign = publish;
    foreign.node = NodeId(8);
    assert!(
        matches!(operation.perform(foreign, b"[]"), Err(E::Operation(e)) if e.code==FailureCode::InvalidInput)
    );
    assert_eq!(fs::read_dir(&root.path).unwrap().count(), 0);
    let (read, reference) = selected(false, "boot/current");
    let mut reader = PreparedFileSnapshotOperation::prepare(&directory, &read, NodeId(7)).unwrap();
    let mut changed = reference.clone();
    changed[0] ^= 1;
    assert!(
        matches!(reader.perform(request(1, changed.len(), false), &changed), Err(E::Operation(e)) if e.code==FailureCode::InvalidInput)
    );
    assert_eq!(fs::read_dir(&root.path).unwrap().count(), 0);
}
#[test]
fn an_opaque_record_with_valid_hash_but_invalid_json_never_completes_a_typed_read() {
    let root = directory::Root::prepare();
    let directory = root.directory();
    let (write, reference) = fixture::placement("boot/old", true, 2);
    PreparedFileSnapshot::prepare(&directory, &write, &reference)
        .unwrap()
        .publish(&write.authority[0], b"xx")
        .unwrap();
    let (read, reference) = selected(false, "boot/new");
    let mut reader = PreparedFileSnapshotOperation::prepare(&directory, &read, NodeId(7)).unwrap();
    let read = request(1, reference.len(), false);
    assert!(
        matches!(reader.perform(read, &reference), Err(E::Operation(e)) if e.code==FailureCode::HostCallFailed && e.detail==214)
    );
    assert!(matches!(reader.completion(read), Err(E::Operation(e)) if e.detail==214));
    lifecycle(reader.perform(read, &reference));
    reader.finish(read).unwrap();
}
#[test]
fn storage_refusals_do_not_become_success_or_implicit_retry() {
    let root = directory::Root::prepare();
    let directory = root.directory();
    let (read, reference) = selected(false, "boot/current");
    let mut reader = PreparedFileSnapshotOperation::prepare(&directory, &read, NodeId(7)).unwrap();
    let read = request(1, reference.len(), false);
    assert!(matches!(
        reader.perform(read, &reference),
        Err(E::Storage(FileSnapshotRefusal::Missing))
    ));
    lifecycle(reader.completion(read));
    lifecycle(reader.perform(read, &reference));
    reader.finish(read).unwrap();
    let (write, reference) = fixture::placement("boot/current", true, 2);
    PreparedFileSnapshot::prepare(&directory, &write, &reference)
        .unwrap()
        .publish(&write.authority[0], b"[]")
        .unwrap();
    let (write, _) = selected(true, "boot/current");
    let mut writer = PreparedFileSnapshotOperation::prepare(&directory, &write, NodeId(7)).unwrap();
    let write = request(1, 2, true);
    assert!(matches!(
        writer.perform(write, b"{}"),
        Err(E::Storage(FileSnapshotRefusal::PublishedNameExists))
    ));
    lifecycle(writer.completion(write));
    lifecycle(writer.perform(write, b"{}"));
    writer.finish(write).unwrap();
}
