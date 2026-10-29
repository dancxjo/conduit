#![cfg(feature = "kernel-step")]
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    FixedValueStore, HostCallDisposition, HostCallOutcome, RequestId, ValueStorage,
};
use conduit_time::TickBack;

#[test]
fn refused_and_pressured_retirement_preserves_counted_back_and_wait() {
    let mut store = FixedValueStore::<1, 8>::new(8).unwrap();
    let wait = store.store(&60_000u64.to_le_bytes()).unwrap();
    let back = TickBack::counted(wait, 1).unwrap();
    let mut back = match back.retire_retained_values() {
        Err(back) => back,
        Ok(_) => panic!("an unfinished finite Back must retain its state"),
    };
    let input = StepInputBytes::test_frame([None], None);
    let mut io = StepIo::test_frame([None], [false], [Some(8)], None, 8);
    assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
    let mut back = match back.retire_retained_values() {
        Err(back) => back,
        Ok(_) => panic!("pending wait must refuse retirement"),
    };
    let completion = Some((
        RequestId(0),
        HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: None,
            failure: None,
        },
    ));
    let mut pressured = StepIo::test_frame([None], [false], [None], completion, 8);
    assert_eq!(back.step(&mut pressured, &input), StepOutcome::Await);
    let mut back = match back.retire_retained_values() {
        Err(back) => back,
        Ok(_) => panic!("pressure must not consume the pending wait"),
    };
    let mut io = StepIo::test_frame([None], [false], [Some(8)], completion, 8);
    assert_eq!(back.step(&mut io, &input), StepOutcome::Complete);
    let retired = back
        .retire_retained_values()
        .unwrap_or_else(|_| panic!("complete"));
    let (values, waits) = retired.into_parts();
    assert!(values.is_empty());
    assert_eq!(waits, [wait]);
    assert_eq!(store.get(wait).unwrap(), &60_000u64.to_le_bytes());
    store.release(waits[0]).unwrap();
    assert_eq!(store.used_items(), 0);
}

#[test]
fn repeated_fresh_backs_return_the_same_store_without_orphaning_wait_slots() {
    let mut store = FixedValueStore::<1, 8>::new(8).unwrap();
    let mut oldest = None;
    for generation in 1..=2000 {
        let wait = store.store(&1u64.to_le_bytes()).unwrap();
        assert_eq!(wait.generation, generation);
        oldest.get_or_insert(wait);
        if generation > 1 {
            assert!(store.get(oldest.unwrap()).is_err());
        }
        let mut back = TickBack::counted(wait, 1).unwrap();
        let input = StepInputBytes::test_frame([None], None);
        let mut io = StepIo::test_frame([None], [false], [Some(8)], None, 8);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        let mut io = StepIo::test_frame(
            [None],
            [false],
            [Some(8)],
            Some((
                RequestId(0),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )),
            8,
        );
        assert_eq!(back.step(&mut io, &input), StepOutcome::Complete);
        let (_, waits) = back
            .retire_retained_values()
            .unwrap_or_else(|_| panic!("complete"))
            .into_parts();
        store.release(waits[0]).unwrap();
        assert_eq!(store.used_items(), 0);
        assert_eq!(store.used_bytes(), 0);
    }
}

#[test]
fn cancelled_finite_back_returns_original_vectors_and_recurring_requires_cancel() {
    let mut store = FixedValueStore::<4, 8>::new(32).unwrap();
    let values = vec![store.store(&[1; 8]).unwrap(), store.store(&[2; 8]).unwrap()];
    let waits = vec![store.store(&[3; 8]).unwrap(), store.store(&[4; 8]).unwrap()];
    let (value_pointer, wait_pointer) = (values.as_ptr(), waits.as_ptr());
    let mut back = TickBack::finite(values, waits).unwrap();
    <TickBack as StepBack<1>>::cancel(&mut back);
    let (values, waits) = back
        .retire_retained_values()
        .unwrap_or_else(|_| panic!("cancelled"))
        .into_parts();
    assert_eq!(values.as_ptr(), value_pointer);
    assert_eq!(waits.as_ptr(), wait_pointer);
    for value in values.into_iter().chain(waits) {
        store.release(value).unwrap();
    }
    let wait = store.store(&[5; 8]).unwrap();
    let back = TickBack::recurring(wait);
    let mut back = match back.retire_retained_values() {
        Err(back) => back,
        Ok(_) => panic!("a recurring Back has no finite completion"),
    };
    <TickBack as StepBack<1>>::cancel(&mut back);
    let (_, waits) = back
        .retire_retained_values()
        .unwrap_or_else(|_| panic!("cancelled"))
        .into_parts();
    store.release(waits[0]).unwrap();
    assert_eq!(store.used_items(), 0);
}
