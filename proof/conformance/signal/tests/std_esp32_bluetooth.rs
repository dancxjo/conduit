#![cfg(feature = "host-profile")]

use conduit_core::{BaseImplementationId, BootId};
use conduit_signal_conformance::{
    exact_std_esp32_bluetooth_plan, exact_std_esp32_c3_bluetooth_plan,
    exact_std_esp32_s3_bluetooth_plan, std_esp32_bluetooth_session_binding,
    std_esp32_bluetooth_session_binding_for_host, ESP32_C3_IMAGE_BOOT_ID,
    ESP32_C3_PHYSICAL_HOST_ID, ESP32_S3_IMAGE_BOOT_ID, ESP32_S3_PHYSICAL_HOST_ID,
    ESP32_WROOM_IMAGE_BOOT_ID, ESP32_WROOM_PHYSICAL_HOST_ID, STD_ESP32_BLUETOOTH_BASE_INSTANCE_ID,
    STD_PICO_USB_SOURCE_HOST_ID,
};

#[test]
fn unchanged_signal_form_seals_one_exact_esp32_bluetooth_fragment() {
    let exact = exact_std_esp32_bluetooth_plan([1, 2, 3, 4, 5, 6]).unwrap();
    let source = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == STD_PICO_USB_SOURCE_HOST_ID)
        .unwrap();
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == ESP32_WROOM_PHYSICAL_HOST_ID)
        .unwrap();
    let cord = &sink.connections[0];
    let line = cord.selected_line.as_ref().unwrap();

    assert_eq!(exact.plan.fragments.len(), 2);
    assert_eq!(source.connections.len(), 1);
    assert_eq!(source.connections[0].connection_id, cord.connection_id);
    assert_eq!(sink.boot_id.as_str(), ESP32_WROOM_IMAGE_BOOT_ID);
    assert_eq!(sink.connections.len(), 1);
    assert_eq!(
        line.binding.base,
        BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
    );
    assert_eq!(
        line.binding.base_instance_id.as_str(),
        STD_ESP32_BLUETOOTH_BASE_INSTANCE_ID
    );
}

#[test]
fn inspected_s3_gets_its_own_exact_plan_and_runtime_binding() {
    let exact = exact_std_esp32_s3_bluetooth_plan([1, 2, 3, 4, 5, 6]).unwrap();
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == ESP32_S3_PHYSICAL_HOST_ID)
        .unwrap();
    assert_eq!(sink.boot_id.as_str(), ESP32_S3_IMAGE_BOOT_ID);
    assert_ne!(
        exact.plan.plan_id,
        exact_std_esp32_bluetooth_plan([1, 2, 3, 4, 5, 6])
            .unwrap()
            .plan
            .plan_id
    );

    let runtime = std_esp32_bluetooth_session_binding_for_host(
        ESP32_S3_PHYSICAL_HOST_ID,
        BootId::from("esp32/s3/runtime-boot"),
    )
    .unwrap();
    assert_eq!(runtime.sink.host_id.as_str(), ESP32_S3_PHYSICAL_HOST_ID);
    assert_eq!(runtime.sink.boot_id.as_str(), "esp32/s3/runtime-boot");
}

#[test]
fn inspected_c3_gets_its_own_exact_plan_and_runtime_binding() {
    let exact = exact_std_esp32_c3_bluetooth_plan([1, 2, 3, 4, 5, 6]).unwrap();
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == ESP32_C3_PHYSICAL_HOST_ID)
        .unwrap();
    assert_eq!(sink.boot_id.as_str(), ESP32_C3_IMAGE_BOOT_ID);
    assert_ne!(
        exact.plan.plan_id,
        exact_std_esp32_bluetooth_plan([1, 2, 3, 4, 5, 6])
            .unwrap()
            .plan
            .plan_id
    );

    let runtime = std_esp32_bluetooth_session_binding_for_host(
        ESP32_C3_PHYSICAL_HOST_ID,
        BootId::from("esp32/c3/runtime-boot"),
    )
    .unwrap();
    assert_eq!(runtime.sink.host_id.as_str(), ESP32_C3_PHYSICAL_HOST_ID);
    assert_eq!(runtime.sink.boot_id.as_str(), "esp32/c3/runtime-boot");
}

#[test]
fn observed_boot_changes_only_runtime_boot_bound_identity() {
    let planned =
        std_esp32_bluetooth_session_binding(BootId::from(ESP32_WROOM_IMAGE_BOOT_ID)).unwrap();
    let runtime =
        std_esp32_bluetooth_session_binding(BootId::from("esp32/runtime-boot-7")).unwrap();

    assert_eq!(runtime.plan_id, planned.plan_id);
    assert_eq!(runtime.source_fragment_id, planned.source_fragment_id);
    assert_eq!(runtime.sink_fragment_id, planned.sink_fragment_id);
    assert_eq!(runtime.connection_id, planned.connection_id);
    assert_eq!(runtime.source, planned.source);
    assert_eq!(runtime.sink.host_id, planned.sink.host_id);
    assert_eq!(runtime.value_kind, planned.value_kind);
    assert_eq!(runtime.limits, planned.limits);
    assert_eq!(runtime.attachment.line_id, planned.attachment.line_id);
    assert_eq!(
        runtime.attachment.link_binding_id,
        planned.attachment.link_binding_id
    );
    assert_eq!(runtime.attachment.base, planned.attachment.base);
    assert_eq!(
        runtime.attachment.base_instance_id,
        planned.attachment.base_instance_id
    );
    assert_eq!(
        runtime.attachment.source_endpoint_id,
        planned.attachment.source_endpoint_id
    );
    assert_eq!(
        runtime.attachment.sink_endpoint_id,
        planned.attachment.sink_endpoint_id
    );

    assert_eq!(runtime.sink.boot_id.as_str(), "esp32/runtime-boot-7");
    assert_eq!(
        runtime.attachment.sink_boot_id.as_str(),
        "esp32/runtime-boot-7"
    );
    assert_ne!(runtime.sink_active_play_id, planned.sink_active_play_id);
}

#[test]
fn observed_radio_address_does_not_mutate_exact_plan_identity() {
    let first = exact_std_esp32_bluetooth_plan([1, 2, 3, 4, 5, 6]).unwrap();
    let replacement_address = exact_std_esp32_bluetooth_plan([6, 5, 4, 3, 2, 1]).unwrap();

    assert_eq!(first.plan, replacement_address.plan);
    assert_eq!(first.line_offer, replacement_address.line_offer);
}

#[test]
fn exact_session_reply_packet_bound_is_finite() {
    let binding =
        std_esp32_bluetooth_session_binding(BootId::from("0123456789abcdef0123456789abcdef"))
            .unwrap();
    let mut encoded = [0_u8; 2_048];
    let hello =
        conduit_wire::encode_session_frame_into(binding.hello_frame(), &mut encoded, 96, 2_048)
            .unwrap();
    let profile = conduit_bluetooth::BleGattProfile::FIRST;
    let hello_packets = conduit_bluetooth::fragment_count(hello, profile).unwrap();
    let mut delivery_packets = 0;
    for message in [
        conduit_wire::SessionMessage::Accepted { sequence: 0 },
        conduit_wire::SessionMessage::Delivered { sequence: 0 },
    ] {
        let length = conduit_wire::encode_session_frame_into(
            binding.frame(message),
            &mut encoded,
            96,
            2_048,
        )
        .unwrap();
        delivery_packets += conduit_bluetooth::fragment_count(length, profile).unwrap();
    }
    assert!(hello_packets <= 8);
    assert!(delivery_packets <= 8);
}
