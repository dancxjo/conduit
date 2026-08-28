#![cfg(feature = "form-catalog")]

use conduit_ai::{
    deterministic_context_select_offer, deterministic_rerank_offer, install_r3_catalog,
    install_rag_answer_catalog, ordinary_rag_answer_offer, rag_answer_contract,
    GROUNDED_ANSWER_VALUE_KIND, RETRIEVAL_QUERY_INTENT_VALUE_KIND, STRUCTURED_CONTEXT_VALUE_KIND,
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
use std::collections::BTreeMap;

fn form(answer_kind: &str, maximum_citations: u16) -> String {
    format!(
        "form r4 {{\n rerank: retrieval/rerank(\"rerank/preserve-hybrid-deterministic@1\", 8, 32)\n select: context/select(\"context/reranked-diverse@1\", \"tokens/exact-fixture@1\", \"keep-all\", \"reranked\", 8, 4096, 1024, 32)\n answer: rag/answer(\"grounding/exact-context-citations@1\", \"{answer_kind}\", 4096, 16, {maximum_citations}, 1024)\n rerank.result > select.candidates\n select.result > answer.context\n}}\n"
    )
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_r3_catalog(&mut startup, &mut profile).unwrap();
    install_rag_answer_catalog(&mut startup, &mut profile).unwrap();
    (startup, profile)
}

fn plan(source: &str) -> conduit_core::Plan {
    let (startup, profile) = catalogs();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "r4", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/r4"),
        boot_id: BootId::from("boot/r4/1"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("host/r4@1"),
        resources: vec![],
        capabilities: vec![
            deterministic_rerank_offer("pid-rerank").unwrap(),
            deterministic_context_select_offer("pid-context").unwrap(),
            ordinary_rag_answer_offer("pid-answer").unwrap(),
        ],
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

#[test]
fn ordinary_answer_face_consumes_query_and_structured_context_without_prompt_semantics() {
    let contract = rag_answer_contract();
    assert_eq!(contract.inputs.len(), 2);
    assert_eq!(
        contract.inputs[0].value_kind.as_str(),
        RETRIEVAL_QUERY_INTENT_VALUE_KIND
    );
    assert_eq!(
        contract.inputs[1].value_kind.as_str(),
        STRUCTURED_CONTEXT_VALUE_KIND
    );
    assert_eq!(
        contract.outputs[0].value_kind.as_str(),
        GROUNDED_ANSWER_VALUE_KIND
    );
    let offer = ordinary_rag_answer_offer("pid-answer").unwrap();
    assert!(offer.host_operations.is_empty());
    assert!(offer.resource_requirements.is_empty());
    assert!(offer.authority_requirements.is_empty());
    let debug = format!("{contract:?}{offer:?}");
    for forbidden in ["prompt", "provider", "browser", "credential", "tool-call"] {
        assert!(!debug.contains(forbidden));
    }
}

#[test]
fn retrieval_context_answer_chain_and_grounding_bounds_are_exact_plan_facts() {
    let text = plan(&form("value/text-utf8@1", 8));
    let structured = plan(&form("value/structured-answer@1", 16));
    assert!(verify_plan(&text));
    assert!(verify_plan(&structured));
    assert_ne!(text.plan_id, structured.plan_id);
    let debug = format!("{structured:?}");
    for fact in [
        "retrieval/rerank",
        "context/select",
        "rag/answer",
        "grounding/exact-context-citations@1",
        "value/structured-answer@1",
        "maximum-citations",
    ] {
        assert!(debug.contains(fact));
    }
    for forbidden in [
        "OpenAI",
        "provider",
        "prompt-template",
        "https://",
        "/home/",
    ] {
        assert!(!debug.contains(forbidden));
    }
}

#[test]
fn opaque_provider_citation_policy_refuses_canonical_expansion() {
    let (startup, profile) = catalogs();
    let source = form("value/text-utf8@1", 8).replace(
        "grounding/exact-context-citations@1",
        "provider/citation-magic@1",
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    assert!(expand_canonical_form(&checked, "r4", &profile).is_err());
}
