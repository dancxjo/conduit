use std::collections::BTreeMap;

use conduit_ai::{deterministic_hybrid_retrieval_offer, install_hybrid_retrieval_catalog};
use conduit_core::{
    BootId, ConnectionBase, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_canonical_with_options, PlanningOptions,
};

pub fn exact_hybrid_plan(policy_identity: &str, maximum_value_bytes: u32) -> conduit_core::Plan {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_hybrid_retrieval_catalog(&mut startup, &mut profile).unwrap();
    let source = format!(
        "form hybrid {{\n fusion: retrieval/hybrid-fuse(\"{policy_identity}\", \"reciprocal-rank\", 60, \"none\", 8, 8, 32)\n}}\n"
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
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
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: maximum_value_bytes,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
}
