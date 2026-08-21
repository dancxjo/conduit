#![cfg(feature = "form-catalog")]

use std::collections::BTreeMap;

use conduit_ai::{
    context_select_contract, deterministic_context_select_offer, deterministic_rerank_offer,
    install_r3_catalog, rerank_contract, RERANKED_CANDIDATES_VALUE_KIND,
    STRUCTURED_CONTEXT_VALUE_KIND,
};
use conduit_core::{
    verify_plan, BootId, ConnectionBase, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};

fn form(rerank_policy: &str, context_policy: &str, redundancy: &str, ordering: &str) -> String {
    format!(
        "form r3 {{\n rerank: retrieval/rerank(\"{rerank_policy}\", 8, 32)\n select: context/select(\"{context_policy}\", \"tokens/exact-fixture@1\", \"{redundancy}\", \"{ordering}\", 8, 4096, 1024, 32)\n}}\n"
    )
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_r3_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn plan(source: &str) -> conduit_core::Plan {
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "r3", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/r3"),
        boot_id: BootId::from("boot/r3/1"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("host/r3@1"),
        resources: vec![],
        capabilities: vec![
            deterministic_rerank_offer("pid-rerank").unwrap(),
            deterministic_context_select_offer("pid-select").unwrap(),
        ],
        planner_capabilities: vec![],
    };
    let placements = default_expanded_placements(&expanded, core::slice::from_ref(&host)).unwrap();
    plan_expanded_canonical_with_options(
        &expanded,
        core::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
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

#[test]
fn portable_faces_are_typed_bounded_and_require_no_authority() {
    let rerank = rerank_contract();
    let select = context_select_contract();
    assert_eq!(
        rerank.outputs[0].value_kind.as_str(),
        RERANKED_CANDIDATES_VALUE_KIND
    );
    assert_eq!(
        select.inputs[0].value_kind.as_str(),
        RERANKED_CANDIDATES_VALUE_KIND
    );
    assert_eq!(
        select.outputs[0].value_kind.as_str(),
        STRUCTURED_CONTEXT_VALUE_KIND
    );
    for offer in [
        deterministic_rerank_offer("pid-rerank").unwrap(),
        deterministic_context_select_offer("pid-select").unwrap(),
    ] {
        assert_eq!(offer.limits.max_active_instances, 1);
        assert_eq!(offer.limits.max_queue_items, 1);
        assert!(offer.host_operations.is_empty());
        assert!(offer.resource_requirements.is_empty());
        assert!(offer.authority_requirements.is_empty());
    }
}

#[test]
fn proof_ordering_and_redundancy_policies_are_exact_plan_facts() {
    let deterministic = plan(&form(
        "rerank/preserve-hybrid-deterministic@1",
        "context/reranked-diverse@1",
        "keep-all",
        "reranked",
    ));
    let model_chronological = plan(&form(
        "rerank/observed-model-derived@1",
        "context/chronological-diverse@1",
        "one-per-reviewed-group",
        "chronological-oldest-first",
    ));
    assert!(verify_plan(&deterministic));
    assert!(verify_plan(&model_chronological));
    assert_ne!(deterministic.plan_id, model_chronological.plan_id);
    let debug = format!("{model_chronological:?}");
    for fact in [
        "rerank/observed-model-derived@1",
        "context/chronological-diverse@1",
        "one-per-reviewed-group",
        "chronological-oldest-first",
        "tokens/exact-fixture@1",
    ] {
        assert!(debug.contains(fact));
    }
    for forbidden in ["provider", "OpenAI", "file://", "/home/", "credential"] {
        assert!(!debug.contains(forbidden));
    }
}

#[test]
fn unreviewed_reranking_policy_refuses_canonical_expansion() {
    let (startup, profile) = catalogs();
    let source = form(
        "provider/opaque-score@1",
        "context/reranked-diverse@1",
        "keep-all",
        "reranked",
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    assert!(expand_canonical_form(&checked, "r3", &profile).is_err());
}
