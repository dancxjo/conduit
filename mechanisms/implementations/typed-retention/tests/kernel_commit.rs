//! The real fixed scheduler drives this CPU fixture; no Body/Line admission is claimed.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_kernel::ValueStorage;
use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerError, SchedulerStatus,
    },
    CordId, Failure, FailureCode, FixedRoutes, FixedSignLog, FixedValueStore, KernelEvent, NodeId,
    PortId, RemoteEndpointId,
};
use conduit_typed_retention::{CommitOrder, PreparedTypedRetention};
type Play<'a> = FixedScheduler<
    PreparedTypedRetention<'a>,
    FixedValueStore<2, 128>,
    FixedSignLog<128>,
    1,
    1,
    1,
    1,
    1,
    1,
>;
fn fixture(schema: &[u8], order: CommitOrder) -> Play<'_> {
    let back = PreparedTypedRetention::prepare(schema, 128, 2, order).unwrap();
    let mut routes = FixedRoutes::new(1);
    routes.seal().unwrap();
    let signs = FixedSignLog::new_with_remote_storage(
        (128 * core::mem::size_of::<KernelEvent>()) as u32,
        128,
        conduit_kernel::remote_sign_storage_bytes(128).unwrap(),
    )
    .unwrap();
    Play::new(
        [NodeSpec {
            input_cords: [Some(CordId(0))],
            maximum_step_fuel: 4,
        }],
        [CordSpec::remote_ingress(
            CordId(0),
            RemoteEndpointId(0),
            (NodeId(0), PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: 128,
                pressure_policy: Default::default(),
            },
        )],
        routes,
        [back],
        FixedValueStore::new(256).unwrap(),
        signs,
    )
    .unwrap()
}
fn boolean(value: bool) -> (Vec<u8>, Vec<u8>) {
    let ty = c::StructuredInfoType::leaf(c::kind_id(c::BOOL_INFO_ID)).unwrap();
    let payload = if value {
        c::InfoBool::TRUE
    } else {
        c::InfoBool::FALSE
    };
    let bytes = c::StructuredInfoValue::leaf(ty.clone(), payload.encode().to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    (ty.canonical_bytes().unwrap(), bytes)
}
#[test]
fn scheduler_consumes_and_commits_each_value_without_allocating_then_drains() {
    let (schema, yes) = boolean(true);
    let (_, no) = boolean(false);
    let order = CommitOrder::prepare(2);
    let mut play = fixture(&schema, order.clone());
    let ((), memory) = allocation_probe::observe(|| {
        let mut idle = false;
        for _ in 0..4 {
            if play.step().unwrap() == SchedulerStatus::Idle {
                idle = true;
                break;
            }
        }
        assert!(idle);
        assert!(play.drivers()[0].snapshot().is_none());
        assert_eq!(order.committed(), 0);
        for (sequence, bytes) in [&yes, &no].into_iter().enumerate() {
            play.admit_remote_input(RemoteEndpointId(0), CordId(0), sequence as u64, bytes)
                .unwrap();
            assert_eq!(order.committed(), sequence as u64);
            if sequence == 0 {
                assert!(play.drivers()[0].snapshot().is_none());
            } else {
                assert_eq!(play.drivers()[0].snapshot().unwrap().canonical, yes);
            }
            assert!(matches!(play.step(), Ok(SchedulerStatus::Progress { .. })));
            let snapshot = play.drivers()[0].snapshot().unwrap();
            assert_eq!(snapshot.canonical, bytes);
            assert_eq!(snapshot.sequence, sequence as u64 + 1);
            assert_eq!(order.committed(), sequence as u64 + 1);
            assert_eq!(play.values().used_items(), 0);
        }
        play.close_remote_input(RemoteEndpointId(0), CordId(0))
            .unwrap();
        let mut drained = false;
        for _ in 0..4 {
            if play.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!(play.drivers()[0].snapshot().unwrap().canonical, no);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn kernel_refusal_preserves_the_last_commit_and_does_not_number_a_bad_value() {
    let (schema, yes) = boolean(true);
    let mut bad = yes.clone();
    *bad.last_mut().unwrap() = 2;
    let order = CommitOrder::prepare(2);
    let mut play = fixture(&schema, order.clone());
    play.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &yes)
        .unwrap();
    play.step().unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        play.admit_remote_input(RemoteEndpointId(0), CordId(0), 1, &bad)
            .unwrap();
        assert_eq!(
            play.step(),
            Err(SchedulerError::BackFailed(Failure {
                code: FailureCode::InvalidInput,
                detail: 4,
            }))
        );
        assert_eq!(play.drivers()[0].snapshot().unwrap().canonical, yes);
        assert_eq!(order.committed(), 1);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn kernel_cancellation_cannot_publish_a_queued_value() {
    let (schema, yes) = boolean(true);
    let (_, no) = boolean(false);
    let order = CommitOrder::prepare(2);
    let mut play = fixture(&schema, order.clone());
    play.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &yes)
        .unwrap();
    play.step().unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        play.admit_remote_input(RemoteEndpointId(0), CordId(0), 1, &no)
            .unwrap();
        play.cancel().unwrap();
        assert_eq!(play.drivers()[0].snapshot().unwrap().canonical, yes);
        assert_eq!(order.committed(), 1);
        assert_eq!(play.values().used_items(), 0);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
