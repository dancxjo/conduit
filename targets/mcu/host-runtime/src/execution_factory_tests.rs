use super::*;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome, RequestId, ValueRef};

fn value() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 1,
    }
}
fn completion(output: bool) -> Option<(RequestId, HostCallOutcome)> {
    Some((
        RequestId(0),
        HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: output.then(|| BoundedValueRef::new(value(), 1).unwrap()),
            failure: None,
        },
    ))
}
fn request(back: &mut Driver<'_, HostCallBack>) {
    let mut io = StepIo::test_frame([Some(value())], [false], [Some(1)], None, 4);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(io.test_host_request().unwrap().0, RequestId(0));
}

#[test]
fn effect_only_driver_accepts_empty_completion_and_preserves_committed_generations() {
    let input = StepInputBytes::test_frame([None], None);
    let mut back: Driver<'_, HostCallBack> = Driver::Write(WriteBack::new(2));
    request(&mut back);
    request(&mut back); // Discarded transactions do not advance request identity.
    <Driver<HostCallBack> as StepBack<1>>::step_committed(&mut back);
    for _ in 0..2 {
        let mut io = StepIo::test_frame([None], [false], [None], completion(false), 4);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        assert!(io.test_host_completion_consumed());
    }
    <Driver<HostCallBack> as StepBack<1>>::step_committed(&mut back);
    let mut io = StepIo::test_frame([Some(value())], [false], [None], None, 4);
    assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
    assert_eq!(io.test_host_request().unwrap().0, RequestId(1));
}

#[test]
fn completion_shape_remains_specific_to_checked_driver_kind() {
    for sink in [false, true] {
        let mut back: Driver<'_, HostCallBack> = if sink {
            Driver::Write(WriteBack::new(2))
        } else {
            Driver::Native(HostCallBack::new(1))
        };
        request(&mut back);
        <Driver<HostCallBack> as StepBack<1>>::step_committed(&mut back);
        let mut io = StepIo::test_frame([None], [false], [Some(1)], completion(sink), 4);
        assert!(matches!(
            back.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Fail(_)
        ));
        assert!(!io.test_host_completion_consumed());
    }
}

#[test]
fn composite_terminal_laws_survive_software_and_driver_wrappers() {
    let schema =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    let actual = conduit_composite::FlowMergeFiniteBack::prepare(&schema, 8).unwrap();
    let expected =
        <conduit_composite::FlowMergeFiniteBack as StepBack<2>>::terminal_transductions(&actual);
    assert!(expected.iter().all(Option::is_some));
    let software = SoftwareBack::merge(&schema, 8).unwrap();
    let wrapped: Driver<'_, HostCallBack> = Driver::Software(software);
    assert_eq!(
        <Driver<HostCallBack> as StepBack<2>>::terminal_transductions(&wrapped),
        expected
    );
}

#[test]
fn terminal_state_constructor_refuses_a_schema_without_terminal_boolean() {
    let schema =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    let contract = conduit_core::CheckedValueContract {
        value_kind: conduit_core::kind_id("value/u64"),
        maximum_bytes: 8,
        constraints: alloc::vec![],
    };
    assert!(SoftwareBack::seeded_until(&contract, &schema).is_err());
}
