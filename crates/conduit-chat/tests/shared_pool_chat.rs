use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};

const SOURCE: &str = include_str!("../../../examples/pool-webchat.conduit");

#[test]
fn native_pool_chat_has_explicit_fan_merge_and_no_authored_carrier() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_chat::install_pool_chat_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "pool-webchat", &profile).unwrap();
    assert_eq!(expanded.shared_pools.len(), 1);
    assert_eq!(expanded.shared_pools[0].maximum_members, 32);
    assert_eq!(expanded.shared_pools[0].consumers.len(), 3);
    assert!(expanded
        .operations
        .iter()
        .any(|operation| operation.kind_id.as_str() == conduit_chat::FLOW_FAN_KIND));
    assert!(expanded
        .operations
        .iter()
        .any(|operation| operation.kind_id.as_str() == conduit_chat::FLOW_MERGE_KIND));
    for forbidden in ["WebSocket", "websocket", "net/", "socket", "address"] {
        assert!(
            !SOURCE.contains(forbidden),
            "authored carrier fact: {forbidden}"
        );
    }
}
