use super::*;
use crate::{FixedSignLog, HostedSignLog, SignQuery, SignSink};
use alloc::vec::Vec;
const KINDS: [KernelEventKind; 23] = [
    KernelEventKind::StepFuelGranted,
    KernelEventKind::StepYielded,
    KernelEventKind::StepFuelExceeded,
    KernelEventKind::ValueStored,
    KernelEventKind::ValueRouted,
    KernelEventKind::ValueConsumed,
    KernelEventKind::RemoteValueOffered,
    KernelEventKind::RemoteValueAccepted,
    KernelEventKind::RemoteValueDelivered,
    KernelEventKind::RemoteOutputClosed,
    KernelEventKind::RemoteInputAdmitted,
    KernelEventKind::RemoteInputClosed,
    KernelEventKind::InputClosed,
    KernelEventKind::InputAbnormal,
    KernelEventKind::HostCallRequested,
    KernelEventKind::HostCallCancellationRequested,
    KernelEventKind::HostCallCompleted,
    KernelEventKind::BackCompleted,
    KernelEventKind::BackFailed,
    KernelEventKind::SemanticAbnormal,
    KernelEventKind::SemanticAbnormalRecovered,
    KernelEventKind::CancellationRequested,
    KernelEventKind::RunCancelled,
];
#[test]
fn every_kind_and_full_identifier_range_round_trip_without_sentinels() {
    assert!(
        core::mem::size_of::<Option<PackedKernelEvent>>()
            < core::mem::size_of::<Option<KernelEvent>>()
    );
    for kind in KINDS {
        for sequence in [0, 1, u32::MAX] {
            for node in [0, 1, u16::MAX] {
                for port in [None, Some(PortId(0)), Some(PortId(u16::MAX))] {
                    for request in [None, Some(RequestId(0)), Some(RequestId(u32::MAX))] {
                        let event = KernelEvent {
                            sequence,
                            node: NodeId(node),
                            port,
                            request,
                            kind,
                        };
                        assert_eq!(KernelEvent::from(PackedKernelEvent::from(event)), event);
                    }
                }
            }
        }
    }
}
#[test]
fn packed_hosted_and_canonical_fixed_logs_match_under_pressure_and_refusal() {
    let charge = core::mem::size_of::<KernelEvent>() as u32;
    for slots in [1, 3, 7] {
        let mut fixed = FixedSignLog::<7, 3>::new_with_remote_storage(
            slots * charge,
            3,
            crate::remote_sign_storage_bytes(3).unwrap(),
        )
        .unwrap();
        let mut hosted = HostedSignLog::new_with_remote_storage(
            7,
            slots * charge,
            3,
            crate::remote_sign_storage_bytes(3).unwrap(),
        )
        .unwrap();
        for index in 0..200u32 {
            let kind = if index % 11 == 0 {
                KINDS[(index as usize / 11) % KINDS.len()]
            } else {
                KernelEventKind::ValueStored
            };
            assert_eq!(fixed.ensure_capacity(2), hosted.ensure_capacity(2));
            assert_eq!(
                fixed.record(
                    NodeId(u16::MAX),
                    Some(PortId(u16::MAX)),
                    Some(RequestId(index)),
                    kind
                ),
                hosted.record(
                    NodeId(u16::MAX),
                    Some(PortId(u16::MAX)),
                    Some(RequestId(index)),
                    kind
                )
            );
            assert_eq!(
                fixed.events().collect::<Vec<_>>(),
                hosted.events().collect::<Vec<_>>()
            );
            assert_eq!(fixed.used_bytes(), hosted.used_bytes());
            assert_eq!(fixed.retention_gap(), hosted.retention_gap());
            for kind in KINDS {
                assert_eq!(fixed.contains_kind(kind), hosted.contains_kind(kind));
            }
        }
        let identity = crate::RemoteLifecycleIdentity {
            endpoint: crate::RemoteEndpointId(u16::MAX),
            cord: crate::CordId(u16::MAX),
            direction: crate::RemoteCordDirection::Ingress,
            sequence: u64::MAX,
        };
        for _ in 0..5 {
            assert_eq!(
                fixed.record_remote(
                    NodeId(0),
                    PortId(0),
                    KernelEventKind::RemoteValueOffered,
                    identity
                ),
                hosted.record_remote(
                    NodeId(0),
                    PortId(0),
                    KernelEventKind::RemoteValueOffered,
                    identity
                )
            );
            assert_eq!(
                fixed.events().collect::<Vec<_>>(),
                hosted.events().collect::<Vec<_>>()
            );
            for seq in 0..201 {
                assert_eq!(fixed.remote_identity(seq), hosted.remote_identity(seq));
            }
        }
    }
}
