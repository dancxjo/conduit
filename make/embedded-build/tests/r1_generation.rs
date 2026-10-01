use conduit_core::{BaseImplementationId, BootId};
use conduit_embedded_build::{generate_embedded_plan, EmbeddedImageBounds};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_r1_network_conformance::{
    exact_r1_control_plan, exact_r1_signal_plan, R1SignalRouteSet,
};

#[test]
fn current_r1_plans_generate_exact_single_and_dual_line_ingress() {
    for (routes, expected) in [
        (
            R1SignalRouteSet::WebSocketOnly,
            vec![(
                BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
                conduit_r1_network_conformance::R1_WEBSOCKET_LINK_BINDING_ID,
            )],
        ),
        (
            R1SignalRouteSet::UsbOnly,
            vec![(
                BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
                conduit_r1_network_conformance::R1_USB_LINK_BINDING_ID,
            )],
        ),
        (
            R1SignalRouteSet::WebSocketThenUsb,
            vec![
                (
                    BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
                    conduit_r1_network_conformance::R1_WEBSOCKET_LINK_BINDING_ID,
                ),
                (
                    BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
                    conduit_r1_network_conformance::R1_USB_LINK_BINDING_ID,
                ),
            ],
        ),
    ] {
        let exact = exact_r1_signal_plan(
            BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
            routes,
        )
        .expect("exact R1 Signal Plan");
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id.as_str() == conduit_r1_network_conformance::R1_PICO_HOST_ID
            })
            .expect("R1 Pico fragment");
        let lowered = lower_plan_fragment(fragment).expect("R1 Pico fragment lowers");
        let generated =
            generate_embedded_plan(fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING)
                .expect("current single-route R1 fragment generates");

        let generated_endpoints: Vec<_> = generated
            .remote_endpoints
            .iter()
            .map(|endpoint| (endpoint.base.clone(), endpoint.link_binding_id.as_str()))
            .collect();
        assert_eq!(generated_endpoints, expected);
    }
}

#[test]
fn three_peer_control_plans_generate_the_same_exact_pico_ingress_family() {
    for (routes, expected_count) in [
        (R1SignalRouteSet::WebSocketOnly, 1),
        (R1SignalRouteSet::UsbOnly, 1),
        (R1SignalRouteSet::WebSocketThenUsb, 2),
    ] {
        let exact = exact_r1_control_plan(
            BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
            routes,
        )
        .expect("exact R1 three-peer control Plan");
        let source = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id.as_str() == conduit_r1_network_conformance::R1_STD_HOST_ID
            })
            .expect("R1 control source fragment");
        assert_eq!(source.placements.len(), 4);
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| {
                fragment.host_id.as_str() == conduit_r1_network_conformance::R1_PICO_HOST_ID
            })
            .expect("R1 control Pico fragment");
        let lowered = lower_plan_fragment(fragment).expect("R1 control Pico fragment lowers");
        let generated =
            generate_embedded_plan(fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING)
                .expect("R1 control Pico ingress generates");
        assert_eq!(generated.nodes.len(), 1);
        assert_eq!(generated.remote_endpoints.len(), expected_count);
        assert!(generated
            .remote_endpoints
            .iter()
            .all(|endpoint| endpoint.value_kind.as_str() == conduit_signal::SIGNAL_VALUE_KIND));
    }
}
