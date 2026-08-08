use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

const SOURCE: &str = include_str!("../../../examples/webchat.conduit");

#[test]
fn canonical_browser_chat_expands_to_three_exact_bounded_leaves() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile).unwrap();
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "webchat-browser-demo", &profile).unwrap();
    assert_eq!(expanded.operations.len(), 3);
    assert_eq!(expanded.connections.len(), 2);
    for kind in [
        conduit_chat::WEB_TEXT_INPUT_KIND,
        conduit_net::EXTERNAL_WEBSOCKET_CLIENT_KIND,
        conduit_chat::WEB_LIST_KIND,
    ] {
        assert!(expanded
            .operations
            .iter()
            .any(|operation| operation.kind_id.as_str() == kind));
    }
    assert!(expanded.connections.iter().all(|connection| {
        connection.temporal == conduit_core::PortTemporal::Flow { closes: true }
    }));
}

#[test]
fn browser_chat_family_is_finite_and_does_not_log_or_store_credentials() {
    let family = conduit_chat::browser_chat_family();
    assert_eq!(family.resource.capacity_units, 2);
    for offer in family.capabilities {
        assert!(offer.limits.max_queue_items > 0);
        assert!(offer.limits.max_queue_bytes > 0);
        assert!(offer.limits.max_queue_items <= conduit_chat::MAXIMUM_CHAT_HISTORY_ITEMS);
        assert!(offer.authority_requirements.is_empty());
    }
}
