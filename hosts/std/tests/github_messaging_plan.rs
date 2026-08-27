use conduit_core::{
    BootId, ConnectionBase, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::hosted_messaging::{
    github_messaging_authority_grant, github_messaging_offer, github_messaging_resource_offer,
};

const SOURCE: &str = include_str!("../../../examples/messaging-delivery.conduit");

#[test]
fn unchanged_portable_form_selects_exact_github_profile_with_authority() {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_chat::install_messaging_catalogs(&mut startup, &mut profiles).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "messaging-delivery", &profiles).unwrap();
    let host = host();
    let github = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_chat::MESSAGING_DELIVERY_KIND)
        .unwrap();
    let grant = github_messaging_authority_grant(
        github,
        "grant/github-comment",
        host.host_id.clone(),
        host.boot_id.clone(),
    )
    .unwrap();
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let connection_bases = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
    let refused = conduit_planner::plan_expanded_canonical_with_options(
        &authored.expanded,
        core::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    );
    assert!(refused.is_err(), "delivery must refuse absent authority");
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &authored.expanded,
        core::slice::from_ref(&host),
        &placements,
        &[ConnectionBase::Local],
        conduit_planner::PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let delivery = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_chat::MESSAGING_DELIVERY_KIND)
        .unwrap();
    assert_eq!(delivery.authority.len(), 1);
    assert_eq!(delivery.resources.len(), 1);
    assert_eq!(
        delivery.implementation_id.as_str(),
        "std/kernel-messaging-github-issue-comment@1"
    );
}

fn host() -> HostAdvertisement {
    let message = conduit_std_host::hosted_messaging::messaging_std_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_chat::MESSAGING_MESSAGE_KIND)
        .unwrap();
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/github-messaging"),
        boot_id: BootId::from("boot/github-messaging"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/github-messaging-proof@1"),
        resources: vec![github_messaging_resource_offer()],
        planner_capabilities: vec![],
        capabilities: vec![message, github_messaging_offer()],
    }
}
