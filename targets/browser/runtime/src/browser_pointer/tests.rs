use super::*;

fn sample() -> NormalizedPointerSample {
    NormalizedPointerSample {
        position_x: 250_000,
        position_y: 750_000,
        delta_x: 125_000,
        delta_y: -250_000,
        primary_pressed: true,
        coalesced: 0,
        dropped: 0,
        queue_capacity: 1,
        sequence: 0,
    }
}

#[test]
fn pointer_crosses_checked_plan_and_kernel_with_exact_identities() {
    let delivery = browser_pointer_delivery_contract().unwrap();
    assert_eq!(
        delivery.evolution,
        conduit_core::EvolutionSemantics::CurrentState
    );
    assert_eq!(
        delivery.admission_unit,
        conduit_core::AdmissionUnit::CoherentFrame
    );
    assert_eq!(
        delivery.pressure_policy,
        conduit_core::DeliveryPressurePolicy::CoalesceLatest
    );
    let receipt = execute_browser_pointer(sample()).unwrap();
    assert_eq!(receipt.position_x, 250_000);
    assert_eq!(receipt.schema, "input/pointer-event@1");
    assert!(receipt.value_kind.starts_with("structured-info/profile-"));
    assert!(!receipt.plan_id.is_empty());
    assert!(!receipt.play_id.is_empty());
    assert!(!receipt.sign_id.is_empty());
    assert_ne!(
        receipt.source_placement_id,
        receipt.presentation_placement_id
    );
}

#[test]
fn malformed_normalized_browser_values_refuse_before_play() {
    let mut invalid = sample();
    invalid.position_x = 1_000_001;
    assert!(execute_browser_pointer(invalid).is_err());
    invalid = sample();
    invalid.queue_capacity = 0;
    assert!(execute_browser_pointer(invalid).is_err());
}

#[test]
fn pointer_scheduler_refuses_an_underadmitted_physical_sign_budget_before_play() {
    let value = normalized_pointer_value(sample()).unwrap();
    let (startup, profile) = catalogs(&value).unwrap();
    let syntax = conduit_form::parse_syntax_document(FORM_SOURCE);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "browser-pointer", &profile).unwrap();
    let host = advertisement();
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let fragment = plan.fragments.first().unwrap();
    let mut lowered = lower_plan_fragment(fragment).unwrap();
    lowered.sign_bytes = u32::from(lowered.sign_items)
        * u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>()).unwrap()
        - 1;

    assert_eq!(
        scheduler(fragment, &lowered).err().unwrap(),
        "browser pointer Plan underadmits physical Signs"
    );
}
