//! Remote evidence storage follows its own admitted capacity.
use super::*;
#[test]
fn remote_backing_can_be_smaller_without_reducing_ordinary_evidence() {
    let bytes = 64 * core::mem::size_of::<KernelEvent>() as u32;
    let mut log = FixedSignLog::<64, 6>::new_with_remote_storage(
        bytes,
        6,
        remote_sign_storage_bytes(6).unwrap(),
    )
    .unwrap();
    assert_eq!(log.item_capacity(), 64);
    for sequence in 0..6 {
        log.record_remote(
            NodeId(0),
            PortId(0),
            KernelEventKind::RemoteInputAdmitted,
            RemoteLifecycleIdentity {
                endpoint: RemoteEndpointId(0),
                cord: CordId(0),
                direction: RemoteCordDirection::Ingress,
                sequence,
            },
        )
        .unwrap();
    }
    let before = log.len();
    assert_eq!(
        log.ensure_remote_capacity(1),
        Err(SignError::RemoteItemCapacityExceeded)
    );
    assert_eq!(log.len(), before);
    for sequence in 0..6 {
        assert_eq!(
            log.remote_identity(sequence).unwrap().sequence,
            u64::from(sequence)
        );
    }
    assert!(FixedSignLog::<64, 6>::new_with_remote_storage(
        bytes,
        7,
        remote_sign_storage_bytes(7).unwrap()
    )
    .is_err());
    assert_eq!(
        core::mem::size_of::<FixedSignLog<64>>() - core::mem::size_of::<FixedSignLog<64, 6>>(),
        58 * core::mem::size_of::<Option<RemoteLifecycleSign>>()
    );
}
#[test]
fn a_local_log_needs_no_remote_array() {
    let mut log =
        FixedSignLog::<8, 0>::new(8 * core::mem::size_of::<KernelEvent>() as u32).unwrap();
    assert_eq!(
        log.ensure_remote_capacity(1),
        Err(SignError::RemoteItemCapacityExceeded)
    );
    log.record(
        NodeId(0),
        Some(PortId(0)),
        None,
        KernelEventKind::StepFuelGranted,
    )
    .unwrap();
    assert_eq!(log.len(), 1);
}
