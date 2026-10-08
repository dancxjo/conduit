use super::KernelResourceLedger;
use crate::kernel_multivalue::{advertisement, plan_local, profile_catalog};
use conduit_core::{
    seal_plan, BootId, ExternalEffectBehavior, HostId, ImplementationId, KindSemanticLaw,
    OfferGeneration, PlotIdentity, ResourcePoolId,
};
use conduit_plot::parse;

#[test]
fn body_reservation_verifies_complete_activation_plan_before_exact_offer() {
    let plan = crate::flow_activation::authored_todo_plan();
    let fragment = &plan.fragments[0];
    let host = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: fragment.host_id.clone(),
        boot_id: fragment.placements[0].boot_id.clone(),
        offer_generation: fragment.placements[0].offer_generation,
        profile: conduit_core::HostProfileId::from("std-todo-offer-proof/profile"),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![crate::flow_activation::todo_scan_offer(
            &conduit_todo_plot::TodoState::new("Groceries".into()).unwrap(),
            64,
        )
        .unwrap()],
    };
    let mut ledger = KernelResourceLedger::new(&host).unwrap();
    let reservation = ledger
        .prepare_and_reserve_plans(&host, &[(&plan, false)])
        .expect("the whole sealed activation Plan can be reserved")
        .pop()
        .unwrap();
    assert_eq!(reservation.plan_id, plan.plan_id);
    ledger.release(reservation).unwrap();

    let mut unadvertised = host.clone();
    unadvertised.capabilities.clear();
    let mut absent_ledger = KernelResourceLedger::new(&unadvertised).unwrap();
    let error = absent_ledger
        .prepare_and_reserve_plans(&unadvertised, &[(&plan, false)])
        .unwrap_err();
    assert!(error.contains("unavailable capability"), "{error}");
    assert!(absent_ledger.is_idle());

    let mut stale = plan.clone();
    stale.activations.clear();
    let error = ledger
        .prepare_and_reserve_plans(&host, &[(&stale, false)])
        .unwrap_err();
    assert!(error.contains("lowering"), "{error}");
    assert!(ledger.is_idle());

    let mut extra = plan.clone();
    extra.fragments.push(extra.fragments[0].clone());
    let error = ledger
        .prepare_and_reserve_plans(&host, &[(&extra, false)])
        .unwrap_err();
    assert!(error.contains("exactly one fragment"), "{error}");
    assert!(ledger.is_idle());
}

