use conduit_embedded_build::{generate_embedded_plan, EmbeddedImageBounds};
use conduit_runtime::lowering::{lower_plan_fragment, RemoteCordDirection};
use conduit_signal::{signal_profile_catalog, SIGNAL_ENCODED_LEN};
use conduit_signal_conformance::{
    exact_std_pico_usb_plan, pico_local_advertisement, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
    PICO_LOCAL_HOST_ID, STD_PICO_USB_SINK_HOST_ID,
};

const FORM: &str = include_str!("../../../fixtures/forms/signal-demo.conduit");

#[test]
fn exact_planned_usb_sink_is_the_generated_remote_ingress() {
    let exact = exact_std_pico_usb_plan().expect("exact std-to-Pico plan resolves");
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == STD_PICO_USB_SINK_HOST_ID)
        .expect("Pico sink fragment");
    assert_eq!(sink.startup_dependencies.len(), 1);
    let lowered = lower_plan_fragment(sink).expect("Pico sink lowers");
    assert_eq!(lowered.remote_endpoints.len(), 1);
    assert_eq!(
        lowered.remote_endpoints[0].direction,
        RemoteCordDirection::Ingress
    );
    let generated = generate_embedded_plan(sink, &lowered, EmbeddedImageBounds::HOST_TOOLING)
        .expect("Pico sink generates");
    assert!(generated.startup_dependencies.is_empty());
    let endpoint = &generated.remote_endpoints[0];
    let planned = &exact.line_offer.binding;
    assert_eq!(generated.plan_id, sink.plan_id.as_str());
    assert_eq!(generated.fragment_id, sink.fragment_id.as_str());
    assert_eq!(
        endpoint.connection_id,
        sink.connections[0].connection_id.as_str()
    );
    assert_eq!(
        endpoint.source_fragment_id,
        exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
            .unwrap()
            .fragment_id
            .as_str()
    );
    assert_eq!(endpoint.sink_fragment_id, sink.fragment_id.as_str());
    assert_eq!(endpoint.local_host, planned.sink.host_id.as_str());
    assert_eq!(endpoint.local_boot, planned.sink.boot_id.as_str());
    assert_eq!(endpoint.local_endpoint, planned.sink.endpoint_id.as_str());
    assert_eq!(endpoint.peer_host, planned.source.host_id.as_str());
    assert_eq!(endpoint.peer_boot, planned.source.boot_id.as_str());
    assert_eq!(endpoint.peer_endpoint, planned.source.endpoint_id.as_str());
    assert_eq!(endpoint.base, planned.base);
    assert_eq!(endpoint.base_instance_id, planned.base_instance_id.as_str());
    assert_eq!(endpoint.link_binding_id, planned.binding_id.as_str());
    assert_eq!(endpoint.maximum_in_flight_items, 1);
    assert_eq!(endpoint.maximum_payload_bytes, SIGNAL_ENCODED_LEN);

    let rendered = generated.render_no_alloc_firmware_module();
    for exact_identity in [
        endpoint.connection_id.as_str(),
        endpoint.source_fragment_id.as_str(),
        endpoint.sink_fragment_id.as_str(),
        endpoint.link_binding_id.as_str(),
        endpoint.base_instance_id.as_str(),
        endpoint.local_endpoint.as_str(),
        endpoint.peer_endpoint.as_str(),
    ] {
        assert!(rendered.contains(exact_identity));
    }
}

#[test]
fn local_image_cannot_masquerade_as_the_remote_usb_sink() {
    let form = conduit_form::parse_with_startup(
        FORM,
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("form checks");
    let host = pico_local_advertisement();
    let placements = conduit_planner::default_placements(&form, core::slice::from_ref(&host))
        .expect("local placements");
    let plan = conduit_planner::plan_with_connection_limits(
        &form,
        core::slice::from_ref(&host),
        &placements,
        &[conduit_core::ConnectionBase::Local],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
    )
    .expect("local plan");
    let local = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == PICO_LOCAL_HOST_ID)
        .expect("local fragment");
    let lowered = lower_plan_fragment(local).expect("local fragment lowers");
    assert!(lowered.remote_endpoints.is_empty());
    assert_ne!(
        local.plan_id,
        exact_std_pico_usb_plan().unwrap().plan.plan_id
    );
}
