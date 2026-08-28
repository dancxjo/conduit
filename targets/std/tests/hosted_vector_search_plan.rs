use conduit_ai::{
    exact_vector_search_offer, install_vector_search_catalog, SimilarityMetric,
    EXACT_VECTOR_SEARCH_ARTIFACT, EXACT_VECTOR_SEARCH_IMPLEMENTATION, VECTOR_SEARCH_OPERATION,
    VECTOR_SEARCH_RESOURCE_CLASS,
};
use conduit_core::{
    kind_id, resource_offer, BaseImplementationId, BootId, CapabilityOffer, HostAdvertisement,
    HostId, HostProfileId, OfferGeneration, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_std_host::hosted_vector_index::{
    hosted_hnsw_vector_search_offer, HostedHnswProfile, HostedHnswProviderIdentity,
    HOSTED_HNSW_IMPLEMENTATION_ID, HOSTED_HNSW_LIBRARY_NAME, HOSTED_HNSW_LIBRARY_VERSION,
};

const SOURCE: &str = "form retrieval {\n search: retrieval/vector-search(4096, 8192, 1024, 8)\n}\n";
const VECTOR_WORK_UNITS: u32 = 65_536;

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_vector_search_catalog(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert_eq!(syntax.round_trip(), SOURCE);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    expand_canonical_form(&checked, "retrieval", &profile).unwrap()
}

fn host(name: &str, capability: CapabilityOffer, capacity_units: u32) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/{name}")),
        boot_id: BootId::from(format!("boot/{name}/1")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(format!("host/{name}@1")),
        resources: vec![resource_offer(
            &format!("pool/{name}/vector-index"),
            VECTOR_SEARCH_RESOURCE_CLASS,
            capacity_units,
        )],
        capabilities: vec![capability],
        planner_capabilities: vec![],
    }
}

fn hnsw_offer() -> CapabilityOffer {
    hosted_hnsw_vector_search_offer(
        &HostedHnswProviderIdentity::reviewed("pid-4102").unwrap(),
        HostedHnswProfile {
            metric: SimilarityMetric::CosineSimilarity,
            seed: 7,
            ef_construction: 32,
            ef_search: 16,
        },
    )
    .unwrap()
}

fn plan_on(
    expanded: &conduit_form::ExpandedCanonicalForm,
    host: HostAdvertisement,
) -> conduit_core::Plan {
    let placements =
        conduit_planner::default_expanded_placements(expanded, core::slice::from_ref(&host))
            .unwrap();
    conduit_planner::plan_expanded_canonical(
        expanded,
        core::slice::from_ref(&host),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap()
}

#[test]
fn one_authored_vector_search_plans_to_materially_distinct_exact_and_hnsw_backends() {
    let expanded = expanded();
    let exact = plan_on(
        &expanded,
        host(
            "exact",
            exact_vector_search_offer("exact-process-9").unwrap(),
            VECTOR_WORK_UNITS,
        ),
    );
    let approximate = plan_on(&expanded, host("hnsw", hnsw_offer(), VECTOR_WORK_UNITS));

    assert_eq!(exact.source_document_id, approximate.source_document_id);
    assert_eq!(exact.checked_form_id, approximate.checked_form_id);
    assert_eq!(exact.expanded_form_id, approximate.expanded_form_id);
    let exact = &exact.fragments[0].placements[0];
    let approximate = &approximate.fragments[0].placements[0];
    assert_eq!(exact.kind_id, approximate.kind_id);
    assert_eq!(
        exact.kind_contract_revision,
        approximate.kind_contract_revision
    );
    assert_eq!(exact.configuration, approximate.configuration);
    assert_eq!(exact.inputs, approximate.inputs);
    assert_eq!(exact.outputs, approximate.outputs);
    assert_eq!(exact.host_operations, approximate.host_operations);
    assert_eq!(
        exact.resources[0].class_id,
        approximate.resources[0].class_id
    );
    assert_eq!(exact.resources[0].units, VECTOR_WORK_UNITS);
    assert_eq!(approximate.resources[0].units, VECTOR_WORK_UNITS);

    assert_eq!(
        exact.implementation_id.as_str(),
        EXACT_VECTOR_SEARCH_IMPLEMENTATION
    );
    assert_eq!(exact.artifact_id.as_str(), EXACT_VECTOR_SEARCH_ARTIFACT);
    assert!(exact.capability_id.as_str().contains("exact-process-9"));
    assert_eq!(
        approximate.implementation_id.as_str(),
        HOSTED_HNSW_IMPLEMENTATION_ID
    );
    assert!(approximate
        .artifact_id
        .as_str()
        .contains(HOSTED_HNSW_LIBRARY_NAME));
    assert!(approximate
        .artifact_id
        .as_str()
        .contains(HOSTED_HNSW_LIBRARY_VERSION));
    assert!(approximate.capability_id.as_str().contains("pid-4102"));
    assert!(approximate
        .execution_profile_id
        .as_str()
        .contains("hnsw/cosine"));
    assert!(approximate.execution_profile_id.as_str().contains("seed-7"));
    assert!(approximate
        .execution_profile_id
        .as_str()
        .contains("ef-construction-32"));
    assert!(approximate
        .execution_profile_id
        .as_str()
        .contains("ef-search-16"));
    assert_eq!(
        approximate.host_operations[0].contract_id.as_str(),
        VECTOR_SEARCH_OPERATION
    );

    for forbidden in [
        "hnsw",
        "instant-distance",
        "provider",
        "database",
        "pid-4102",
    ] {
        assert!(!SOURCE.contains(forbidden));
    }
}

#[test]
fn incompatible_or_underprovisioned_host_refuses_before_a_plan_exists() {
    let expanded = expanded();
    let underprovisioned = host("small", hnsw_offer(), VECTOR_WORK_UNITS - 1);
    let placements = conduit_planner::default_expanded_placements(
        &expanded,
        core::slice::from_ref(&underprovisioned),
    )
    .unwrap();
    assert!(conduit_planner::plan_expanded_canonical(
        &expanded,
        core::slice::from_ref(&underprovisioned),
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .is_err());

    let mut incompatible_offer = hnsw_offer();
    incompatible_offer.inputs[0].value_kind = kind_id("retrieval/backend-query@1");
    let incompatible = host("incompatible", incompatible_offer, VECTOR_WORK_UNITS);
    assert!(conduit_planner::default_expanded_placements(
        &expanded,
        core::slice::from_ref(&incompatible),
    )
    .is_err());
}
