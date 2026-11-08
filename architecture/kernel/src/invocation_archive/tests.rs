extern crate std;
use super::*;
use crate::{CordId, KernelEventKind, NodeId, PortId, RemoteCordDirection, RemoteEndpointId};
#[derive(Clone)]
struct Evidence {
    signs: [ArchivedSign; 3],
    count: usize,
    gap: Option<SignRetentionGap>,
}
impl SignQuery for Evidence {
    fn contains_kind(&self, kind: KernelEventKind) -> bool {
        self.events().any(|e| e.kind == kind)
    }
    fn remote_identity(&self, sequence: u32) -> Option<RemoteLifecycleIdentity> {
        self.signs[..self.count]
            .iter()
            .find(|e| e.event.sequence == sequence)
            .and_then(|e| e.remote)
    }
    fn retention_gap(&self) -> Option<SignRetentionGap> {
        self.gap
    }
}
impl InvocationEvidence for Evidence {
    fn events(&self) -> impl Iterator<Item = KernelEvent> {
        self.signs[..self.count].iter().map(|e| e.event)
    }
}
fn trace(ordinal: u32) -> Evidence {
    Evidence {
        signs: core::array::from_fn(|i| ArchivedSign {
            event: KernelEvent {
                sequence: i as u32,
                node: NodeId(1),
                port: Some(PortId(0)),
                request: (i != 0).then_some(RequestId(7 + ordinal * 3)),
                kind: KernelEventKind::RemoteValueDelivered,
            },
            remote: Some(RemoteLifecycleIdentity {
                endpoint: RemoteEndpointId(2),
                cord: CordId(i as u16),
                direction: RemoteCordDirection::Egress,
                sequence: u64::from(ordinal) * [1, 2, 0][i],
            }),
        }),
        count: 3,
        gap: Some(SignRetentionGap {
            first_sequence: 4,
            last_sequence: 6,
            entries: 3,
        }),
    }
}
#[test]
fn reconstructs_960_complete_traces_with_independent_remote_streams() {
    let mut archive = InvocationArchive::<3, 1, 3>::new();
    for ordinal in 0..960 {
        let signs = trace(ordinal);
        archive.can_append(&signs, 11, 3).unwrap();
        archive.append(&signs, 11, 3).unwrap();
    }
    assert_eq!(archive.len(), 960);
    for ordinal in 0..960 {
        let invocation = archive.invocation(ordinal).unwrap();
        assert_eq!(
            invocation.metadata,
            InvocationMetadata {
                gap: trace(ordinal).gap,
                decisions: 11,
                logical_invocations: 3
            }
        );
        assert!(invocation.eq(trace(ordinal).signs));
    }
    assert!(archive.invocation(960).is_none());
}
#[test]
fn variations_fall_back_or_refuse_atomically() {
    for variation in 0..15 {
        let mut changed = trace(2);
        let mut decisions = 11;
        let mut logical = 3;
        match variation {
            0 => changed.signs[0].event.sequence = 7,
            1 => changed.signs[0].event.node = NodeId(9),
            2 => changed.signs[0].event.port = None,
            3 => changed.signs[0].event.kind = KernelEventKind::BackCompleted,
            4 => changed.signs[1].event.request = None,
            5 => changed.signs[2].event.request = Some(RequestId(14)),
            6 => changed.gap = None,
            7 => decisions += 1,
            8 => logical += 1,
            9 => changed.count = 2,
            10 => changed.signs[0].remote = None,
            11 => changed.signs[0].remote.as_mut().unwrap().endpoint = RemoteEndpointId(8),
            12 => changed.signs[0].remote.as_mut().unwrap().cord = CordId(8),
            13 => {
                changed.signs[0].remote.as_mut().unwrap().direction = RemoteCordDirection::Ingress
            }
            _ => changed.signs[2].remote.as_mut().unwrap().sequence = 1,
        }
        let mut archive = InvocationArchive::<6, 2, 3>::new();
        archive.append(&trace(0), 11, 3).unwrap();
        archive.append(&trace(1), 11, 3).unwrap();
        archive.append(&changed, decisions, logical).unwrap();
        assert_eq!(archive.run_len, 2);
        let stored = archive.invocation(2).unwrap();
        assert_eq!(
            stored.metadata,
            InvocationMetadata {
                gap: changed.gap,
                decisions,
                logical_invocations: logical
            }
        );
        assert!(stored.eq(changed.signs[..changed.count].iter().copied()));
        let mut full = InvocationArchive::<3, 1, 3>::new();
        full.append(&trace(0), 11, 3).unwrap();
        full.append(&trace(1), 11, 3).unwrap();
        assert_eq!(
            full.can_append(&changed, decisions, logical),
            Err(ArchiveRefusal::Capacity)
        );
        assert_eq!(
            full.append(&changed, decisions, logical),
            Err(ArchiveRefusal::Capacity)
        );
        assert_eq!(full.len(), 2);
        assert!(full.invocation(0).unwrap().eq(trace(0).signs));
        assert!(full.invocation(1).unwrap().eq(trace(1).signs));
    }
}
#[test]
fn checked_overflow_and_stream_limits_preserve_exact_evidence() {
    let mut archive = InvocationArchive::<9, 3, 3>::new();
    let mut a = trace(0);
    let mut b = a.clone();
    for sign in &mut a.signs {
        sign.event.request = Some(RequestId(0));
        sign.remote.as_mut().unwrap().sequence = 0;
    }
    for sign in &mut b.signs {
        sign.event.request = Some(RequestId(u32::MAX));
        sign.remote.as_mut().unwrap().sequence = u64::MAX;
    }
    archive.append(&a, 11, 3).unwrap();
    archive.append(&b, 11, 3).unwrap();
    archive.append(&a, 11, 3).unwrap(); // Checked multiplication overflow starts a new run.
    assert_eq!(archive.run_len, 2);
    assert!(archive.invocation(1).unwrap().eq(b.signs));
    assert!(archive.invocation(2).unwrap().eq(a.signs));
    archive.invocations = u32::MAX;
    assert_eq!(
        archive.append(&a, 11, 3),
        Err(ArchiveRefusal::InvocationOverflow)
    );
    assert_eq!(archive.invocations, u32::MAX);
    let mut narrow = InvocationArchive::<3, 1, 2>::new();
    assert_eq!(
        narrow.append(&a, 11, 3),
        Err(ArchiveRefusal::StreamCapacity)
    );
    assert!(narrow.is_empty());
    let mut empty = a;
    empty.count = 0;
    let mut zero = InvocationArchive::<0, 1, 0>::new();
    for _ in 0..960 {
        zero.append(&empty, 0, 0).unwrap();
    }
    assert_eq!(zero.invocation(959).unwrap().count(), 0);
}
#[test]
fn storage_fits_old_fixed_archive_budget() {
    type OldMetadata = (u32, Option<SignRetentionGap>, u32, u64, usize, usize);
    let old = core::mem::size_of::<[(u32, KernelEvent); 128]>()
        + core::mem::size_of::<[OldMetadata; 16]>()
        + 2 * core::mem::size_of::<usize>();
    let new = core::mem::size_of::<InvocationArchive<64, 4, 3>>();
    std::println!("archive storage: old={old} new={new}");
    assert!(new <= old);
}

#[test]
fn remote_delta_must_match_every_event_in_each_stream_without_wrapping() {
    let mut a = trace(0);
    for sign in &mut a.signs {
        sign.event.request = None;
        sign.remote.as_mut().unwrap().cord = CordId(0);
    }
    let mut b = a.clone();
    for sign in &mut b.signs {
        sign.remote.as_mut().unwrap().sequence = u64::MAX;
    }
    let mut archive = InvocationArchive::<9, 3, 1>::new();
    archive.append(&a, 11, 3).unwrap();
    archive.append(&b, 11, 3).unwrap();
    archive.append(&a, 11, 3).unwrap();
    assert_eq!(archive.run_len, 2); // u64 multiplication cannot wrap into run0.
    assert!(archive.invocation(1).unwrap().eq(b.signs));
    let mut mixed = a.clone();
    mixed.signs[1].remote.as_mut().unwrap().sequence = 2;
    mixed.signs[2].remote.as_mut().unwrap().sequence = 3;
    archive.append(&mixed, 11, 3).unwrap();
    assert_eq!(archive.run_len, 3); // One stream cannot learn two deltas.
    assert!(archive.invocation(3).unwrap().eq(mixed.signs));
}
