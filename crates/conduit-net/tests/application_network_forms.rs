#![cfg(feature = "form-catalog")]

use conduit_core::{
    authority_grant, resource_offer, BootId, ConnectionBase, HostAdvertisement, HostId,
    HostProfileId, OfferGeneration, StructuredInfoTypeShape, DEFAULT_CONNECTION_BYTE_CAPACITY,
    DEFAULT_CONNECTION_ITEM_CAPACITY, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_net::{
    application_network_std_offers, deterministic_network_fixture, dns_result_type,
    install_application_network_catalogs, network_connection_state_type, DnsResult,
    DNS_RESOLVE_AUTHORITY, DNS_RESOLVE_KIND, DNS_RESOLVER_RESOURCE, NETWORK_CONNECT_AUTHORITY,
    NETWORK_CONNECT_KIND, NETWORK_CONNECTION_RESOURCE,
};

const SOURCE: &str = include_str!("../../../examples/network-resolution.conduit");

#[test]
fn ordinary_form_plans_dns_and_connection_with_explicit_resources_and_authority() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_application_network_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authored =
        expand_canonical_form_for_authoring(&checked, "network-resolution", &profile).unwrap();
    let host = host(application_network_std_offers());
    let grants: Vec<_> = host
        .capabilities
        .iter()
        .filter(|offer| !offer.authority_requirements.is_empty())
        .map(|offer| {
            authority_grant(
                &format!("grant/{}", offer.kind_id.as_str()),
                &offer.authority_requirements[0],
                host.host_id.clone(),
                host.boot_id.clone(),
                offer.capability_id.clone(),
            )
        })
        .collect();
    let placements = conduit_planner::default_expanded_placements(
        &authored.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let connection_bases = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
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
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    for kind in [DNS_RESOLVE_KIND, NETWORK_CONNECT_KIND] {
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .unwrap();
        assert_eq!(placement.authority.len(), 1);
        assert_eq!(placement.resources.len(), 1);
    }
}

#[test]
fn stale_dns_and_connection_outcomes_are_nominal_and_not_line_or_socket_identity() {
    let (_, result, _) = deterministic_network_fixture();
    assert!(matches!(result, DnsResult::Stale { age_seconds: 31, .. }));
    for value_type in [dns_result_type(), network_connection_state_type()] {
        let bytes = value_type.canonical_bytes().unwrap();
        let rendered = String::from_utf8_lossy(&bytes);
        for forbidden in ["ConnectionBase", "HostId", "LineId", "socket-handle"] {
            assert!(!rendered.contains(forbidden));
        }
        assert!(matches!(value_type.shape(), StructuredInfoTypeShape::Variant { .. }));
    }
}

#[test]
fn offers_keep_dns_and_connect_authority_distinct() {
    let offers = application_network_std_offers();
    let dns = offers.iter().find(|offer| offer.kind_id.as_str() == DNS_RESOLVE_KIND).unwrap();
    let connect = offers.iter().find(|offer| offer.kind_id.as_str() == NETWORK_CONNECT_KIND).unwrap();
    assert_eq!(dns.resource_requirements[0].class_id.as_str(), DNS_RESOLVER_RESOURCE);
    assert_eq!(dns.authority_requirements[0].contract_id.as_str(), DNS_RESOLVE_AUTHORITY);
    assert_eq!(connect.resource_requirements[0].class_id.as_str(), NETWORK_CONNECTION_RESOURCE);
    assert_eq!(connect.authority_requirements[0].contract_id.as_str(), NETWORK_CONNECT_AUTHORITY);
    assert_ne!(dns.authority_requirements[0].contract_id, connect.authority_requirements[0].contract_id);
}

fn host(capabilities: Vec<conduit_core::CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/application-network-proof"),
        boot_id: BootId::from("boot/application-network-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/application-network-proof@1"),
        resources: vec![
            resource_offer("pool/dns-resolver", DNS_RESOLVER_RESOURCE, 1),
            resource_offer("pool/network-connect", NETWORK_CONNECTION_RESOURCE, 1),
        ],
        planner_capabilities: vec![],
        capabilities,
    }
}
