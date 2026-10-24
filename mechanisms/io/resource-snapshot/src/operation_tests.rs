//! Exact sealed-placement fixtures; no browser storage or Body start is claimed.
use super::*;
use alloc::{format, vec, vec::Vec};
use conduit_core::{ConfigurationEntry, ConfigurationValue};
use conduit_kernel::{BoundedValueRef, ValueRef};
mod allocation_probe {
    use std::thread_local;
    include!("../../../../architecture/plot/tests/common/allocation_probe.rs");
}
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn prepare(write: bool, bytes: usize, boot: &str) -> (PreparedSnapshotOperation, Vec<u8>) {
    let (mut placement, reference) = super::super::tests::placement(boot, write, bytes);
    let encoded = reference.encode().unwrap();
    let hex = encoded.iter().map(|byte| format!("{byte:02x}")).collect();
    placement.configuration = vec![ConfigurationEntry {
        key: "reference".into(),
        value: ConfigurationValue::Text(hex),
    }];
    (
        PreparedSnapshotOperation::prepare(&placement, NodeId(7)).unwrap(),
        encoded,
    )
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
#[test]
fn publication_and_new_boot_restoration_correlate_without_allocations() {
    let (mut writer, reference) = prepare(true, 2, "boot/old");
    let (mut reader, _) = prepare(false, 2, "boot/new");
    let publish = request(1, 2, true);
    let read = request(1, reference.len(), false);
    let mut bytes = [0; MAXIMUM_RECORD_BYTES];
    let ((), memory) = allocation_probe::observe(|| {
        writer.begin(publish, b"[]").unwrap();
        let effect = writer.describe(publish).unwrap();
        assert_eq!(effect.effect_kind, "resource-publish");
        let record = effect.record.unwrap();
        let length = record.len();
        bytes[..length].copy_from_slice(record);
        assert_eq!(writer.completion(publish, Ok(None)).unwrap(), reference);
        // The adapter validated bytes; an actual kernel commit is independent.
        writer.finish(publish).unwrap();
        assert_eq!(writer.begin(publish, b"[]"), Err(lifecycle()));
        reader.begin(read, &reference).unwrap();
        let effect = reader.describe(read).unwrap();
        assert_eq!(effect.effect_kind, "resource-read");
        assert!(effect.record.is_none());
        assert_eq!(
            reader.completion(read, Ok(Some(&bytes[..length]))).unwrap(),
            b"[]"
        );
        reader.finish(read).unwrap();
        assert_eq!(
            reader.completion(read, Ok(Some(&bytes[..length]))),
            Err(lifecycle())
        );
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn foreign_overlap_replay_and_storage_failures_preserve_pending_identity() {
    let (mut operation, _) = prepare(true, 2, "boot/current");
    let current = request(1, 2, true);
    let next = request(2, 2, true);
    let failure = Failure {
        code: FailureCode::HostCallFailed,
        detail: 211,
    };
    let ((), memory) = allocation_probe::observe(|| {
        for foreign in [
            HostCallRequest {
                node: NodeId(8),
                ..current
            },
            HostCallRequest {
                call: HostCallId(1),
                ..current
            },
        ] {
            assert!(operation.begin(foreign, b"[]").is_err());
        }
        operation.begin(current, b"[]").unwrap();
        assert_eq!(operation.begin(next, b"[]"), Err(lifecycle()));
        let mut forged = current;
        forged.input.value.generation += 1;
        assert!(operation.describe(forged).is_err());
        assert_eq!(operation.completion(forged, Ok(None)), Err(lifecycle()));
        assert_eq!(operation.finish(forged), Err(lifecycle()));
        assert_eq!(operation.completion(current, Err(failure)), Err(failure));
        assert_eq!(
            operation.completion(current, Ok(Some(b"[]"))),
            Err(Failure {
                code: FailureCode::HostCallFailed,
                detail: 209
            })
        );
        operation.finish(current).unwrap();
        operation.begin(next, b"[]").unwrap();
        assert_eq!(operation.finish(current), Err(lifecycle()));
        assert!(operation.describe(next).is_ok());
        operation.finish(next).unwrap();
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn invalid_json_or_reference_cannot_begin_storage() {
    let (mut writer, _) = prepare(true, 3, "boot/current");
    let (mut reader, mut reference) = prepare(false, 3, "boot/current");
    reference[0] ^= 1;
    let publish = request(1, 3, true);
    let read = request(1, reference.len(), false);
    let ((), memory) = allocation_probe::observe(|| {
        assert!(writer.begin(publish, b"[x]").is_err());
        assert!(writer.describe(publish).is_err());
        assert!(reader.begin(read, &reference).is_err());
        assert!(reader.describe(read).is_err());
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
