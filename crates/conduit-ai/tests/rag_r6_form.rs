#![cfg(feature = "form-catalog")]

use std::collections::BTreeMap;

use conduit_ai::{
    deterministic_context_select_offer, deterministic_hybrid_retrieval_offer,
    deterministic_rerank_offer, install_hybrid_retrieval_catalog, install_r3_catalog,
    install_rag_answer_catalog, ordinary_rag_answer_offer,
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

const FORM: &str = r#"form project_history_rag {
    fusion: retrieval/hybrid-fuse("fusion/reciprocal-rank@1", "reciprocal-rank", 60, "none", 16, 16, 64)
    rerank: retrieval/rerank("rerank/preserve-hybrid-deterministic@1", 16, 64)
    context: context/select("context/chronological-diverse@1", "tokens/exact-fixture@1", "keep-all", "chronological-oldest-first", 16, 16384, 4096, 64)
    answer: rag/answer("grounding/exact-context-citations@1", "value/text-utf8@1", 16384, 16, 32, 4096)

    fusion.candidates > rerank.candidates
    rerank.result > context.candidates
    context.result > answer.context
}
"#;

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_hybrid_retrieval_catalog(&mut startup, &mut profile).unwrap();
    install_r3_catalog(&mut startup, &mut profile).unwrap();
    install_rag_answer_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn plan(
    process: &str,
) -> (
    conduit_form::CheckedSyntaxDocument,
    conduit_form::ExpandedCanonicalForm,
    conduit_core::Plan,
) {
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(FORM), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "project_history_rag", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/r6/{process}")),
        boot_id: BootId::from(format!("boot/r6/{process}/1")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("host/r6@1"),
        resources: vec![],
        capabilities: vec![
            deterministic_hybrid_retrieval_offer(process).unwrap(),
            deterministic_rerank_offer(process).unwrap(),
            deterministic_context_select_offer(process).unwrap(),
            ordinary_rag_answer_offer(process).unwrap(),
        ],
        planner_capabilities: vec![],
    };
    let placements = default_expanded_placements(&expanded, core::slice::from_ref(&host)).unwrap();
    let plan = plan_expanded_canonical_with_options(
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
    .unwrap();
    (checked, expanded, plan)
}

#[test]
fn ordinary_capstone_graph_is_exact_and_generically_inspectable() {
    let (checked, expanded, plan) = plan("realization-a");
    assert!(verify_plan(&plan));
    assert_eq!(expanded.gears.len(), 4);
    assert_eq!(expanded.connections.len(), 3);
    assert_eq!(plan.fragments[0].placements.len(), 4);
    assert_eq!(plan.fragments[0].connections.len(), 3);

    let explanation = format!("{checked:?}{expanded:?}{plan:?}");
    for fact in [
        "retrieval/hybrid-fuse",
        "retrieval/rerank",
        "context/select",
        "rag/answer",
        "fusion/reciprocal-rank@1",
        "context/chronological-diverse@1",
        "grounding/exact-context-citations@1",
        "maximum-items",
        "maximum-citations",
    ] {
        assert!(explanation.contains(fact), "missing {fact}");
    }
    for forbidden in [
        "OpenAI",
        "Pinecone",
        "prompt-template",
        "https://",
        "/home/",
    ] {
        assert!(!explanation.contains(forbidden), "leaked {forbidden}");
    }
}

#[test]
fn realization_swap_changes_plan_truth_not_portable_meaning() {
    let (checked_a, expanded_a, plan_a) = plan("realization-a");
    let (checked_b, expanded_b, plan_b) = plan("realization-b");
    assert_eq!(
        checked_a.forms[0].checked_form_id,
        checked_b.forms[0].checked_form_id
    );
    assert_eq!(expanded_a.expanded_form_id, expanded_b.expanded_form_id);
    assert_ne!(plan_a.plan_id, plan_b.plan_id);
    assert!(verify_plan(&plan_a));
    assert!(verify_plan(&plan_b));
}
