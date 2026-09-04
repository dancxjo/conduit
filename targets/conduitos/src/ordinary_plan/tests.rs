use super::*;
use crate::offer::CpuFeatures;
use conduit_core::{ExecutionScheduling, FormIdentity, seal_plan};

fn fixture() -> (BootIdentities, HostOffer<'static>) {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        256 * 1024,
    );
    (identities, offer)
}

#[test]
fn ordinary_source_checks_plans_lowers_and_installs() {
    let (identities, offer) = fixture();
    let prepared = prepare(&identities, &offer, "build").unwrap();
    assert_eq!(prepared.active_play.plan_id, prepared.plan_id);
    assert_eq!(
        prepared.active_play.boot_id.as_str(),
        hex_identity(&identities.boot)
    );
    assert!(prepared.planned_sign_items > 0 && prepared.planned_sign_bytes > 0);
    let [region] = prepared.plan.fragments[0].execution_regions.as_slice() else {
        panic!("ordinary Plan must contain exactly one execution region");
    };
    assert_eq!(region.region_id.as_str(), "region/0");
    assert_eq!(region.admitted_placements.len(), ORDINARY_PLACEMENT_COUNT);
    assert_eq!(
        region.execution_profile_id.as_str(),
        COOPERATIVE_REGION_PROFILE
    );
    assert_eq!(
        region.scheduling,
        ExecutionScheduling::CooperativeBoundedStep
    );
    assert_eq!(region.lane_count, 1);
    assert_eq!(region.lane_resource.units, 1);
    assert_eq!(region.requirements.runtime_memory_bytes, 12_288);
    assert_eq!(region.requirements.timer_slots, 0);
    assert_eq!(region.requirements.cord_item_capacity, 2);
    assert_eq!(region.requirements.cord_byte_capacity, CORD_BYTES * 2);
    assert!(!region.preemption_required && !region.isolation_required);
    assert!(!ORDINARY_FORM_SOURCE.contains("lane"));
    assert!(!ORDINARY_FORM_SOURCE.contains("preemption"));
}

#[test]
fn resealed_wrong_lane_requirement_is_rejected_before_play() {
    let (identities, offer) = fixture();
    let prepared = prepare(&identities, &offer, "build").unwrap();
    let mut fragments = prepared.plan.fragments;
    fragments[0].execution_regions[0].lane_count = 2;
    fragments[0].execution_regions[0].lane_resource.units = 2;
    fragments[0].execution_regions[0]
        .lane_resource
        .compute
        .as_mut()
        .unwrap()
        .selected_lanes = 2;
    let plan = seal_plan(
        FormIdentity {
            source_document_id: prepared.source_document_id,
            checked_form_id: prepared.checked_form_id,
            expanded_form_id: prepared.expanded_form_id,
        },
        fragments,
    );
    assert!(conduit_core::verify_plan(&plan));
    assert_eq!(
        validate_execution_region(&plan.fragments[0], &prepared.advertisement, &offer),
        Err(PreparationError::PlanRejected)
    );
}

#[test]
fn unavailable_execution_lane_is_rejected_before_play() {
    let (identities, mut offer) = fixture();
    let lane = offer
        .bases
        .iter_mut()
        .find(|base| base.kind == crate::machine::BaseKind::ExecutionLane)
        .unwrap();
    lane.capacity = 0;
    assert_eq!(
        prepare(&identities, &offer, "build").err(),
        Some(PreparationError::OfferMismatch)
    );
}

