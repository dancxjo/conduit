#![cfg(feature = "alloc")]
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_kernel::{
    invocation_archive::InvocationArchive,
    live_sign_archive::{ArchiveLimits, BoxedLiveSignLog, LiveSignLog, TransferRefusal},
    HostedSignLog, KernelEvent, KernelEventKind, NodeId, RequestId, SignQuery, SignSink,
};
use core::mem::size_of;
type Boxed = BoxedLiveSignLog<128, 16, 2>;
fn active() -> HostedSignLog {
    let mut log = HostedSignLog::new(16, 16 * size_of::<KernelEvent>() as u32).unwrap();
    log.record(
        NodeId(1),
        None,
        Some(RequestId(7)),
        KernelEventKind::HostCallCompleted,
    )
    .unwrap();
    log
}
fn limits(active: &HostedSignLog) -> ArchiveLimits {
    ArchiveLimits {
        retained_events: 1000,
        retained_bytes: 100000,
        archive_backing_bytes: Boxed::required_archive_backing_bytes(),
        transfer_overlap_bytes: Boxed::required_transfer_overlap_bytes(active).unwrap(),
    }
}
#[test]
fn one_exact_archive_allocation_no_transfer_allocations_and_one_archive_free() {
    let active = active();
    let active_owned = active.reserved_storage_bytes();
    let limits = limits(&active);
    let (result, observed) = allocation_probe::observe(|| Boxed::try_prepare(active, limits));
    let mut log = result.unwrap();
    assert_eq!((observed.allocations, observed.reallocations), (1, 0));
    assert_eq!(
        observed.requested_bytes,
        size_of::<InvocationArchive<128, 16, 2>>()
    );
    assert_eq!(
        log.owned_backing_bytes(),
        Some(size_of::<Boxed>() + active_owned + observed.requested_bytes)
    );
    let (_, transfer) = allocation_probe::observe(|| {
        log.prepare_prefix_transfer().unwrap().commit();
        log.record(
            NodeId(1),
            None,
            Some(RequestId(8)),
            KernelEventKind::HostCallCompleted,
        )
        .unwrap();
        log.prepare_prefix_transfer().unwrap().commit();
        assert_eq!(log.events().count(), 2);
        assert!(log.contains_kind(KernelEventKind::HostCallCompleted));
    });
    assert_eq!(
        (
            transfer.allocations,
            transfer.reallocations,
            transfer.deallocations
        ),
        (0, 0, 0)
    );
    let (_, dropped) = allocation_probe::observe(|| drop(log));
    // HostedSignLog has one active allocation here, plus exactly one archive.
    assert_eq!(dropped.deallocations, 2);
    eprintln!("128/16/2 layout: archive={},inline_wrapper={},boxed_wrapper={},boxed_archive_charge={},boxed_overlap={}",size_of::<InvocationArchive<128,16,2>>(),size_of::<LiveSignLog<128,16,2>>(),size_of::<Boxed>(),limits.archive_backing_bytes,limits.transfer_overlap_bytes);
}
#[test]
fn allocation_refusal_and_preflight_preserve_the_exact_active_sign_owner() {
    let active = active();
    let limits = limits(&active);
    let before: Vec<_> = active.events().collect();
    let original_bytes = active.reserved_storage_bytes();
    let (result, observed) =
        allocation_probe::observe_refusing(1, || Boxed::try_prepare(active, limits));
    let rejected = match result {
        Err(error) => error,
        Ok(_) => panic!("expected preparation refusal"),
    };
    assert_eq!(rejected.reason, TransferRefusal::Allocation);
    assert_eq!(
        (
            observed.allocations,
            observed.refused_allocations,
            observed.deallocations
        ),
        (0, 1, 0)
    );
    let active = rejected.into_active();
    assert_eq!(active.reserved_storage_bytes(), original_bytes);
    assert_eq!(active.events().collect::<Vec<_>>(), before);
    let mut insufficient = limits;
    insufficient.archive_backing_bytes -= 1;
    let (result, observed) = allocation_probe::observe(|| Boxed::try_prepare(active, insufficient));
    let rejected = match result {
        Err(error) => error,
        Ok(_) => panic!("expected preparation refusal"),
    };
    assert_eq!(rejected.reason, TransferRefusal::Bounds);
    assert_eq!(
        (observed.allocation_attempts, observed.deallocations),
        (0, 0)
    );
    let active = rejected.into_active();
    assert_eq!(active.events().collect::<Vec<_>>(), before);
    // The returned SAME sequence clock is still usable after both refusals.
    let mut log = Boxed::try_prepare(active, limits).unwrap();
    let event = log
        .record(NodeId(1), None, None, KernelEventKind::BackCompleted)
        .unwrap();
    assert_eq!(event.sequence, 1);
}
#[test]
fn zero_stock_archive_initializes_safely_and_preserves_capacity_refusal() {
    type Empty = BoxedLiveSignLog<0, 0, 0>;
    let active = active();
    let limits = ArchiveLimits {
        retained_events: 10,
        retained_bytes: 10000,
        archive_backing_bytes: Empty::required_archive_backing_bytes(),
        transfer_overlap_bytes: Empty::required_transfer_overlap_bytes(&active).unwrap(),
    };
    let mut log = Empty::try_prepare(active, limits).unwrap();
    assert_eq!(
        log.prepare_prefix_transfer().err(),
        Some(TransferRefusal::ArchiveCapacity)
    );
    assert_eq!(log.events().count(), 1);
}
