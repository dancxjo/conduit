use conduit_core::{ArtifactId, BaseImplementationId, CapabilityId, ImplementationId};
use conduit_net::{
    browser_external_websocket_family, external_websocket_client_offer,
    external_websocket_listener_offer, std_external_websocket_family,
    EXTERNAL_WEBSOCKET_CLIENT_CLOSE_HOST_OPERATION, EXTERNAL_WEBSOCKET_CLIENT_KIND,
    EXTERNAL_WEBSOCKET_CLIENT_OPEN_HOST_OPERATION,
    EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_OPERATION,
    EXTERNAL_WEBSOCKET_CLIENT_SEND_HOST_OPERATION,
    EXTERNAL_WEBSOCKET_LISTENER_ACCEPT_HOST_OPERATION, EXTERNAL_WEBSOCKET_LISTENER_KIND,
    EXTERNAL_WEBSOCKET_LISTENER_RECEIVE_HOST_OPERATION,
    EXTERNAL_WEBSOCKET_LISTENER_SEND_HOST_OPERATION, MAXIMUM_EXTERNAL_WEBSOCKET_PEERS,
    MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_BYTES, MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_ITEMS,
};

const CLIENT_SOURCE: &str = include_str!("../../../forms/socket-client/main.conduit");

fn client() -> conduit_core::CapabilityOffer {
    external_websocket_client_offer(
        CapabilityId::from("test/client"),
        ImplementationId::from("test/client-implementation"),
        ArtifactId::from("test/client-artifact"),
    )
}

fn listener() -> conduit_core::CapabilityOffer {
    external_websocket_listener_offer(
        CapabilityId::from("test/listener"),
        ImplementationId::from("test/listener-implementation"),
        ArtifactId::from("test/listener-artifact"),
    )
}

#[test]
fn external_websocket_faces_are_exact_finite_and_host_specific() {
    let client = client();
    let listener = listener();
    assert_eq!(client.kind_id.as_str(), EXTERNAL_WEBSOCKET_CLIENT_KIND);
    assert_eq!(listener.kind_id.as_str(), EXTERNAL_WEBSOCKET_LISTENER_KIND);
    assert_eq!(client.startup_parameters[0].name, "url");
    assert_eq!(listener.startup_parameters[0].name, "bind");
    assert_eq!(client.limits.max_active_instances, 1);
    assert_eq!(
        listener.limits.max_active_instances,
        MAXIMUM_EXTERNAL_WEBSOCKET_PEERS
    );
    for offer in [&client, &listener] {
        assert_eq!(
            offer.limits.max_queue_items,
            MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_ITEMS
        );
        assert_eq!(
            offer.limits.max_queue_bytes,
            MAXIMUM_EXTERNAL_WEBSOCKET_QUEUE_BYTES
        );
        assert!(offer
            .host_operations
            .iter()
            .all(|operation| operation.maximum_in_flight == 1));
    }
    assert_eq!(
        client.host_operations[0].contract_id.as_str(),
        EXTERNAL_WEBSOCKET_CLIENT_CLOSE_HOST_OPERATION
    );
    assert_eq!(
        client.host_operations[1].contract_id.as_str(),
        EXTERNAL_WEBSOCKET_CLIENT_OPEN_HOST_OPERATION
    );
    assert_eq!(
        client.host_operations[2].contract_id.as_str(),
        EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_OPERATION
    );
    assert_eq!(
        client.host_operations[3].contract_id.as_str(),
        EXTERNAL_WEBSOCKET_CLIENT_SEND_HOST_OPERATION
    );
    assert_eq!(
        listener.host_operations[0].contract_id.as_str(),
        EXTERNAL_WEBSOCKET_LISTENER_ACCEPT_HOST_OPERATION
    );
    assert_eq!(
        listener.host_operations[1].contract_id.as_str(),
        EXTERNAL_WEBSOCKET_LISTENER_RECEIVE_HOST_OPERATION
    );
    assert_eq!(
        listener.host_operations[2].contract_id.as_str(),
        EXTERNAL_WEBSOCKET_LISTENER_SEND_HOST_OPERATION
    );

    let browser = browser_external_websocket_family();
    let std = std_external_websocket_family();
    assert_eq!(browser.capability.checked_face(), client.checked_face());
    assert_eq!(std.capability.checked_face(), listener.checked_face());
    assert_eq!(browser.resource.capacity_units, 1);
    assert_eq!(std.resource.capacity_units, 1);
    assert_eq!(
        browser.resource.class_id,
        browser.capability.resource_requirements[0].class_id
    );
    assert_eq!(
        std.resource.class_id,
        std.capability.resource_requirements[0].class_id
    );
}

#[test]
fn external_websocket_compatibility_is_the_checked_face_not_the_nominal_kind() {
    let client = client();
    let mut renamed = client.clone();
    renamed.kind_id = conduit_core::kind_id("example/renamed-websocket-client");
    renamed.kind_contract_revision =
        conduit_core::KindContractRevision::from("example/renamed-websocket-client@9");
    assert_eq!(renamed.checked_face(), client.checked_face());

    let mut generic_duplex_bytes = client.clone();
    generic_duplex_bytes.inputs[0].value_kind = conduit_core::kind_id("Bytes");
    generic_duplex_bytes.outputs[0].value_kind = conduit_core::kind_id("Bytes");
    assert_eq!(
        generic_duplex_bytes.kind_id.as_str(),
        EXTERNAL_WEBSOCKET_CLIENT_KIND
    );
    assert_ne!(generic_duplex_bytes.checked_face(), client.checked_face());
}

#[test]
fn authored_external_socket_cannot_masquerade_as_a_conduit_session_line() {
    for line in [
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
    ] {
        assert_ne!(client().kind_id.as_str(), format!("{line:?}"));
        assert_ne!(listener().kind_id.as_str(), format!("{line:?}"));
    }
    assert_eq!(
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1").as_str(),
        "conduit.base/websocket-rfc6455@1"
    );
    assert!(!client().kind_id.as_str().contains("BaseImplementationId"));
}

#[cfg(feature = "form-catalog")]
#[test]
fn canonical_duplex_source_checks_and_expands_to_the_external_client_leaf() {
    let syntax = conduit_form::parse_syntax_document(CLIENT_SOURCE);
    assert_eq!(syntax.round_trip(), CLIENT_SOURCE);
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_net::install_external_websocket_catalogs(&mut startup, &mut profile).unwrap();
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "echo-client-demo", &profile).unwrap();
    assert_eq!(expanded.gears.len(), 1);
    let socket = &expanded.gears[0];
    assert_eq!(socket.kind_id.as_str(), EXTERNAL_WEBSOCKET_CLIENT_KIND);
    assert_eq!(socket.inputs, client().inputs);
    assert_eq!(socket.outputs, client().outputs);
    assert_eq!(socket.configuration[0].key, "url");
}
