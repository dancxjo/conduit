#![cfg(feature = "host-profile")]

use conduit_core::BaseImplementationId;
use conduit_signal_conformance::{
    exact_std_pico_bluetooth_plan, exact_std_pico_usb_plan, std_pico_bluetooth_session_binding,
    STD_PICO_BLUETOOTH_BASE_INSTANCE_ID, STD_PICO_USB_SINK_HOST_ID,
};

#[test]
fn unchanged_signal_form_seals_one_exact_bluetooth_pico_fragment() {
    let exact = exact_std_pico_bluetooth_plan([1, 2, 3, 4, 5, 6]).unwrap();
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == STD_PICO_USB_SINK_HOST_ID)
        .unwrap();
    let line = sink.connections[0].selected_line.as_ref().unwrap();
    assert_eq!(
        line.binding.base,
        BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
    );
    assert_eq!(
        line.binding.base_instance_id.as_str(),
        STD_PICO_BLUETOOTH_BASE_INSTANCE_ID
    );

    let binding = std_pico_bluetooth_session_binding().unwrap();
    assert_eq!(binding.plan_id, exact.plan.plan_id);
    assert_eq!(
        binding.attachment.base,
        BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
    );
    assert_eq!(binding.sink.host_id, sink.host_id);
}

#[test]
fn unchanged_form_can_receive_a_fresh_usb_realization_after_bluetooth_loss() {
    let bluetooth = exact_std_pico_bluetooth_plan([1, 2, 3, 4, 5, 6]).unwrap();
    let usb = exact_std_pico_usb_plan().unwrap();

    assert_eq!(
        bluetooth.plan.source_document_id,
        usb.plan.source_document_id
    );
    assert_eq!(bluetooth.plan.checked_form_id, usb.plan.checked_form_id);
    assert_eq!(bluetooth.plan.expanded_form_id, usb.plan.expanded_form_id);
    assert_ne!(bluetooth.plan.plan_id, usb.plan.plan_id);

    let bluetooth_cord = &bluetooth
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == STD_PICO_USB_SINK_HOST_ID)
        .unwrap()
        .connections[0];
    let usb_cord = &usb
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == STD_PICO_USB_SINK_HOST_ID)
        .unwrap()
        .connections[0];
    assert_eq!(bluetooth_cord.source_port_id, usb_cord.source_port_id);
    assert_eq!(bluetooth_cord.sink_port_id, usb_cord.sink_port_id);
    assert_eq!(bluetooth_cord.value_kind, usb_cord.value_kind);
    assert_ne!(
        bluetooth_cord.selected_line.as_ref().unwrap().line_id,
        usb_cord.selected_line.as_ref().unwrap().line_id
    );
    assert_eq!(
        bluetooth_cord.selected_line.as_ref().unwrap().binding.base,
        BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
    );
    assert_eq!(
        usb_cord.selected_line.as_ref().unwrap().binding.base,
        BaseImplementationId::from("conduit.base/usb-cdc-acm@1")
    );
}

#[test]
fn observed_radio_address_does_not_mutate_exact_plan_identity() {
    let first = exact_std_pico_bluetooth_plan([1, 2, 3, 4, 5, 6]).unwrap();
    let replacement_address = exact_std_pico_bluetooth_plan([6, 5, 4, 3, 2, 1]).unwrap();

    assert_eq!(first.plan, replacement_address.plan);
    assert_eq!(first.line_offer, replacement_address.line_offer);
}
