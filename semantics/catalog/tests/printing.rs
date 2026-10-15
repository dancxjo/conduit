//! Checked portable print Fore and exact effect/authority Plan admission.
use conduit_core::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_plot::{ProfileCatalog, StartupCatalog};
use conduit_semantic_catalog::*;
use std::collections::BTreeMap;

#[test]
fn three_print_jobs_use_ordinary_checked_ports_and_explicit_host_authority() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_print_catalogs(&mut startup, &mut profile).unwrap();
    let source = include_str!("../../../plots/print-contract/main.conduit");
    assert!(!source.contains("ipp://"));
    assert!(!source.contains("/dev/"));
    let plot = conduit_plot::parse_with_startup(source, &startup, &profile).unwrap();
    assert_eq!(plot.gears.len(), 3);
    let kind = print_submit_semantic_contract();
    assert!(kind
        .semantic_laws
        .contains(&KindSemanticLaw::Replay(ReplayBehavior::Ineligible)));
    assert!(kind
        .semantic_laws
        .contains(&KindSemanticLaw::ExternalEffects(
            ExternalEffectBehavior::Observable
        )));
    let authority = AuthorityRequirement {
        contract_id: PRINT_DOCUMENT_AUTHORITY.into(),
        host_call_contract_id: PRINT_SUBMIT_HOST_CALL.into(),
        subject_kind: kind.kind_id.clone(),
    };
    let mut offer = capability_offer_from_parts! {
        semantic_contract: kind.semantic_contract(),
        startup_parameters:kind.startup_parameters.clone(),shorthand:kind.shorthand.clone(),
        capability_id:"fixture/print-provider".into(),kind_id:kind.kind_id.clone(),kind_contract_revision:kind.kind_contract_revision.clone(),
        implementation:ImplementationOffer {execution_profile_id:"fixture/print-simulator@1".into(),implementation_id:"fixture/print-protocol@1".into(),artifact_id:"fixture/print-protocol-artifact@1".into()},
        inputs:kind.inputs.clone(),outputs:kind.outputs.clone(),
        host_calls:vec![HostCallRequirement {contract_id:PRINT_SUBMIT_HOST_CALL.into(),target_kind:Some(kind.kind_id.clone()),maximum_in_flight:1,maximum_input_bytes:8192,maximum_output_bytes:8192}],
        resource_requirements:vec![resource_requirement("fixture/print-slot",1)],
        authority_requirements:vec![authority.clone()],
        limits:CapabilityLimits {max_active_instances:3,max_queue_items:1,max_queue_bytes:8192},
    };
    let support = PrintSupport::new(
        PrintColorSupport::new(
            BoundedSequence::try_from_iter([PrintColor::Monochrome, PrintColor::Color]).unwrap(),
        )
        .unwrap(),
        10,
        PrintMediaSupport::new(
            BoundedSequence::try_from_iter([PrintMedia::A4, PrintMedia::Letter]).unwrap(),
        )
        .unwrap(),
        PrintSidesSupport::new(
            BoundedSequence::try_from_iter([PrintSides::OneSided, PrintSides::DuplexLongEdge])
                .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    offer.realization_properties = vec![support.property().unwrap()];
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/printer-owner".into(),
        boot_id: "fixture/printer-owner/boot-1".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/print-protocol-only@1".into(),
        bases: vec![],
        resources: vec![resource_offer(
            "fixture/print-slot",
            "fixture/print-slot",
            3,
        )],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let placements =
        conduit_planner::default_placements(&plot, core::slice::from_ref(&host)).unwrap();
    let grant = authority_grant(
        "grant/print",
        &authority,
        host.host_id.clone(),
        host.boot_id.clone(),
        host.capabilities[0].capability_id.clone(),
    );
    let empty = BTreeMap::new();
    let lines = BTreeMap::new();
    let prepare = |grants: &[AuthorityGrant]| {
        conduit_planner::plan_with_options(
            &plot,
            core::slice::from_ref(&host),
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &empty,
                line_candidates: &lines,
                connection_item_capacity: 1,
                connection_byte_capacity: 8192,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
    };
    assert!(prepare(&[]).is_err());
    let plan = prepare(core::slice::from_ref(&grant)).unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.fragments[0].placements.len(), 3);
    for placement in &plan.fragments[0].placements {
        assert_eq!(placement.authority.len(), 1);
        assert_eq!(
            PrintSupport::from_property(&placement.realization_properties[0]).unwrap(),
            support
        );
        assert_eq!(
            placement.host_calls[0].contract_id.as_str(),
            PRINT_SUBMIT_HOST_CALL
        );
        assert!(placement
            .semantic_contract
            .laws
            .contains(&KindSemanticLaw::Replay(ReplayBehavior::Ineligible)));
    }
    let mut stale = grant;
    stale.boot_id = "fixture/stale-boot".into();
    assert!(prepare(&[stale]).is_err());
}
