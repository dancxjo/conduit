use conduit_core::{BootId, ConnectionBase};
use conduit_embedded_build::{generate_embedded_plan, EmbeddedImageBounds};
use conduit_runtime::lowering::lower_plan_fragment;
use conduit_system_continuity::{exact_r1_signal_plan, R1SignalRouteSet};

#[test]
fn current_r1_plans_generate_exact_single_and_dual_line_ingress() {
    for (routes, expected) in [
        (
            R1SignalRouteSet::WebSocketOnly,
            vec![(
                ConnectionBase::WebSocket,
                conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
            )],
        ),
        (
            R1SignalRouteSet::UsbOnly,
            vec![(ConnectionBase::UsbCdc, conduit_net::R1_USB_LINK_BINDING_ID)],
        ),
        (
            R1SignalRouteSet::WebSocketThenUsb,
            vec![
                (
                    ConnectionBase::WebSocket,
                    conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
                ),
                (ConnectionBase::UsbCdc, conduit_net::R1_USB_LINK_BINDING_ID),
            ],
        ),
    ] {
        let exact = exact_r1_signal_plan(BootId::from(conduit_net::R1_PICO_BOOT_ID), routes)
            .expect("exact R1 Signal Plan");
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id.as_str() == conduit_net::R1_PICO_HOST_ID)
            .expect("R1 Pico fragment");
        let lowered = lower_plan_fragment(fragment).expect("R1 Pico fragment lowers");
        let generated =
            generate_embedded_plan(fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING)
                .expect("current single-route R1 fragment generates");

        let generated_endpoints: Vec<_> = generated
            .remote_endpoints
            .iter()
            .map(|endpoint| (endpoint.base, endpoint.link_binding_id.as_str()))
            .collect();
        assert_eq!(generated_endpoints, expected);
    }
}
