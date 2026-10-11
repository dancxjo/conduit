//! Deterministic admitted-owner/ring fault conformance, below native physics.
//! This fixture supplies stop acknowledgement explicitly; no mapped controller
//! or physical quiescence is inferred from a software fault observation.
use super::*;
use crate::usb_base::endpoint_ring::EndpointRingCursor;
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};

#[test]
fn stalled_and_lost_transfers_retain_dma_until_ack_then_publish_once() {
    for (disposition, tag) in [
        (EndpointWriteDisposition::Stalled, "stalled"),
        (EndpointWriteDisposition::ProviderLost, "provider-lost"),
    ] {
        let mut owner = fixture::owner(7);
        let node = owner.node;
        let mut cursor = EndpointRingCursor::new(64).unwrap();
        let submission = owner
            .begin(node, HostCallId(0), RequestId(0), &fixture::input())
            .unwrap();
        let reservation = cursor.reserve(8).unwrap();
        // A failed native stop cannot call either unsafe retirement entrance.
        assert!(owner.pending.is_some());
        assert!(cursor.ensure_idle().is_err());
        assert_eq!(
            owner
                .table
                .inspections()
                .next()
                .unwrap()
                .in_flight_operations,
            1
        );
        assert_eq!(
            owner
                .begin(node, HostCallId(0), RequestId(1), &fixture::input())
                .err(),
            Some(EndpointWriteOwnerRefusal::Pressure)
        );
        // Explicit deterministic native-stop acknowledgement settles both exact
        // retained identities. No success or fault output preceded this point.
        let mut observed = [0u8; 4096];
        let mut length = 0;
        let allocations = crate::test_allocations::allocations(|| {
            unsafe { cursor.complete_quiesced(&reservation) }.unwrap();
            let bytes = unsafe {
                owner.finish_quiesced(
                    &submission,
                    NativeEndpointWriteObservation::Disposition(disposition),
                )
            }
            .unwrap();
            observed[..bytes.len()].copy_from_slice(bytes);
            length = bytes.len();
        });
        assert_eq!(allocations, 0);
        let value = StructuredInfoValue::from_canonical_bytes(&observed[..length]).unwrap();
        assert!(
            matches!(value.shape(), StructuredInfoValueShape::Variant {tag: actual, ..} if actual == tag)
        );
        assert!(owner.pending.is_none());
        cursor.ensure_idle().unwrap();
        assert_eq!(
            owner
                .table
                .inspections()
                .next()
                .unwrap()
                .completed_operations,
            1
        );
        assert!(unsafe { cursor.complete_quiesced(&reservation) }.is_err());
        assert_eq!(
            unsafe {
                owner.finish_quiesced(
                    &submission,
                    NativeEndpointWriteObservation::Completed { actual: 8 },
                )
            }
            .err(),
            Some(EndpointWriteOwnerRefusal::StaleTransfer)
        );
    }
}

#[test]
fn revoked_fault_lease_cannot_publish_terminal_or_late_success() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &fixture::input())
        .unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.pending.is_some());
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointWriteObservation::Disposition(EndpointWriteDisposition::Stalled),
            )
        }
        .err(),
        Some(EndpointWriteOwnerRefusal::Capability(
            BaseCapabilityRefusal::StaleCompletion
        ))
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        0
    );
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeEndpointWriteObservation::Completed { actual: 8 },
            )
        }
        .err(),
        Some(EndpointWriteOwnerRefusal::StaleTransfer)
    );
}