#[test]
fn stale_boot_and_unavailable_implementation_fail_closed() {
    let (identities, mut offer) = fixture();
    offer.boot_id = [3; 32];
    assert!(matches!(
        prepare(&identities, &offer, "build"),
        Err(PreparationError::OfferMismatch)
    ));
    offer.boot_id = identities.boot;
    offer.capabilities[2].maximum_output_bytes = (TEXT_LITERAL.len() - 1) as u32;
    assert_eq!(
        prepare(&identities, &offer, "build").err(),
        Some(PreparationError::OfferMismatch)
    );
    offer.capabilities[2].maximum_output_bytes = conduit_text::MAX_TEXT_BYTES;
    offer.capabilities[3].implementation = "unavailable";
    assert_eq!(
        prepare(&identities, &offer, "build").err(),
        Some(PreparationError::OfferMismatch)
    );
    offer.capabilities[3].implementation = crate::offer::TEXT_UPPER_IMPLEMENTATION;
    offer.capabilities[3].maximum_output_bytes -= 1;
    assert_eq!(
        prepare(&identities, &offer, "build").err(),
        Some(PreparationError::OfferMismatch)
    );
}

#[test]
fn stale_offer_plan_identity_and_missing_serial_base_fail_closed() {
    let (identities, mut offer) = fixture();
    let prepared = prepare(&identities, &offer, "build").unwrap();

    offer.generation += 1;
    assert_eq!(
        validate_execution_region(&prepared.plan.fragments[0], &prepared.advertisement, &offer,),
        Err(PreparationError::PlanRejected)
    );

    let mut stale_plan = prepared.plan;
    stale_plan.plan_id = PlanId::from("stale-plan");
    assert!(!conduit_core::verify_plan(&stale_plan));

    let (identities, mut offer) = fixture();
    offer
        .bases
        .iter_mut()
        .find(|base| base.kind == crate::machine::BaseKind::Serial)
        .unwrap()
        .capacity = 0;
    assert_eq!(
        prepare(&identities, &offer, "build").err(),
        Some(PreparationError::OfferMismatch)
    );
}

#[test]
fn insufficient_memory_timer_and_sign_reserves_fail_before_play() {
    let (identities, mut offer) = fixture();
    offer.resources[0].capacity = 4_096;
    assert!(matches!(
        prepare(&identities, &offer, "build"),
        Err(PreparationError::PlanRejected)
    ));
    offer.resources[0].capacity = 256 * 1024;
    offer.resources[2].capacity = 0;
    assert!(matches!(
        prepare(&identities, &offer, "build"),
        Err(PreparationError::OfferMismatch)
    ));
    offer.resources[2].capacity = 1;
    offer.sign_item_capacity = 6;
    assert!(matches!(
        prepare(&identities, &offer, "build"),
        Err(PreparationError::PlanRejected)
    ));
}

#[test]
fn undersized_cord_reserve_and_stale_planned_boot_fail_closed() {
    let (identities, offer) = fixture();
    let advertisement = advertisement(&identities, &offer, "build").unwrap();
    let form = crate::ordinary_form::checked_expanded_text_form(ORDINARY_FORM_SOURCE).unwrap();
    let hosts = [advertisement];
    let placements = default_expanded_placements(&form, &hosts).unwrap();
    assert_eq!(
        validate_text_capacity(&form, (TEXT_LITERAL.len() - 1) as u32),
        Err(PreparationError::PlanRejected)
    );
    let mut plan = conduit_planner::plan_expanded_canonical(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    plan.fragments[0].boot_id = BootId::from("stale-boot");
    assert!(lower_plan_fragment(&plan.fragments[0]).is_err());
}

#[test]
fn oversized_text_is_refused_during_source_checking() {
    let oversized = "x".repeat(conduit_text::MAX_TEXT_BYTES as usize + 1);
    let source = format!("form too-large {{\n    \"{oversized}\" > presentation/text\n}}\n");
    let syntax = conduit_form::parse_syntax_document(&source);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    assert!(conduit_form::expand_canonical_form(&checked, "too-large", &profile).is_err());

    let malformed = conduit_form::parse_syntax_document(
        "form malformed {\n    \"bad\\q\" > presentation/text\n}\n",
    );
    assert!(conduit_form::check_syntax_document(&malformed, &startup).is_err());
}
