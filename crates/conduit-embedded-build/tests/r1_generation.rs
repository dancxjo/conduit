use conduit_core::{BootId, ConnectionBase};
use conduit_embedded_build::{generate_embedded_plan, EmbeddedImageBounds};
use conduit_runtime::lowering::lower_plan_fragment;
use conduit_system_continuity::{exact_r1_signal_plan, R1SignalRouteSet};

#[test]
fn current_r1_single_route_plans_generate_exact_usb_and_websocket_ingress() {
    for (routes, expected_base, expected_binding) in [
        (
            R1SignalRouteSet::WebSocketOnly,
            ConnectionBase::WebSocket,
            conduit_net::R1_WEBSOCKET_LINK_BINDING_ID,
        ),
        (
            R1SignalRouteSet::UsbOnly,
            ConnectionBase::UsbCdc,
            conduit_net::R1_USB_LINK_BINDING_ID,
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

        assert_eq!(generated.remote_endpoints.len(), 1);
        assert_eq!(generated.remote_endpoints[0].base, expected_base);
        assert_eq!(
            generated.remote_endpoints[0].link_binding_id,
            expected_binding
        );
    }
}
