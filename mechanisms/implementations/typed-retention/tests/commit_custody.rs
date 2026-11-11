//! Component proof through the production StepBack interface, not installed Play.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};
use conduit_typed_retention::{CommitOrder, PreparedTypedRetention as Retention};

fn boolean(value: bool) -> (Vec<u8>, Vec<u8>) {
    let ty = c::StructuredInfoType::leaf(c::kind_id(c::BOOL_INFO_ID)).unwrap();
    let payload = if value {
        c::InfoBool::TRUE
    } else {
        c::InfoBool::FALSE
    };
    let value = c::StructuredInfoValue::leaf(ty.clone(), payload.encode().to_vec()).unwrap();
    (
        ty.canonical_bytes().unwrap(),
        value.canonical_bytes().unwrap(),
    )
}
fn step(back: &mut Retention<'_>, bytes: &[u8], declared: u32, fuel: u16) -> (StepOutcome, bool) {
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: declared,
    };
    let mut io = StepIo::test_frame([Some(reference)], [false], [None], None, fuel);
    let outcome = back.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None));
    (outcome, io.test_consumed(PortId(0)))
}
fn stage(back: &mut Retention<'_>, bytes: &[u8]) -> StepOutcome {
    let (outcome, consumed) = step(back, bytes, bytes.len() as u32, 8);
    assert_eq!(consumed, outcome == StepOutcome::Progress);
    outcome
}
fn commit(back: &mut Retention<'_>) {
    <Retention<'_> as StepBack<1>>::step_committed(back);
}
fn cancel(back: &mut Retention<'_>) {
    <Retention<'_> as StepBack<1>>::cancel(back);
}
fn refusal(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

#[test]
fn borrowed_schema_prepares_only_two_buffers_then_commits_without_play_allocation() {
    let (schema, yes) = boolean(true);
    let (_, no) = boolean(false);
    let order = CommitOrder::prepare(2);
    let (mut back, preparation) =
        allocation_probe::observe(|| Retention::prepare(&schema, 128, 2, order.clone()).unwrap());
    assert_eq!(preparation.allocations, 2);
    assert_eq!(preparation.reallocations, 0);
    assert_eq!(back.value_storage_bytes(), Some(256));
    let ((), memory) = allocation_probe::observe(|| {
        assert!(back.snapshot().is_none());
        assert_eq!(stage(&mut back, &yes), StepOutcome::Progress);
        assert!(back.snapshot().is_none());
        assert_eq!(order.committed(), 0);
        commit(&mut back);
        assert_eq!(back.snapshot().unwrap().canonical, yes);
        assert_eq!(stage(&mut back, &no), StepOutcome::Progress);
        assert_eq!(back.snapshot().unwrap().canonical, yes);
        commit(&mut back);
        assert_eq!(back.snapshot().unwrap().canonical, no);
        assert_eq!(back.snapshot().unwrap().sequence, 2);
        commit(&mut back);
        assert_eq!(order.committed(), 2);
        assert_eq!(
            stage(&mut back, &yes),
            refusal(FailureCode::InvalidInput, 2)
        );
        assert_eq!(back.snapshot().unwrap().canonical, no);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn shared_reservation_pressure_and_repeated_cancel_do_not_spend_commit_identity() {
    let (schema, bytes) = boolean(true);
    let order = CommitOrder::prepare(1);
    let mut a = Retention::prepare(&schema, 128, 2, order.clone()).unwrap();
    let mut b = Retention::prepare(&schema, 128, 2, order.clone()).unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(stage(&mut a, &bytes), StepOutcome::Progress);
        assert_eq!(stage(&mut b, &bytes), refusal(FailureCode::InvalidInput, 5));
        assert_eq!(order.committed(), 0);
        cancel(&mut a);
        cancel(&mut a);
        commit(&mut a);
        assert!(a.snapshot().is_none());
        assert_eq!(stage(&mut b, &bytes), StepOutcome::Progress);
        commit(&mut b);
        assert_eq!(b.snapshot().unwrap().sequence, 1);
        assert_eq!(order.committed(), 1);
        assert_eq!(stage(&mut a, &bytes), refusal(FailureCode::Cancelled, 1));
        assert_eq!(stage(&mut b, &bytes), refusal(FailureCode::InvalidInput, 5));
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn drop_and_uncommitted_retry_return_the_original_reservation() {
    let (schema, bytes) = boolean(true);
    let order = CommitOrder::prepare(1);
    let mut a = Retention::prepare(&schema, 128, 1, order.clone()).unwrap();
    let mut b = Retention::prepare(&schema, 128, 1, order.clone()).unwrap();
    let mut c = Retention::prepare(&schema, 128, 1, order.clone()).unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        assert_eq!(stage(&mut a, &bytes), StepOutcome::Progress);
        // A new step without a committed predecessor discards its candidate.
        let mut io = StepIo::test_frame([None], [false], [None], None, 8);
        assert_eq!(
            a.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Await
        );
        commit(&mut a);
        assert!(a.snapshot().is_none());
        assert_eq!(stage(&mut b, &bytes), StepOutcome::Progress);
        drop(b);
        assert_eq!(stage(&mut c, &bytes), StepOutcome::Progress);
        commit(&mut c);
        assert_eq!(order.committed(), 1);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn malformed_primitive_and_wrong_type_extent_or_fuel_preserve_last_value() {
    let (schema, bytes) = boolean(true);
    let mut back = Retention::prepare(&schema, 128, 8, CommitOrder::prepare(8)).unwrap();
    assert_eq!(stage(&mut back, &bytes), StepOutcome::Progress);
    commit(&mut back);
    let mut malformed = bytes.clone();
    *malformed.last_mut().unwrap() = 2;
    let count_type = c::StructuredInfoType::leaf(c::kind_id(c::COUNT_INFO_ID)).unwrap();
    let wrong = c::StructuredInfoValue::leaf(count_type, 42u64.to_le_bytes().to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let mut trailing = bytes.clone();
    trailing.push(0);
    let oversized = vec![0; 129];
    let ((), memory) = allocation_probe::observe(|| {
        for input in [
            &malformed[..],
            &wrong,
            &trailing,
            &bytes[..bytes.len() - 1],
            &oversized,
        ] {
            assert_eq!(
                stage(&mut back, input),
                refusal(FailureCode::InvalidInput, 4)
            );
            commit(&mut back);
            assert_eq!(back.snapshot().unwrap().canonical, bytes);
            assert_eq!(back.snapshot().unwrap().sequence, 1);
        }
        assert_eq!(
            step(&mut back, &bytes, 0, 8),
            (refusal(FailureCode::InvalidInput, 4), false)
        );
        assert_eq!(
            step(&mut back, &bytes, bytes.len() as u32, 0),
            (refusal(FailureCode::InvalidLifecycle, 2), false)
        );
        let mut io = StepIo::test_frame([None], [true], [None], None, 8);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(0)));
        assert_eq!(back.snapshot().unwrap().canonical, bytes);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn unrelated_types_share_order_without_a_domain_channel_enum() {
    let (schema, bytes) = boolean(false);
    let count = c::StructuredInfoType::leaf(c::kind_id(c::COUNT_INFO_ID)).unwrap();
    let count_schema = count.canonical_bytes().unwrap();
    let count_bytes = c::StructuredInfoValue::leaf(count, 17u64.to_le_bytes().to_vec())
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let order = CommitOrder::prepare(2);
    let mut flag = Retention::prepare(&schema, 128, 1, order.clone()).unwrap();
    let mut total = Retention::prepare(&count_schema, 128, 1, order.clone()).unwrap();
    assert_eq!(stage(&mut flag, &bytes), StepOutcome::Progress);
    assert_eq!(stage(&mut total, &count_bytes), StepOutcome::Progress);
    // Shared order is commit order, not staging order.
    commit(&mut total);
    commit(&mut flag);
    assert_eq!(total.snapshot().unwrap().sequence, 1);
    assert_eq!(flag.snapshot().unwrap().sequence, 2);
    assert_eq!(total.snapshot().unwrap().canonical, count_bytes);
}

#[test]
fn invalid_preparation_refuses_before_allocating_buffers_and_zero_budget_stays_zero() {
    let (schema, bytes) = boolean(true);
    let mut torn_schema = schema.clone();
    torn_schema.push(0);
    let order = CommitOrder::prepare(0);
    let ((), memory) = allocation_probe::observe(|| {
        for (schema, maximum) in [
            (&schema[..], 0),
            (&schema[..], schema.len()),
            (&torn_schema[..], 128),
        ] {
            assert!(Retention::prepare(schema, maximum, 1, order.clone()).is_err());
        }
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let mut back = Retention::prepare(&schema, 128, 1, order.clone()).unwrap();
    assert_eq!(
        stage(&mut back, &bytes),
        refusal(FailureCode::InvalidInput, 5)
    );
    commit(&mut back);
    assert!(back.snapshot().is_none());
    assert_eq!(order.committed(), 0);
}
