//! Owned queued bytes delivered through actual native member execution.
use super::*;
use crate::{NativePoolCustodyRefusal, NativePoolRequestCustody};
use crate::{NativePoolQueueError, NativePoolRequestQueue};
use conduit_plan_lowering::shared_pool_runtime::{PoolWaitingClass, QueuedPoolOperation};

fn waiting_fixture() -> NativeFixture {
    let mut fixture = native_fixture();
    for fragment in &mut fixture.plan.fragments {
        for pool in &mut fragment.shared_pools {
            pool.selection_policy =
                SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity;
        }
    }
    fixture.plan = seal_plan(
        PlotIdentity {
            source_document_id: fixture.plan.source_document_id.clone(),
            checked_plot_id: fixture.plan.checked_plot_id.clone(),
            expanded_plot_id: fixture.plan.expanded_plot_id.clone(),
        },
        fixture.plan.fragments,
    );
    fixture.selection.plan_id = fixture.plan.plan_id.clone();
    fixture
}

fn metadata(fixture: &NativeFixture, bytes: usize) -> QueuedPoolOperation {
    let fragment = &fixture.plan.fragments[0];
    let clock = MonotonicClockIdentity::new(
        fragment.host_id.clone(),
        fragment.boot_id.clone(),
        "fixture/custody-clock".into(),
        TemporalScale::Microseconds,
        1,
        0,
    )
    .unwrap();
    QueuedPoolOperation {
        plan: fixture.plan.plan_id.clone(),
        pool: fixture.selection.pool_id.clone(),
        operation: fixture.selection.operation_id.clone(),
        requester: "fixture/requester".into(),
        class: PoolWaitingClass::Interactive,
        key: conduit_kernel::shared_pool::MemberKey([7; 32]),
        demand: [8; 32],
        admitted_bytes: bytes.try_into().unwrap(),
        waiting_expires: MonotonicInstant::new(1000, clock).unwrap(),
    }
}

#[test]
fn common_fifo_cancellation_and_expiry_return_the_original_owned_payloads() {
    use conduit_plan_lowering::shared_pool_runtime::{
        HostedSharedPool, PoolWaitingLimits, PoolWaitingPolicy,
    };
    let fixture = waiting_fixture();
    with_demand(&fixture.gear, b"first", |first| {
        with_demand(&fixture.gear, b"second", |second| {
            let a = metadata(&fixture, first.input_bytes());
            let mut b = metadata(&fixture, second.input_bytes());
            b.operation = "fixture/second".into();
            b.key = conduit_kernel::shared_pool::MemberKey([9; 32]);
            b.demand = [10; 32];
            let now = MonotonicInstant::new(0, a.waiting_expires.clock().clone()).unwrap();
            let pool = HostedSharedPool::<2, 64, 8>::new(
                &fixture.plan,
                0,
                fixture.selection.pool_id.clone(),
                conduit_kernel::NodeId(10),
                1,
            )
            .unwrap();
            let mut queue = NativePoolRequestQueue::prepare(
                pool,
                PoolWaitingLimits {
                    operations: 2,
                    per_requester: 2,
                    admitted_bytes: 11,
                },
                PoolWaitingPolicy::Fifo,
                now.clone(),
            )
            .unwrap();
            queue
                .enqueue(
                    NativePoolRequestCustody::prepare(&fixture.plan, a.clone(), first).unwrap(),
                    &now,
                )
                .unwrap();
            queue
                .enqueue(
                    NativePoolRequestCustody::prepare(&fixture.plan, b.clone(), second).unwrap(),
                    &now,
                )
                .unwrap();
            assert_eq!(queue.waiting_population(), 2);
            assert_eq!(
                queue.next_request(&now).unwrap().unwrap().payload().encoded,
                b"first"
            );
            assert_eq!(
                queue.cancel(&a.operation).unwrap().payload().encoded,
                b"first"
            );
            assert_eq!(
                queue.next_request(&now).unwrap().unwrap().payload().encoded,
                b"second"
            );
            let expired = queue.retire_expired(&b.waiting_expires).unwrap().unwrap();
            assert_eq!(expired.payload().encoded, b"second");
            assert_eq!(queue.waiting_population(), 0);
            assert!(queue.retire_expired(&b.waiting_expires).unwrap().is_none());
            assert_eq!(fixture.state.lock().unwrap().0, 0);
        });
    });
}

