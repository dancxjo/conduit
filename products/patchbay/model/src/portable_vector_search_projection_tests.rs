use conduit_ai::{
    exact_vector_search_offer, install_vector_search_catalog, SimilarityMetric,
    VECTOR_SEARCH_RESOURCE_CLASS,
};
use conduit_core::{
    resource_offer, BaseImplementationId, BootId, CapabilityOffer, HostAdvertisement, HostId,
    HostProfileId, OfferGeneration, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_presentation::{
    Presentation, PresentationAspect, PresentationBasis, PresentationDepth, PresentationPlace,
    PresentationPropertyValue, PresentationRole, ProjectionItem,
};
use conduit_std_host::hosted_vector_index::{
    hosted_hnsw_vector_search_offer, HostedHnswProfile, HostedHnswProviderIdentity,
};

use crate::portable_projection::ContentBuilder;
use crate::{PatchbayGraph, PatchbayNavigationProjection};

const SOURCE: &str = "form retrieval {\n search: retrieval/vector-search(4096, 8192, 1024, 8)\n}\n";
const VECTOR_WORK_UNITS: u32 = 65_536;

fn expanded() -> conduit_form::ExpandedCanonicalForm {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_vector_search_catalog(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    expand_canonical_form(&checked, "retrieval", &profiles).unwrap()
}

fn host(name: &str, capability: CapabilityOffer) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/{name}")),
        boot_id: BootId::from(format!("boot/{name}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(format!("host/{name}@1")),
        resources: vec![resource_offer(
            &format!("pool/{name}/vector-index"),
            VECTOR_SEARCH_RESOURCE_CLASS,
            VECTOR_WORK_UNITS,
        )],
        capabilities: vec![capability],
        planner_capabilities: Vec::new(),
    }
}

fn plan(
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

fn projected_property_names(
    expanded: &conduit_form::ExpandedCanonicalForm,
    plan: &conduit_core::Plan,
    aspect: PresentationAspect,
) -> Vec<(String, PresentationPropertyValue)> {
    let graph = PatchbayGraph::from_expanded(expanded).unwrap();
    let mut content = ContentBuilder::new();
    let body_subject = content.subject_with_identity(
        "body/vector-search",
        PresentationRole::Body,
        "Vector search Body",
        "Vector search Body",
    );
    let form = content.subject_with_identity(
        format!("form/{}", plan.checked_form_id.as_str()),
        PresentationRole::Form,
        "retrieval",
        "Portable vector retrieval Form",
    );
    content.contains(&body_subject, &form);
    crate::portable_graph_projection::append_exact_graph(
        &form,
        &graph,
        Some(
            &crate::PlanDocument::from_plan(
                crate::PatchbayRequestId::new("vector/presentation").unwrap(),
                plan,
            )
            .unwrap(),
        ),
        None,
        &mut content,
    );
    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        1,
        "sign/vector-search-born".into(),
    )
    .unwrap();
    let presentation = Presentation::new(
        1,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: None,
            source_document_id: Some(plan.source_document_id.clone()),
            checked_form_id: Some(plan.checked_form_id.clone()),
            expanded_form_id: Some(plan.expanded_form_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: Vec::new(),
        },
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
    )
    .unwrap();
    let navigation = PatchbayNavigationProjection::for_embodied(&presentation).unwrap();
    let gear = presentation
        .subjects
        .iter()
        .find(|subject| subject.role == PresentationRole::Gear)
        .unwrap();
    let mut cursor = navigation.cursor.clone();
    cursor.place = PresentationPlace::Program;
    cursor.aspect = aspect;
    cursor.focus = Some(gear.identity.clone());
    cursor.depth = PresentationDepth::Exact;
    navigation
        .projection
        .project(&presentation, &navigation.navigation, &cursor)
        .unwrap()
        .items
        .iter()
        .filter_map(|membership| match membership.item {
            ProjectionItem::Property(index) => {
                let property = &presentation.properties[usize::from(index)];
                (property.subject == gear.identity)
                    .then(|| (property.name.clone(), property.value.clone()))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn unchanged_semantics_disclose_distinct_exact_and_hnsw_realizations() {
    let expanded = expanded();
    let exact = plan(
        &expanded,
        host("exact", exact_vector_search_offer("exact-process").unwrap()),
    );
    let hnsw = plan(
        &expanded,
        host(
            "hnsw",
            hosted_hnsw_vector_search_offer(
                &HostedHnswProviderIdentity::reviewed("hnsw-process").unwrap(),
                HostedHnswProfile {
                    metric: SimilarityMetric::CosineSimilarity,
                    seed: 7,
                    ef_construction: 32,
                    ef_search: 16,
                },
            )
            .unwrap(),
        ),
    );

    let exact_structure =
        projected_property_names(&expanded, &exact, PresentationAspect::Structure);
    let hnsw_structure = projected_property_names(&expanded, &hnsw, PresentationAspect::Structure);
    assert_eq!(exact_structure, hnsw_structure);
    assert!(exact_structure.iter().any(|(name, value)| {
        name == "kind-id"
            && value == &PresentationPropertyValue::Identity(conduit_ai::VECTOR_SEARCH_KIND.into())
    }));
    for forbidden in [
        "implementation-id",
        "execution-profile-id",
        "vector-search-proof-class",
        "vector-index-resource",
    ] {
        assert!(!exact_structure.iter().any(|(name, _)| name == forbidden));
    }

    let exact_plan = projected_property_names(&expanded, &exact, PresentationAspect::Plan);
    let hnsw_plan = projected_property_names(&expanded, &hnsw, PresentationAspect::Plan);
    assert!(exact_plan.iter().any(|(name, value)| {
        name == "vector-search-proof-class"
            && value == &PresentationPropertyValue::Text("deterministic-exact".into())
    }));
    assert!(hnsw_plan.iter().any(|(name, value)| {
        name == "vector-search-proof-class"
            && value == &PresentationPropertyValue::Text("approximate".into())
    }));
    assert!(hnsw_plan.iter().any(|(name, value)| {
        name == "execution-profile-id"
            && matches!(value, PresentationPropertyValue::Identity(profile) if profile.contains("hnsw/cosine") && profile.contains("ef-search-16"))
    }));
    assert!(exact_plan.iter().any(|(name, value)| {
        name == "vector-index-resource"
            && matches!(value, PresentationPropertyValue::Text(resource) if resource.contains("pool=pool/exact/vector-index") && resource.contains("class=resource/vector-index@1"))
    }));
    assert!(hnsw_plan.iter().any(|(name, value)| {
        name == "vector-index-resource"
            && matches!(value, PresentationPropertyValue::Text(resource) if resource.contains("pool=pool/hnsw/vector-index") && resource.contains("units=65536"))
    }));
}

#[test]
fn unknown_vector_implementation_does_not_invent_a_proof_class() {
    let expanded = expanded();
    let plan = plan(
        &expanded,
        host("exact", exact_vector_search_offer("exact-process").unwrap()),
    );
    let graph = PatchbayGraph::from_expanded(&expanded).unwrap();
    let gear = &graph.gears[0];
    let mut placement = plan.fragments[0].placements[0].clone();
    placement.implementation_id = "unknown/vector-search@1".into();
    let mut content = ContentBuilder::new();
    crate::portable_vector_search_projection::append_vector_search_realization(
        &mut content,
        "gear/vector-search",
        gear,
        &placement,
    );

    assert!(!content
        .properties
        .iter()
        .any(|property| property.name == "vector-search-proof-class"));
    assert!(content
        .properties
        .iter()
        .any(|property| property.name == "vector-index-resource"));
}
