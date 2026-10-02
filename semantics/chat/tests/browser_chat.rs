use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};

const SOURCE: &str = include_str!("../../../plots/webchat/main.conduit");

#[test]
fn canonical_browser_chat_accepts_face_interaction_at_its_fore() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile).unwrap();
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile).unwrap();
    let syntax = parse_syntax_document(SOURCE);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "chat/browser-client", &profile).unwrap();
    let expanded = authoring.expanded;
    assert_eq!(expanded.gears.len(), 6);
    assert_eq!(expanded.connections.len(), 6);
    for kind in [
        conduit_chat::CHAT_STATE_KIND,
        conduit_chat::CHAT_SUBMIT_KIND,
        conduit_chat::CHAT_FROM_WEBSOCKET_KIND,
        conduit_chat::CHAT_TO_WEBSOCKET_KIND,
        conduit_chat::CHAT_CONNECTION_FROM_WEBSOCKET_KIND,
        conduit_net::EXTERNAL_WEBSOCKET_CLIENT_KIND,
    ] {
        assert!(expanded
            .gears
            .iter()
            .any(|operation| operation.kind_id.as_str() == kind));
    }
    assert_eq!(authoring.input_bindings.len(), 1);
    assert_eq!(authoring.output_bindings.len(), 1);
    assert!(!SOURCE.contains("web/text-input"));
    assert!(!SOURCE.contains("web/list"));
}

#[test]
fn browser_chat_family_is_finite_and_does_not_log_or_store_credentials() {
    let family = conduit_chat::browser_chat_family();
    assert!(family.resources.is_empty());
    for offer in family.capabilities {
        assert!(offer.limits.max_queue_items > 0);
        assert!(offer.limits.max_queue_bytes > 0);
        assert!(offer.authority_requirements.is_empty());
    }
}

#[test]
fn browser_shell_and_adapter_do_not_own_chat_controls_or_policy() {
    let html = include_str!("../../../proof/browser/webchat.test.html");
    let javascript = include_str!("../../../proof/browser/webchat-runtime.mjs");
    for forbidden in ["id=\"history\"", "id=\"message\"", "id=\"send\""] {
        assert!(!html.contains(forbidden));
    }
    for forbidden in [
        "MAXIMUM_MESSAGE_BYTES",
        "MAXIMUM_HISTORY_ITEMS",
        "hello from A",
        "Chat history",
        ">Send<",
    ] {
        assert!(!javascript.contains(forbidden));
    }
    assert!(javascript.contains("subject.role"));
    assert!(javascript.contains("action.arguments"));
    assert!(javascript.contains("presentation.actions"));
}

#[test]
fn application_plan_needs_no_human_input_back_because_the_mask_owns_it() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile).unwrap();
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(SOURCE), &startup).unwrap();
    let authoring =
        expand_canonical_plot_for_authoring(&checked, "chat/browser-client", &profile).unwrap();
    let expanded = authoring.expanded;
    let socket = conduit_net::browser_external_websocket_family();
    let chat = conduit_chat::browser_chat_family();
    let mut resources = vec![socket.resource];
    resources.extend(chat.resources);
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let mut capabilities = vec![socket.capability];
    capabilities.extend(chat.capabilities);
    let host = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: conduit_core::HostId::from("browser/output-only"),
        boot_id: conduit_core::BootId::from("browser/output-only-boot"),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: conduit_core::HostProfileId::from("browser/output-only@1"),
        bases: vec![],
        resources,
        capabilities,
        planner_capabilities: vec![],
    };
    conduit_planner::default_expanded_placements(&expanded, &[host]).unwrap();
}