#[test]
fn exact_reservation_rejects_overlap_releases_and_does_not_grow() {
    let host = advertisement(
        HostId::from("resource-host"),
        BootId::from("resource-boot"),
        OfferGeneration(1),
    );
    let plot = parse(
        include_str!("../../../../proof/fixtures/plots/kernel-multivalue.conduit"),
        &profile_catalog(),
    )
    .expect("multi-value plot parses");
    let plan = plan_local(&plot, &host).expect("multi-value plan resolves");
    let fragment = &plan.fragments[0];
    let mut ledger = KernelResourceLedger::new(&host).expect("ledger installs");
    let capacity = ledger.allocation_capacity();

    let first = ledger
        .prepare_and_reserve(&host, fragment)
        .expect("first exact reservation succeeds");
    assert_eq!(first.plan_id, fragment.plan_id);
    assert_eq!(first.bindings.len(), 3);
    let overlap = ledger
        .prepare_and_reserve(&host, fragment)
        .expect_err("second reservation exceeds the selected capability instance limit");
    assert!(
        overlap.contains("combined active-instance limit"),
        "{overlap}"
    );
    ledger.release(first).expect("terminal release succeeds");
    let second = ledger
        .prepare_and_reserve(&host, fragment)
        .expect("released capacity can be reserved again");
    ledger.release(second).expect("second release succeeds");
    assert_eq!(ledger.allocation_capacity(), capacity);

    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let mut wrong_implementation = fragment.clone();
    wrong_implementation.placements[0].implementation_id =
        ImplementationId::from("std/not-installed@1");
    let wrong_implementation = seal_plan(identity.clone(), vec![wrong_implementation]);
    let error = ledger
        .prepare_and_reserve(&host, &wrong_implementation.fragments[0])
        .expect_err("resealed implementation lie must fail before reservation");
    assert!(error.contains("installed exact capability"), "{error}");

    let mut wrong_semantics = fragment.clone();
    wrong_semantics.placements[0]
        .semantic_contract
        .laws
        .push(KindSemanticLaw::ExternalEffects(
            ExternalEffectBehavior::Observable,
        ));
    let wrong_semantics = seal_plan(identity.clone(), vec![wrong_semantics]);
    let error = ledger
        .prepare_and_reserve(&host, &wrong_semantics.fragments[0])
        .expect_err("resealed semantic-contract lie must fail before reservation");
    assert!(error.contains("installed exact capability"), "{error}");

    let coverage = conduit_language::LanguageCoverage::new(
        "proof/exact-declaration".into(),
        conduit_plot::rust_binding::BoundedSequence::try_from_iter([
            conduit_language::LanguageId::new("English".into()).unwrap(),
        ])
        .unwrap(),
        conduit_plot::rust_binding::BoundedSequence::try_from_iter([]).unwrap(),
        "declaration@1".into(),
        conduit_plot::rust_binding::BoundedSequence::try_from_iter([]).unwrap(),
        false,
    )
    .unwrap();
    let mut wrong_coverage = fragment.clone();
    wrong_coverage.placements[0].realization_properties =
        vec![conduit_language::language_coverage_property(coverage).unwrap()];
    let wrong_coverage = seal_plan(identity.clone(), vec![wrong_coverage]);
    let error = ledger
        .prepare_and_reserve(&host, &wrong_coverage.fragments[0])
        .expect_err(
            "a resealed declaration not offered by this exact Back must refuse before reservation",
        );
    assert!(error.contains("installed exact capability"), "{error}");
    assert_eq!(ledger.allocation_capacity(), capacity);

    let mut wrong_pool = fragment.clone();
    wrong_pool.placements[0].resources[0].pool_id = ResourcePoolId::from("not/offered");
    let wrong_pool = seal_plan(identity, vec![wrong_pool]);
    let error = ledger
        .prepare_and_reserve(&host, &wrong_pool.fragments[0])
        .expect_err("resealed resource-pool lie must fail before reservation");
    assert!(error.contains("not offered"), "{error}");
}

#[test]
fn pool_member_reservation_is_atomic_and_releases_exact_capacity() {
    let host = advertisement(
        HostId::from("pool-resource-host"),
        BootId::from("pool-resource-boot"),
        OfferGeneration(1),
    );
    let plot = parse(
        include_str!("../../../../proof/fixtures/plots/kernel-multivalue.conduit"),
        &profile_catalog(),
    )
    .expect("multi-value plot parses");
    let plan = plan_local(&plot, &host).expect("multi-value plan resolves");
    let placement = &plan.fragments[0].placements[0];
    let mut ledger = KernelResourceLedger::new(&host).expect("ledger installs");

    let first = ledger
        .reserve_pool_member(
            &host,
            plan.plan_id.clone(),
            &placement.capability_id,
            &placement.resources,
        )
        .expect("selected member reserves its exact capability and resources");
    let overlap = ledger
        .reserve_pool_member(
            &host,
            plan.plan_id.clone(),
            &placement.capability_id,
            &placement.resources,
        )
        .expect_err("a second member cannot exceed exact capacity");
    assert!(overlap.contains("active-instance capacity"), "{overlap}");

    ledger
        .release(first)
        .expect("member release restores capacity");
    let replacement = ledger
        .reserve_pool_member(
            &host,
            plan.plan_id.clone(),
            &placement.capability_id,
            &placement.resources,
        )
        .expect("released capacity admits a later operation");
    ledger
        .release(replacement)
        .expect("replacement release restores capacity");
}