#[test]
fn common_byte_admission_refusal_leaves_existing_payload_custody_unchanged() {
    use conduit_plan_lowering::shared_pool_runtime::{
        HostedSharedPool, PoolWaitingError, PoolWaitingLimits, PoolWaitingPolicy,
    };
    let fixture = waiting_fixture();
    with_demand(&fixture.gear, b"first", |first| {
        with_demand(&fixture.gear, b"second", |second| {
            let a = metadata(&fixture, first.input_bytes());
            let mut b = metadata(&fixture, second.input_bytes());
            b.operation = "fixture/second".into();
            b.key = conduit_kernel::shared_pool::MemberKey([9; 32]);
            let now = MonotonicInstant::new(0, a.waiting_expires.clock().clone()).unwrap();
            let pool = HostedSharedPool::<2, 64, 8>::new(
                &fixture.plan,
                0,
                fixture.selection.pool_id.clone(),
                conduit_kernel::NodeId(10),
                1,
            )
            .unwrap();
            let mut queue = NativePoolRequestQueue::prepare(
                pool,
                PoolWaitingLimits {
                    operations: 2,
                    per_requester: 2,
                    admitted_bytes: 5,
                },
                PoolWaitingPolicy::Fifo,
                now.clone(),
            )
            .unwrap();
            queue
                .enqueue(
                    NativePoolRequestCustody::prepare(&fixture.plan, a.clone(), first).unwrap(),
                    &now,
                )
                .unwrap();
            assert!(matches!(
                queue.enqueue(
                    NativePoolRequestCustody::prepare(&fixture.plan, b, second).unwrap(),
                    &now
                ),
                Err(NativePoolQueueError::Waiting(PoolWaitingError::ByteLimit))
            ));
            assert_eq!(queue.waiting_population(), 1);
            assert_eq!(
                queue.next_request(&now).unwrap().unwrap().payload().encoded,
                b"first"
            );
            assert_eq!(
                queue.cancel(&a.operation).unwrap().payload().encoded,
                b"first"
            );
            assert_eq!(queue.waiting_population(), 0);
            assert_eq!(fixture.state.lock().unwrap().0, 0);
        });
    });
}

#[test]
fn custody_owns_payload_storage_and_diagnostics_do_not_disclose_it() {
    let fixture = native_fixture();
    with_demand(&fixture.gear, b"retained-secret", |demand| {
        let custody = NativePoolRequestCustody::prepare(
            &fixture.plan,
            metadata(&fixture, demand.input_bytes()),
            demand,
        )
        .unwrap();
        assert_eq!(custody.payload().encoded, b"retained-secret");
        assert_ne!(
            custody.payload().encoded.as_ptr(),
            demand.inputs()[0].value.encoded.as_ptr()
        );
        assert!(!format!("{custody:?}").contains("retained-secret"));
        assert_eq!(fixture.state.lock().unwrap().0, 0);
    });
}

#[test]
fn underclaimed_extent_and_foreign_queue_scope_refuse_before_native_effects() {
    let fixture = native_fixture();
    with_demand(&fixture.gear, b"request", |demand| {
        let mut operation = metadata(&fixture, demand.input_bytes());
        operation.admitted_bytes -= 1;
        assert_eq!(
            NativePoolRequestCustody::prepare(&fixture.plan, operation, demand).unwrap_err(),
            NativePoolCustodyRefusal::ExtentChanged
        );
        let mut operation = metadata(&fixture, demand.input_bytes());
        operation.plan = "foreign/plan".into();
        assert_eq!(
            NativePoolRequestCustody::prepare(&fixture.plan, operation, demand).unwrap_err(),
            NativePoolCustodyRefusal::Correlation
        );
        let mut operation = metadata(&fixture, demand.input_bytes());
        operation.pool = "foreign/pool".into();
        assert_eq!(
            NativePoolRequestCustody::prepare(&fixture.plan, operation, demand).unwrap_err(),
            NativePoolCustodyRefusal::Correlation
        );
        assert_eq!(fixture.state.lock().unwrap().0, 0);
    });
}

#[test]
fn retained_payload_executes_once_through_the_existing_native_binding() {
    let mut fixture = native_fixture();
    let gear = fixture.gear.clone();
    with_demand(&gear, b"request", |demand| {
        let custody = NativePoolRequestCustody::prepare(
            &fixture.plan,
            metadata(&fixture, demand.input_bytes()),
            demand,
        )
        .unwrap();
        let mut member = fixture
            .host
            .prepare_local_model_pool_member_with_demand(
                &fixture.plan,
                fixture.selection.clone(),
                &fixture.consumer,
                custody.demand(),
            )
            .unwrap();
        let mut output = Vec::with_capacity(4096);
        let allocations = crate::allocation_probe::begin();
        let terminal = fixture.host.execute_local_model_pool_member(
            &mut member,
            &custody.payload().encoded,
            &mut output,
        );
        assert_eq!(allocations.finish(), 0);
        assert_eq!(terminal, LocalModelAdapterTerminal::Produced);
        assert_eq!(fixture.state.lock().unwrap().0, 1);
        assert_eq!(
            fixture.host.execute_local_model_pool_member(
                &mut member,
                &custody.payload().encoded,
                &mut output,
            ),
            LocalModelAdapterTerminal::Refused
        );
        assert_eq!(fixture.state.lock().unwrap().0, 1);
        fixture
            .host
            .release_local_model_pool_member(member)
            .unwrap();
    });
}

#[path = "request_custody_dispatch.rs"]
mod dispatch;
