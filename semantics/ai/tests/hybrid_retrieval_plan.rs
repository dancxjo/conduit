#![cfg(feature = "form-catalog")]

use std::collections::BTreeMap;

use conduit_ai::{
    deterministic_hybrid_retrieval_offer, hybrid_retrieval_contract,
    install_hybrid_retrieval_catalog, HYBRID_RETRIEVAL_CANDIDATES_VALUE_KIND,
    RETRIEVAL_CANDIDATE_BATCH_VALUE_KIND,
};
use conduit_core::{
    verify_plan, BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};

fn plan(source: &str) -> conduit_core::Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_hybrid_retrieval_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "hybrid", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/hybrid"),
        boot_id: BootId::from("boot/hybrid/1"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("host/hybrid@1"),
        resources: vec![],
        capabilities: vec![deterministic_hybrid_retrieval_offer("pid-7").unwrap()],
        planner_capabilities: vec![],
    };
    let placements = default_expanded_placements(&expanded, core::slice::from_ref(&host)).unwrap();
    plan_expanded_canonical_with_options(
        &expanded,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 1,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
}

fn form(policy: &str, temporal: &str) -> String {
    format!(
        "form hybrid {{\n fusion: retrieval/hybrid-fuse(\"{policy}\", \"reciprocal-rank\", 60, \"{temporal}\", 8, 8, 32)\n}}\n"
    )
}

#[test]
fn portable_face_names_four_explicit_paths_and_no_realization_facts() {
    let contract = hybrid_retrieval_contract();
    assert_eq!(contract.inputs.len(), 4);
    assert!(contract
        .inputs
        .iter()
        .all(|port| port.value_kind.as_str() == RETRIEVAL_CANDIDATE_BATCH_VALUE_KIND));
    assert_eq!(
        contract.outputs[0].value_kind.as_str(),
        HYBRID_RETRIEVAL_CANDIDATES_VALUE_KIND
    );
    let offer = deterministic_hybrid_retrieval_offer("pid-7").unwrap();
    assert!(offer.host_operations.is_empty());
    assert!(offer.resource_requirements.is_empty());
    assert!(offer.authority_requirements.is_empty());
}

#[test]
fn fusion_policy_and_temporal_behavior_are_exact_plan_configuration() {
    let ordinary = plan(&form("fusion/reciprocal-rank@1", "none"));
    let origin = plan(&form("fusion/reciprocal-rank-origin@1", "created-duration"));
    assert!(verify_plan(&ordinary));
    assert!(verify_plan(&origin));
    assert_ne!(ordinary.plan_id, origin.plan_id);
    let gear = &origin.fragments[0].placements[0];
    assert_eq!(gear.configuration.len(), 7);
    assert!(gear.configuration.iter().any(|entry| {
        entry.key == "policy"
            && entry.value
                == conduit_core::ConfigurationValue::Text("fusion/reciprocal-rank-origin@1".into())
    }));
    assert!(gear.configuration.iter().any(|entry| {
        entry.key == "temporal-hard-filter"
            && entry.value == conduit_core::ConfigurationValue::Text("created-duration".into())
    }));

    let debug = format!("{origin:?}");
    for forbidden in ["provider", "Pinecone", "OpenAI", "file://", "/home/"] {
        assert!(!debug.contains(forbidden));
    }
}

#[test]
fn unreviewed_policy_refuses_during_canonical_expansion() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_hybrid_retrieval_catalog(&mut startup, &mut profile).unwrap();
    let source = form("provider/opaque-score@1", "none");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    assert!(expand_canonical_form(&checked, "hybrid", &profile).is_err());
}
