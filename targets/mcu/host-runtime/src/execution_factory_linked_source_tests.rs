//! Actual software-inventory construction and dispatch; not admission authority.
use super::tests::{empty, identity, now, zero};
use super::*;
use crate::member_return_tests::return_allocation_probe as allocation;
use conduit_core::{
    kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoType as T,
    StructuredInfoValue as V,
};
use conduit_environment_runtime::{
    publication::PublicationSequence, publication_face_custody::Publisher,
};
use core::cell::RefCell;

fn material() -> (&'static [u8], &'static [u8]) {
    let leaf = T::leaf(kind_id("value/u8")).unwrap();
    let ty = T::record(
        kind_id("fixture/base-settings@1"),
        alloc::vec![StructuredFieldType::new("address", leaf.clone()).unwrap()],
    )
    .unwrap();
    let value = V::record(
        ty.clone(),
        alloc::vec![
            StructuredFieldValue::new("address", V::leaf(leaf, alloc::vec![118]).unwrap()).unwrap()
        ],
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (
        alloc::boxed::Box::leak(ty.canonical_bytes().unwrap().into_boxed_slice()),
        alloc::boxed::Box::leak(value.into_boxed_slice()),
    )
}

#[test]
fn installed_fact_uses_original_commit_and_schema_dispatch_without_play_allocation() {
    let (schema, value) = material();
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let mut inventory = SoftwareInventory::<1>::empty();
    // SAFETY: explicit CPU constructor/identity fixture, no authority asserted.
    unsafe {
        inventory.initialize_by(
            |_| {
                Ok(Some(SoftwareDefinition::LinkedSourceStatic {
                    schema,
                    value,
                    maximum_bytes: value.len(),
                }))
            },
            |_| Ok(Some(identity(0))),
            &publisher,
            PublicationSequence::prepare(1),
            now,
            zero,
            &[],
            None,
        )
    }
    .unwrap();
    let back = inventory.slots[0].as_mut().unwrap();
    assert_eq!(
        <SoftwareBack as StepBack<1>>::canonical_input_schema(back, PortId(0)),
        None
    );
    assert_eq!(
        <SoftwareBack as StepBack<1>>::canonical_output_schema(back, PortId(0)),
        Some(schema)
    );
    assert_eq!(
        <SoftwareBack as StepBack<1>>::canonical_output_schema(back, PortId(1)),
        None
    );
    let input = StepInputBytes::test_frame([None], None);
    let frame = |ready: bool| {
        StepIo::test_frame(
            [None],
            [false],
            [ready.then_some(value.len() as u32)],
            None,
            2,
        )
    };
    let ((), cost) = allocation::observe(|| {
        assert_eq!(back.step(&mut frame(false), &input), StepOutcome::Await);
        assert_eq!(back.step(&mut frame(true), &input), StepOutcome::Progress);
        assert_eq!(
            <SoftwareBack as StepBack<1>>::prepared_output(back, PortId(0))
                .unwrap()
                .as_ptr(),
            value.as_ptr()
        );
        // A retry before commit still borrows the original flash material.
        assert_eq!(back.step(&mut frame(true), &input), StepOutcome::Progress);
        <SoftwareBack as StepBack<1>>::step_committed(back);
        assert_eq!(back.step(&mut frame(true), &input), StepOutcome::Complete);
        assert_eq!(
            <SoftwareBack as StepBack<1>>::prepared_output(back, PortId(0)),
            None
        );
    });
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
}

#[test]
fn late_invalid_installed_fact_clears_partial_inventory() {
    let (schema, value) = material();
    let publisher = RefCell::new(None::<Publisher<'_>>);
    let mut inventory = SoftwareInventory::<2>::empty();
    let result = unsafe {
        inventory.initialize_by(
            |i| {
                Ok(Some(if i == 0 {
                    SoftwareDefinition::Trigger
                } else {
                    SoftwareDefinition::LinkedSourceStatic {
                        schema,
                        value,
                        maximum_bytes: value.len() - 1,
                    }
                }))
            },
            |i| Ok(Some(identity(i as u16))),
            &publisher,
            PublicationSequence::prepare(1),
            now,
            zero,
            &[],
            None,
        )
    };
    assert_eq!(result, Err("linked-source-material"));
    empty(&inventory);
}
