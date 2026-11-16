extern crate std;
use super::*;
use crate::{CordId, KernelEventKind, PortId, RemoteCordDirection, RemoteEndpointId, SignQuery};
use std::vec::Vec;
struct Evidence {
    signs: Vec<ArchivedSign>,
    gap: Option<SignRetentionGap>,
}
impl SignQuery for Evidence {
    fn contains_kind(&self, k: KernelEventKind) -> bool {
        self.signs.iter().any(|s| s.event.kind == k)
    }
    fn remote_identity(&self, seq: u32) -> Option<RemoteLifecycleIdentity> {
        self.signs
            .iter()
            .find(|s| s.event.sequence == seq)
            .and_then(|s| s.remote)
    }
    fn retention_gap(&self) -> Option<SignRetentionGap> {
        self.gap
    }
}
impl InvocationEvidence for Evidence {
    fn events(&self) -> impl Iterator<Item = KernelEvent> {
        self.signs.iter().map(|s| s.event)
    }
}
fn evidence(x: u32, y: u32, remote: u64, mixed: bool) -> Evidence {
    let count = if mixed { 4 } else { 3 };
    Evidence {
        signs: (0..count)
            .map(|i| ArchivedSign {
                event: KernelEvent {
                    sequence: i,
                    node: NodeId(if i == 1 { 1 } else { 0 }),
                    port: Some(PortId(0)),
                    request: Some(RequestId(if i == 1 { y } else { x + i })),
                    kind: if i == 3 {
                        KernelEventKind::BackCompleted
                    } else {
                        KernelEventKind::HostCallCompleted
                    },
                },
                remote: (i < 2).then(|| RemoteLifecycleIdentity {
                    endpoint: RemoteEndpointId(i as u16),
                    cord: CordId(i as u16),
                    direction: RemoteCordDirection::Ingress,
                    sequence: remote + u64::from(i),
                }),
            })
            .collect(),
        gap: Some(SignRetentionGap {
            first_sequence: 4,
            last_sequence: 4,
            entries: 1,
        }),
    }
}
#[test]
fn dictionary_reuses_nonadjacent_shapes_and_preserves_independent_node_offsets() {
    let mut archive = LivePrefixArchive::<16, 3, 48, 2, 2>::new();
    let mut expected = Vec::new();
    for i in 0..128u32 {
        let signs = evidence(i * 3, i / 8, u64::from(i), i % 8 == 0);
        archive.can_append(&signs, 5).unwrap();
        archive.append(&signs, 5).unwrap();
        expected.push(signs);
    }
    assert_eq!(archive.template_count(), 2);
    assert_eq!(archive.template_event_count(), 7);
    assert_eq!(archive.tape_run_count(), 32);
    for (i, signs) in expected.iter().enumerate() {
        let prefix = archive.prefix(i as u32).unwrap();
        assert_eq!(prefix.gap(), signs.gap);
        assert_eq!(prefix.span(), 5);
        assert_eq!(prefix.collect::<Vec<_>>(), signs.signs)
    }
    assert!(archive.prefix(128).is_none());
}
#[test]
fn each_explicit_stock_and_overflow_refuses_without_mutating_archive() {
    let a = evidence(1, 2, 0, false);
    let b = evidence(5, 3, 1, true);
    let mut nodes = LivePrefixArchive::<16, 3, 8, 1, 2>::new();
    assert_eq!(nodes.append(&a, 5), Err(DictionaryRefusal::NodeCapacity));
    assert!(nodes.is_empty());
    let mut streams = LivePrefixArchive::<16, 3, 8, 2, 1>::new();
    assert_eq!(
        streams.append(&a, 5),
        Err(DictionaryRefusal::StreamCapacity)
    );
    assert!(streams.is_empty());
    let mut tape = LivePrefixArchive::<16, 3, 1, 2, 2>::new();
    tape.append(&a, 5).unwrap();
    assert_eq!(tape.append(&b, 5), Err(DictionaryRefusal::Capacity));
    assert_eq!(tape.template_count(), 1);
    assert_eq!(tape.prefix(0).unwrap().collect::<Vec<_>>(), a.signs);
    let mut templates = LivePrefixArchive::<16, 1, 8, 2, 2>::new();
    templates.append(&a, 5).unwrap();
    assert_eq!(templates.append(&b, 5), Err(DictionaryRefusal::Capacity));
    assert_eq!(templates.len(), 1);
    let mut rows = LivePrefixArchive::<3, 3, 8, 2, 2>::new();
    assert_eq!(rows.append(&b, 5), Err(DictionaryRefusal::Capacity));
    assert!(rows.is_empty());
    let mut overflow = LivePrefixArchive::<16, 3, 8, 2, 2>::new();
    overflow.prefixes = u32::MAX;
    assert_eq!(overflow.append(&a, 5), Err(DictionaryRefusal::Overflow));
    assert_eq!(overflow.event_len, 0);
    overflow.prefixes = 0;
    overflow.span = u32::MAX - 1;
    assert_eq!(overflow.append(&a, 5), Err(DictionaryRefusal::Overflow));
    assert_eq!(overflow.event_len, 0);
}
#[test]
fn maximum_original_request_and_remote_values_do_not_wrap() {
    let mut archive = LivePrefixArchive::<16, 3, 8, 2, 2>::new();
    let a = evidence(u32::MAX - 3, u32::MAX, u64::MAX - 1, false);
    archive.append(&a, 5).unwrap();
    let b = evidence(0, 0, 0, false);
    archive.append(&b, 5).unwrap();
    assert_eq!(archive.template_count(), 1);
    assert_eq!(archive.tape_run_count(), 2);
    assert_eq!(archive.prefix(0).unwrap().collect::<Vec<_>>(), a.signs);
    assert_eq!(archive.prefix(1).unwrap().collect::<Vec<_>>(), b.signs);
}
#[cfg(feature = "alloc")]
#[test]
fn explicit_candidate_fits_original_backing_limits_and_transfers_same_owner() {
    use crate::{live_sign_archive::ArchiveLimits, HostedSignLog, SignSink};
    type Log = DictionaryLiveSignLog<128, 4, 48, 2, 2>;
    let active = HostedSignLog::new_with_remote_storage(
        61,
        61 * core::mem::size_of::<KernelEvent>() as u32,
        6,
        crate::remote_sign_storage_bytes(6).unwrap(),
    )
    .unwrap();
    let bytes = Log::required_archive_backing_bytes();
    let overlap = Log::required_transfer_overlap_bytes(&active).unwrap();
    std::println!("DICTIONARY_PROFILE_ROW {{\"event_stock\":128,\"template_stock\":4,\"tape_stock\":48,\"request_lanes\":2,\"remote_lanes\":2,\"archive_backing_bytes\":{bytes},\"overlap_bytes\":{overlap}}}");
    assert!(bytes <= 8192);
    assert!(overlap <= 12288);
    let mut log = Log::try_prepare_dictionary(
        active,
        ArchiveLimits {
            retained_events: 32768,
            retained_bytes: 2 * 1024 * 1024,
            archive_backing_bytes: 8192,
            transfer_overlap_bytes: 12288,
        },
    )
    .unwrap();
    let event = log
        .record(
            NodeId(0),
            None,
            Some(RequestId(1)),
            KernelEventKind::HostCallCompleted,
        )
        .unwrap();
    let before = log.events().collect::<Vec<_>>();
    drop(log.prepare_prefix_transfer().unwrap());
    assert_eq!(log.events().collect::<Vec<_>>(), before);
    log.prepare_prefix_transfer().unwrap().commit();
    assert_eq!(log.events().collect::<Vec<_>>(), [event]);
}
