use conduit_core::{
    authority_grant, kind_id, BootId, ConnectionBase, HostAdvertisement, HostId, HostProfileId,
    OfferGeneration, DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    install_reminder_catalogs, reminder_std_offers, REMINDER_DELIVERY_AUTHORITY,
    REMINDER_DELIVER_KIND, REMINDER_DELIVER_OPERATION,
};

const SOURCE: &str = include_str!("../../../examples/scheduled-reminder.conduit");

#[test]
fn reminder_delivery_is_bounded_and_requires_ordinary_planned_authority() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_reminder_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        expand_canonical_form_for_authoring(&checked, "scheduled-reminder", &profile).unwrap();
    let host = host(reminder_std_offers());
    let placements = conduit_planner::default_expanded_placements(
        &expanded.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let connection_bases = std::collections::BTreeMap::new();
    let line_candidates = std::collections::BTreeMap::new();
    assert!(conduit_planner::plan_expanded_canonical_with_options(
        &expanded.expanded,
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
    )
    .is_err());

    let offer = host
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == REMINDER_DELIVER_KIND)
        .unwrap();
    assert_eq!(offer.limits.max_active_instances, 4);
    assert_eq!(offer.limits.max_queue_items, 4);
    assert_eq!(offer.host_operations[0].maximum_in_flight, 1);
    let grant = authority_grant(
        "grant/reminder-delivery",
        &offer.authority_requirements[0],
        host.host_id.clone(),
        host.boot_id.clone(),
        offer.capability_id.clone(),
    );
    let grants = [grant];
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &expanded.expanded,
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
    let delivery = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| placement.kind_id == kind_id(REMINDER_DELIVER_KIND))
        .unwrap();
    assert_eq!(delivery.authority.len(), 1);
    assert_eq!(
        delivery.authority[0].contract_id.as_str(),
        REMINDER_DELIVERY_AUTHORITY
    );
    assert_eq!(
        delivery.authority[0].host_operation_contract_id.as_str(),
        REMINDER_DELIVER_OPERATION
    );
}

fn host(capabilities: Vec<conduit_core::CapabilityOffer>) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/reminder-proof"),
        boot_id: BootId::from("boot/reminder-proof"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/reminder-proof@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }
}
