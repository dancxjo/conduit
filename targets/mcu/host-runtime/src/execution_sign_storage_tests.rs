//! CPU backing geometry and exact original evidence; no Source/HIL activation.
use super::*;
use alloc::vec::Vec;
use conduit_kernel::{
    dictionary_live_sign_archive::{DictionaryLiveSignLog, LivePrefixArchive},
    invocation_archive::InvocationArchive,
    live_sign_archive::{ArchiveLimits, BoxedLiveSignLog, LiveArchiveBackend, LiveSignLog},
};
use core::mem::size_of;
type Inline = LiveSignLog<128, 16, 2>;
type Boxed = BoxedLiveSignLog<128, 16, 2>;
type Dictionary = DictionaryLiveSignLog<128, 4, 48, 2, 2>;
fn active() -> HostedSignLog {
    HostedSignLog::new_with_remote_storage(
        61,
        61 * size_of::<KernelEvent>() as u32,
        6,
        remote_sign_storage_bytes(6).unwrap(),
    )
    .unwrap()
}
fn limits<const E: usize, const R: usize, const S: usize, A: LiveArchiveBackend<E, R, S>>(
    active: &HostedSignLog,
) -> ArchiveLimits {
    ArchiveLimits {
        retained_events: 32768,
        retained_bytes: 2097152,
        archive_backing_bytes: LiveSignLog::<E, R, S, A>::required_archive_backing_bytes(),
        transfer_overlap_bytes: LiveSignLog::<E, R, S, A>::required_transfer_overlap_bytes(active)
            .unwrap(),
    }
}
struct Idle;
impl StepBack<1> for Idle {
    fn step(&mut self, _: &mut StepIo<1>, _: &StepInputBytes<'_, 1>) -> StepOutcome {
        StepOutcome::Await
    }
}
type Geometry<L> = FixedScheduler<Idle, FixedValueStore<1, 8>, L, 1, 0, 1, 0, 0, 0>;
fn check<const E: usize, const R: usize, const S: usize, A: LiveArchiveBackend<E, R, S>>(
    mut log: LiveSignLog<E, R, S, A>,
    active_bytes: usize,
    archive_heap: usize,
) {
    let external = ExecutionSigns::external_backing_bytes(&log);
    assert_eq!(external, active_bytes + archive_heap);
    assert_eq!(
        log.owned_backing_bytes(),
        Some(size_of::<LiveSignLog<E, R, S, A>>() + external)
    );
    let kernel = size_of::<Geometry<LiveSignLog<E, R, S, A>>>();
    let other_inline = kernel
        .checked_sub(size_of::<LiveSignLog<E, R, S, A>>())
        .unwrap();
    assert_eq!(
        kernel.checked_add(external),
        other_inline.checked_add(log.owned_backing_bytes().unwrap())
    );
    // In particular, the Box pointer/inline archive is already in kernel storage.
    assert_ne!(external, log.owned_backing_bytes().unwrap());
    let first = log
        .record(
            NodeId(2),
            None,
            Some(RequestId(7)),
            KernelEventKind::HostCallCompleted,
        )
        .unwrap();
    log.prepare_prefix_transfer().unwrap().commit();
    let remote = RemoteLifecycleIdentity {
        endpoint: RemoteEndpointId(3),
        cord: CordId(4),
        direction: RemoteCordDirection::Ingress,
        sequence: 9,
    };
    let second = log
        .record_remote(
            NodeId(2),
            PortId(0),
            KernelEventKind::RemoteInputAdmitted,
            remote,
        )
        .unwrap();
    assert_eq!(
        ExecutionSigns::events(&log).collect::<Vec<_>>(),
        [first, second]
    );
    log.prepare_prefix_transfer().unwrap().commit();
    let third = log
        .record(NodeId(1), None, None, KernelEventKind::BackCompleted)
        .unwrap();
    assert_eq!(
        ExecutionSigns::events(&log).collect::<Vec<_>>(),
        [first, second, third]
    );
    assert_eq!(log.remote_identity(second.sequence), Some(remote));
    assert_eq!(ExecutionSigns::external_backing_bytes(&log), external);
}
#[test]
fn inline_boxed_and_dictionary_charge_only_reachable_external_allocations() {
    let original = active();
    let bytes = original.reserved_storage_bytes();
    let limit = limits::<128, 16, 2, InvocationArchive<128, 16, 2>>(&original);
    check(Inline::prepare(original, limit).unwrap(), bytes, 0);
    let original = active();
    let limit = limits::<128, 16, 2, alloc::boxed::Box<InvocationArchive<128, 16, 2>>>(&original);
    check(
        Boxed::try_prepare(original, limit).unwrap(),
        bytes,
        size_of::<InvocationArchive<128, 16, 2>>(),
    );
    let original = active();
    let limit =
        limits::<128, 48, 2, alloc::boxed::Box<LivePrefixArchive<128, 4, 48, 2, 2>>>(&original);
    check(
        Dictionary::try_prepare_dictionary(original, limit).unwrap(),
        bytes,
        size_of::<LivePrefixArchive<128, 4, 48, 2, 2>>(),
    );
}
#[test]
fn overflow_and_underflow_refuse_nonzero_kernel_sizing_instead_of_undercharging() {
    for total in [None, Some(0), Some(7)] {
        let refused = external_sign_backing(total, 8);
        assert_eq!(refused, usize::MAX);
        assert!(size_of::<Geometry<Inline>>().checked_add(refused).is_none());
    }
    assert_eq!(external_sign_backing(Some(108), 8), 100);
}
#[test]
fn default_fixed_and_hosted_charges_are_unchanged() {
    let fixed = FixedSignLog::<4>::new(4 * size_of::<KernelEvent>() as u32).unwrap();
    assert_eq!(ExecutionSigns::external_backing_bytes(&fixed), 0);
    let hosted = active();
    assert_eq!(
        ExecutionSigns::external_backing_bytes(&hosted),
        hosted.reserved_storage_bytes()
    );
}
