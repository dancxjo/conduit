//! Transaction pressure and failure are independent of caller-owned output staging.
use super::*;
use alloc::vec;
use conduit_core::{
    kind_id, StructuredFieldType as F, StructuredFieldValue as V, StructuredInfoType as T,
    StructuredInfoValue as I, StructuredSelector,
};
#[test]
fn workspace_selector_refuses_short_output_and_bad_unselected_leaf_without_consuming() {
    let byte = T::leaf(kind_id("value/u8")).unwrap();
    let boolean = T::leaf(kind_id("value/bool")).unwrap();
    let ty = T::record(
        kind_id("test/workspace-selector-pressure"),
        vec![
            F::new("chosen", byte.clone()).unwrap(),
            F::new("unused", boolean.clone()).unwrap(),
        ],
    )
    .unwrap();
    let value = I::record(
        ty.clone(),
        vec![
            V::new("chosen", I::leaf(byte, vec![7]).unwrap()).unwrap(),
            V::new("unused", I::leaf(boolean, vec![1]).unwrap()).unwrap(),
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let definition = StructuredSelector::field(ty, "chosen").unwrap();
    let material = &*Box::leak(definition.canonical_bytes().unwrap().into_boxed_slice());
    let mut back = WorkspaceSelectorBack::prepare_static(material, 512, 512).unwrap();
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: value.len() as u32,
    };
    let mut output = [0xa5; 512];
    let mut io = StepIo::test_frame([Some(reference)], [false], [Some(512)], None, 8);
    assert!(matches!(
        back.run(
            &mut io,
            &StepInputBytes::test_frame([Some(&value)], None),
            Some(&mut output[..1])
        ),
        StepOutcome::Fail(_)
    ));
    assert_eq!(output, [0xa5; 512]);
    assert!(!io.test_consumed(PortId(0)));
    assert!(!back.staged);
    let mut malformed = value.clone();
    *malformed.last_mut().unwrap() = 2;
    let mut io = StepIo::test_frame([Some(reference)], [false], [Some(512)], None, 8);
    assert!(matches!(
        back.run(
            &mut io,
            &StepInputBytes::test_frame([Some(&malformed)], None),
            Some(&mut output)
        ),
        StepOutcome::Fail(_)
    ));
    assert_eq!(output, [0xa5; 512]);
    assert!(!io.test_consumed(PortId(0)));
    for _ in 0..2 {
        let mut io = StepIo::test_frame([Some(reference)], [false], [Some(1)], None, 8);
        back.staged = true;
        assert_eq!(
            back.run(
                &mut io,
                &StepInputBytes::test_frame([Some(&value)], None),
                Some(&mut output)
            ),
            StepOutcome::Await
        );
        assert!(!io.test_consumed(PortId(0)));
        assert!(!back.staged);
    }
    back.staged = true;
    <WorkspaceSelectorBack as StepBack<1>>::step_committed(&mut back);
    assert!(!back.staged);
    back.staged = true;
    <WorkspaceSelectorBack as StepBack<1>>::cancel(&mut back);
    assert!(!back.staged);
    let mut io = StepIo::test_frame([None], [true], [None], None, 8);
    assert_eq!(
        back.step(&mut io, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    assert!(io.test_consumed_closed(PortId(0)));
    assert!(<WorkspaceSelectorBack as StepBack<1>>::prepared_output(&back, PortId(0)).is_none());
}
