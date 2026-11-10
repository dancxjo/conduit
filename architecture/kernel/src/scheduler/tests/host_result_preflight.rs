//! A synchronous adapter can check publication custody before dispatching an effect.
use super::*;
use crate::{SignError, StorageError};
fn signs() -> FixedSignLog<64> {
    FixedSignLog::new((64 * core::mem::size_of::<crate::KernelEvent>()) as u32).unwrap()
}

#[test]
fn result_preflight_preserves_the_exact_undispatched_request_and_values() {
    let mut kernel = host_scheduler(FixedValueStore::<2, 4>::new(5).unwrap(), signs());
    kernel.step().unwrap();
    kernel.step().unwrap();
    let before = (
        kernel.values().used_items(),
        kernel.values().used_bytes(),
        kernel.signs().len(),
    );
    assert_eq!(kernel.pending_host_call_count(), 1);
    for _ in 0..3 {
        assert_eq!(kernel.preflight_host_result_storage(4), Ok(()));
        assert_eq!(
            kernel.preflight_host_result_storage(5),
            Err(SchedulerError::Storage(StorageError::ValueTooLarge))
        );
        assert_eq!(
            (
                kernel.values().used_items(),
                kernel.values().used_bytes(),
                kernel.signs().len()
            ),
            before
        );
        assert_eq!(kernel.pending_host_call_count(), 1);
    }
    let request = kernel.next_host_request().unwrap();
    assert_eq!(kernel.host_value(request.input.value).unwrap(), &[3]);
    assert!(kernel.next_host_request().is_none());
    // Synchronous publication can consume the checked remaining capacity.
    let result = kernel.store_host_value(&[7; 4]).unwrap();
    assert_eq!(kernel.host_value(result).unwrap(), &[7; 4]);
    assert_eq!(kernel.host_value(request.input.value).unwrap(), &[3]);
}

#[test]
fn result_preflight_keeps_item_byte_and_sign_pressure_distinct() {
    let mut bytes = host_scheduler(FixedValueStore::<2, 4>::new(1).unwrap(), signs());
    bytes.step().unwrap();
    bytes.step().unwrap();
    assert_eq!(
        bytes.preflight_host_result_storage(1),
        Err(SchedulerError::Storage(StorageError::ByteCapacityExceeded))
    );
    assert!(bytes.next_host_request().is_some());

    let mut items = host_scheduler(FixedValueStore::<1, 4>::new(4).unwrap(), signs());
    items.step().unwrap();
    items.step().unwrap();
    assert_eq!(
        items.preflight_host_result_storage(1),
        Err(SchedulerError::Storage(StorageError::ItemCapacityExceeded))
    );
    assert!(items.next_host_request().is_some());

    let mut evidence = host_scheduler(FixedValueStore::<2, 4>::new(5).unwrap(), signs());
    evidence.step().unwrap();
    evidence.step().unwrap();
    // Fill retained evidence independently of value capacity. Mandatory Sign
    // pressure must refuse before an adapter marks the pending call dispatched.
    while evidence.signs.ensure_capacity(1).is_ok() {
        evidence
            .signs
            .record(NodeId(0), None, None, KernelEventKind::RunCancelled)
            .unwrap();
    }
    let refused = evidence.preflight_host_result_storage(4);
    assert!(matches!(
        refused,
        Err(SchedulerError::Sign(
            SignError::ItemCapacityExceeded | SignError::ByteCapacityExceeded
        ))
    ));
    assert!(evidence.next_host_request().is_some());
    assert_eq!(evidence.values().used_items(), 1);
}

#[test]
fn result_preflight_refuses_cancelled_work() {
    let mut kernel = host_scheduler(FixedValueStore::<2, 4>::new(5).unwrap(), signs());
    kernel.cancel().unwrap();
    assert_eq!(
        kernel.preflight_host_result_storage(4),
        Err(SchedulerError::Cancelled)
    );
}

#[test]
#[cfg(feature = "alloc")]
fn result_preflight_refuses_unsupported_representation_profiles() {
    use crate::NativeU64ValueStore;
    let values =
        NativeU64ValueStore::<_, 1>::new(FixedValueStore::<2, 4>::new(5).unwrap(), 8).unwrap();
    let kernel = host_scheduler(values, signs());
    assert_eq!(
        kernel.preflight_host_result_storage(1),
        Err(SchedulerError::Storage(
            StorageError::RepresentationMismatch
        ))
    );
    assert_eq!(kernel.values().used_items(), 1);
}
