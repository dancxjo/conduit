//! Planning fixtures only: no inference or device execution is claimed.
use conduit_ai::model_infer_catalog::*;
use conduit_core::*;
use conduit_planner::{default_expanded_placements, plan_expanded_canonical, PlannerError};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../../../semantics/ai/tests/fixtures/onnx-model-fore.conduit");

fn offer(name: &str, property: Option<StructuredConfigurationValue>) -> CapabilityOffer {
    let mut offer = BackOfferBuilder::new(
        model_infer_contract().unwrap(),
        Back {
            capability_id: name.into(),
            execution_profile_id: "fixture/model-planning@1".into(),
            implementation_id: "fixture/model-planning-only@1".into(),
            artifact_id: format!("fixture/{name}@1").into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    offer.realization_properties = property.into_iter().collect();
    offer
}
fn host(offers: Vec<CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/model-planning".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: offers,
    }
}

#[test]
fn authored_model_signature_selects_only_the_exact_concrete_realization() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_model_infer_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(|e| format!("{}: {}", e.code, e.message))
        .unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "tiny-model", &profile).unwrap();
    let signature = configured_model_signature(&authored.expanded.gears[0].configuration).unwrap();
    let ConfigurationValue::Structured(property) = signature_configuration(&signature).unwrap()
    else {
        unreachable!()
    };
    let mut stale = signature;
    stale.identity = "test/stale-model@1".into();
    let ConfigurationValue::Structured(stale) = signature_configuration(&stale).unwrap() else {
        unreachable!()
    };
    let hosts = [host(vec![
        offer("aaa-stale", Some(stale)),
        offer("zzz-current", Some(property)),
    ])];
    let placements = default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let plan = plan_expanded_canonical(
        &authored.expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(
        plan.fragments[0].placements[0].capability_id.as_str(),
        "zzz-current"
    );
    assert_eq!(
        plan.fragments[0].placements[0].artifact_id.as_str(),
        "fixture/zzz-current@1"
    );
    verify_plan(&plan).unwrap();
}

#[test]
fn a_host_without_the_requested_property_is_ineligible() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_model_infer_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup)
        .map_err(|e| format!("{}: {}", e.code, e.message))
        .unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "tiny-model", &profile).unwrap();
    let result =
        default_expanded_placements(&authored.expanded, &[host(vec![offer("missing", None)])]);
    let Err(PlannerError::HardRealizationRequirementUnsatisfied(detail)) = result else {
        panic!("missing exact property must refuse")
    };
    assert!(detail.contains("MissingProperty"));
}
