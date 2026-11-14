//! Compile and exercise the exact generated zero-inventory implementation.
#[allow(dead_code)]
mod generated {
    include!("resident_assignment_source.rs");
    include!("resident_library_empty_source.rs");
}

#[test]
fn empty_inventory_retains_no_computation_owners_and_lends_no_backs() {
    assert_eq!(generated::ENTRY_COUNT, 0);
    assert_eq!(generated::VALUE_SLOT_BYTES, 0);
    let mut owner = generated::Initialized::prepare().unwrap();
    assert!(owner.offers().is_empty());
    assert!(owner.available_identity(0).is_none());
    assert!(owner.available_value_slot_bytes(0).is_none());
    assert!(owner.borrow_selected_array([]).unwrap().is_empty());
    owner.borrow_selected([]).unwrap();
    assert!(owner.prepare_selected_array([]).unwrap().bind().is_empty());
    assert!(owner.identify_assignments(b"not a plan").is_err());
    assert!(owner
        .identify_mixed_assignments(b"not a plan", &[])
        .is_err());
    owner.into_backs();
}

#[test]
fn empty_inventory_generation_still_obeys_its_explicit_byte_bound() {
    let exact = concat!(
        include_str!("resident_assignment_source.rs"),
        include_str!("resident_library_empty_source.rs")
    );
    assert_eq!(
        crate::generate_resident_library(&[], exact.len() as u32).unwrap(),
        exact
    );
    assert!(matches!(
        crate::generate_resident_library(&[], exact.len() as u32 - 1),
        Err(crate::ResidentExpressionRefusal::Bounds)
    ));
}
