use conduit_core::{BaseImplementationId, PortDirection};
use conduit_net::{
    encode_network_join_request, network_omitting_advertisement, NetworkJoinRequest,
    MAXIMUM_JOIN_INPUT_BYTES, NETWORK_ATTACHMENT_KIND, NETWORK_JOIN_REQUEST_KIND,
};
use conduit_r1_network_conformance::{
    exact_r1_network_bootstrap_plan, R1_JOIN_GRANT_ID, R1_PICO_HOST_ID, R1_STD_HOST_ID,
};

#[test]
fn one_exact_plan_carries_runtime_credentials_over_the_observed_usb_line() {
    let exact = exact_r1_network_bootstrap_plan().unwrap();
    assert_eq!(exact.plan.fragments.len(), 2);
    assert_eq!(
        exact.usb_line.binding.base,
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1")
    );
    assert_eq!(exact.usb_line.binding.limits.maximum_in_flight_items, 1);
    assert_eq!(
        exact.usb_line.binding.limits.maximum_payload_bytes,
        MAXIMUM_JOIN_INPUT_BYTES
    );
    let std = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == R1_STD_HOST_ID)
        .unwrap();
    let pico = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == R1_PICO_HOST_ID)
        .unwrap();
    assert_eq!(std.connections.len(), 1);
    assert_eq!(pico.connections.len(), 2);
    assert_eq!(
        std.connections[0].value_kind.as_str(),
        NETWORK_JOIN_REQUEST_KIND
    );
    assert_eq!(std.connections[0].source_port_id.as_str(), "request");
    let remote = pico
        .connections
        .iter()
        .find(|connection| {
            connection
                .selected_line
                .as_ref()
                .map(|line| line.binding.base.clone())
                == Some(BaseImplementationId::from("conduit.base/usb-cdc-acm@1"))
        })
        .unwrap();
    assert_eq!(remote.sink_port_id.as_str(), "request");
    let attachment = pico
        .connections
        .iter()
        .find(|connection| connection.selected_line.is_none())
        .unwrap();
    assert_eq!(attachment.value_kind.as_str(), NETWORK_ATTACHMENT_KIND);
    assert_eq!(attachment.source_port_id.as_str(), "attachment");
    assert_eq!(attachment.sink_port_id.as_str(), "attachment");
    let join = pico
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_net::NETWORK_JOIN_OPERATION)
        .unwrap();
    assert!(join
        .authority
        .iter()
        .any(|binding| binding.grant_id.as_str() == R1_JOIN_GRANT_ID));
    assert_eq!(
        exact.source_advertisement.capabilities[0].outputs[0].direction,
        PortDirection::Output
    );
}

#[test]
fn secret_bytes_change_runtime_info_but_never_plan_identity() {
    let exact = exact_r1_network_bootstrap_plan().unwrap();
    let plan_id = exact.plan.plan_id.clone();
    let mut first = [0_u8; MAXIMUM_JOIN_INPUT_BYTES as usize];
    let mut second = [0_u8; MAXIMUM_JOIN_INPUT_BYTES as usize];
    let first_len = encode_network_join_request(
        NetworkJoinRequest {
            ssid: b"ordinary-lan",
            credential: b"first-secret",
        },
        &mut first,
    )
    .unwrap();
    let second_len = encode_network_join_request(
        NetworkJoinRequest {
            ssid: b"ordinary-lan",
            credential: b"second-secret",
        },
        &mut second,
    )
    .unwrap();
    assert_ne!(&first[..first_len], &second[..second_len]);
    assert_eq!(
        exact_r1_network_bootstrap_plan().unwrap().plan.plan_id,
        plan_id
    );
    let serialized = serde_json::to_string(&exact.plan).unwrap();
    assert!(!serialized.contains("ordinary-lan"));
    assert!(!serialized.contains("first-secret"));
    assert!(!serialized.contains("second-secret"));
}

#[test]
fn network_free_composition_has_no_wifi_or_join_offer() {
    let omitted = network_omitting_advertisement("r1/pico-minimal", "r1/pico-minimal-boot");
    assert!(omitted.resources.is_empty());
    assert!(omitted.capabilities.is_empty());
}
