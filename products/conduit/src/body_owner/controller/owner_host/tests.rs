use super::*;

fn checked_timer_host() -> StdHost {
    use conduit_host_make::{
        build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
        BuildInputs,
    };
    let source = r#"host return-frame-test {
      schema = 1
      target = {architecture: "x86_64", machine: "computer", os: "linux"}
      base = {kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}
      capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
      need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}
      limits = {static_memory_bytes: 16777216, heap_arena_bytes: 67108864, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}
    }"#;
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/return-frame".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    StdHost::from_selected_timer_image(
        conduit_std_host::StdHostConfig {
            host_id: "std/durable".into(),
            boot_id: "boot/return-frame".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap()
}

#[test]
fn boxed_birth_refusal_preserves_allocation_for_equal_labels_other_counter() {
    let host = Box::new(checked_timer_host());
    let other = checked_timer_host();
    assert_eq!(host.advertisement(), other.advertisement());
    let address = core::ptr::from_ref(host.as_ref());
    let advertisement = host.advertisement().clone();
    let foreign = OwnerHost::prepare(&other).unwrap();
    let (returned, error) = match OwnerHost::from_prepared_boxed(host, foreign) {
        Ok(_) => panic!("copied identity labels transferred original counter custody"),
        Err(refusal) => refusal,
    };
    assert!(!error.is_empty());
    assert_eq!(core::ptr::from_ref(returned.as_ref()), address);
    assert_eq!(returned.advertisement(), &advertisement);
    // Refusal leaves the actual Host usable, not replaced or retired.
    let own = OwnerHost::prepare(&returned).unwrap().unwrap();
    own.verify_host(&returned).unwrap();
}

#[test]
fn boxed_birth_refusal_preserves_allocation_for_retired_original_owner() {
    let host = Box::new(checked_timer_host());
    let address = core::ptr::from_ref(host.as_ref());
    let advertisement = host.advertisement().clone();
    let retired = OwnerHost::prepare(&host).unwrap().unwrap();
    retired.invalidate();
    let (returned, error) = match OwnerHost::from_prepared_boxed(host, Some(retired)) {
        Ok(_) => panic!("retired Root preparation owner was accepted"),
        Err(refusal) => refusal,
    };
    assert!(!error.is_empty());
    assert_eq!(core::ptr::from_ref(returned.as_ref()), address);
    assert_eq!(returned.advertisement(), &advertisement);
    let fresh = OwnerHost::prepare(&returned).unwrap().unwrap();
    fresh.verify_host(&returned).unwrap();
}

#[test]
fn boxed_birth_keeps_original_counter_through_host_loan_and_restore() {
    let host = Box::new(checked_timer_host());
    let advertisement = host.advertisement().clone();
    let preparation = OwnerHost::prepare(&host).unwrap();
    let witness = preparation.as_ref().unwrap().clone();
    let mut owner = match OwnerHost::from_prepared_boxed(host, preparation) {
        Ok(owner) => owner,
        Err((_, error)) => panic!("lawful original Host refused: {error}"),
    };
    assert!(owner.has_image_custody());
    assert!(owner.optional_root_preparation().is_some());
    owner
        .root_preparation()
        .unwrap()
        .verify_host(owner.current())
        .unwrap();
    let loaned = owner.take_for_play().unwrap();
    assert!(owner.is_playing());
    assert_eq!(owner.advertisement(), &advertisement);
    witness.verify_host(&loaned).unwrap();
    owner
        .root_preparation()
        .unwrap()
        .verify_host(&loaned)
        .unwrap();
    owner.restore_after_play(loaned).unwrap();
    assert!(!owner.is_playing());
    assert_eq!(owner.current().advertisement(), &advertisement);
    witness.verify_host(owner.current()).unwrap();
    // Owner lifetime ends new preparation without invalidating the affine Host
    // earlier during a lawful loan/restore cycle.
    drop(owner);
    // SAFETY: retirement must refuse before this fixed, effect-free factory.
    assert!(unsafe {
        witness.prepare_value::<[u8; 8]>(8, 8, || panic!("retired owner invoked factory"))
    }
    .is_err());
}
