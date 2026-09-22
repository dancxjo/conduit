use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

const SOURCE: &str = include_str!("../../../forms/pool-webchat/main.conduit");

#[test]
fn shared_pool_offers_keep_exact_bounded_semantics() {
    let offers = conduit_chat::pool_chat_capabilities();
    assert_eq!(offers.len(), 4);
    assert_eq!(
        offers[0].limits.max_active_instances,
        conduit_chat::POOL_WEBCHAT_MAXIMUM_PEERS
    );
    for offer in &offers[1..] {
        assert_eq!(offer.limits.max_active_instances, 1);
        assert_eq!(offer.limits.max_queue_items, 32);
        assert_eq!(offer.limits.max_queue_bytes, 8_192);
    }
    assert!(offers.iter().all(|offer| {
        offer.host_calls.is_empty()
            && offer.resource_requirements.is_empty()
            && offer.authority_requirements.is_empty()
    }));
}

#[test]
fn native_pool_chat_has_explicit_fan_merge_and_no_authored_line() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_chat::install_pool_chat_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "pool-webchat", &profile).unwrap();
    assert_eq!(expanded.shared_pools.len(), 1);
    assert_eq!(expanded.shared_pools[0].maximum_members, 32);
    assert_eq!(expanded.shared_pools[0].consumers.len(), 3);
    assert!(expanded
        .gears
        .iter()
        .any(|operation| operation.kind_id.as_str() == conduit_chat::FLOW_FAN_KIND));
    assert!(expanded
        .gears
        .iter()
        .any(|operation| operation.kind_id.as_str() == conduit_chat::FLOW_MERGE_KIND));
    for forbidden in ["WebSocket", "websocket", "net/", "socket", "address"] {
        assert!(
            !SOURCE.contains(forbidden),
            "authored line fact: {forbidden}"
        );
    }
}
