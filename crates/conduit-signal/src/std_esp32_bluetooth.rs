//! Exact std-to-ESP32 Signal plan over the first finite BLE GATT profile.

use alloc::collections::BTreeMap;

use conduit_bluetooth::{
    offer_ble_gatt_line, BleGattLineIdentity, BleGattProfile, BluetoothAddressKind,
    BluetoothControllerId, BluetoothDiscoveryCandidate, BluetoothLineObservation,
    BluetoothLineState, BluetoothPairingState, NegotiatedPeerIdentity,
};
use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, GearId, HostId, LineId,
    LinkAuthorityReference, LinkCredentialReference, Plan, SignId,
};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_wire::{LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits};

use crate::{
    esp32_c3_build_fixture_advertisement, esp32_s3_build_fixture_advertisement,
    esp32_wroom_build_fixture_advertisement, signal_profile_catalog,
    std_pico_usb_source_advertisement, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN,
    STD_PICO_USB_SOURCE_HOST_ID,
};

pub const ESP32_WROOM_PHYSICAL_HOST_ID: &str = "esp32/24dcc39a0a44";
pub const ESP32_WROOM_IMAGE_BOOT_ID: &str = "esp32/wroom/image-boot";
pub const ESP32_C3_PHYSICAL_HOST_ID: &str = "esp32/c04e30ee5ca8";
pub const ESP32_C3_IMAGE_BOOT_ID: &str = "esp32/c3/image-boot";
pub const ESP32_S3_PHYSICAL_HOST_ID: &str = "esp32/c04e30371ab8";
pub const ESP32_S3_IMAGE_BOOT_ID: &str = "esp32/s3/image-boot";
pub const STD_ESP32_BLUETOOTH_LINE_ID: &str = "bluetooth/line/std-esp32-gatt";
pub const STD_ESP32_BLUETOOTH_BINDING_ID: &str = "bluetooth/binding/std-esp32-gatt";
pub const STD_ESP32_BLUETOOTH_BASE_INSTANCE_ID: &str = "esp32/controller/ble-session-0";
pub const STD_ESP32_BLUETOOTH_SOURCE_ENDPOINT_ID: &str = "bluetooth/std-source-write";
pub const STD_ESP32_BLUETOOTH_SINK_ENDPOINT_ID: &str = "bluetooth/esp32-sink-notify";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactStdEsp32BluetoothPlan {
    pub plan: Plan,
    pub line_offer: conduit_core::LineOffer,
}

pub fn exact_std_esp32_bluetooth_plan(
    peer_address: [u8; 6],
) -> Result<ExactStdEsp32BluetoothPlan, alloc::string::String> {
    exact_std_esp32_bluetooth_plan_for_host(peer_address, ESP32_WROOM_PHYSICAL_HOST_ID)
}

pub fn exact_std_esp32_s3_bluetooth_plan(
    peer_address: [u8; 6],
) -> Result<ExactStdEsp32BluetoothPlan, alloc::string::String> {
    exact_std_esp32_bluetooth_plan_for_host(peer_address, ESP32_S3_PHYSICAL_HOST_ID)
}

pub fn exact_std_esp32_c3_bluetooth_plan(
    peer_address: [u8; 6],
) -> Result<ExactStdEsp32BluetoothPlan, alloc::string::String> {
    exact_std_esp32_bluetooth_plan_for_host(peer_address, ESP32_C3_PHYSICAL_HOST_ID)
}

pub fn exact_std_esp32_bluetooth_plan_for_host(
    peer_address: [u8; 6],
    physical_host_id: &str,
) -> Result<ExactStdEsp32BluetoothPlan, alloc::string::String> {
    let source = std_pico_usb_source_advertisement();
    let (mut sink, image_boot_id, capability_id) = match physical_host_id {
        ESP32_WROOM_PHYSICAL_HOST_ID => (
            esp32_wroom_build_fixture_advertisement(),
            ESP32_WROOM_IMAGE_BOOT_ID,
            "esp32-wroom-uart-show-1",
        ),
        ESP32_S3_PHYSICAL_HOST_ID => (
            esp32_s3_build_fixture_advertisement(),
            ESP32_S3_IMAGE_BOOT_ID,
            "esp32-s3-uart-show-1",
        ),
        ESP32_C3_PHYSICAL_HOST_ID => (
            esp32_c3_build_fixture_advertisement(),
            ESP32_C3_IMAGE_BOOT_ID,
            "esp32-c3-uart-show-1",
        ),
        _ => {
            return Err(alloc::format!(
                "unsupported inspected ESP32 Host identity: {physical_host_id}"
            ));
        }
    };
    sink.host_id = HostId::from(physical_host_id);
    sink.boot_id = BootId::from(image_boot_id);
    let observation = BluetoothLineObservation {
        candidate: BluetoothDiscoveryCandidate {
            observation_sign_id: SignId::from("bluetooth/discovery/esp32"),
            controller_id: BluetoothControllerId::from("esp32/controller"),
            address: peer_address,
            address_kind: BluetoothAddressKind::RandomStatic,
            advertises_conduit_service: true,
        },
        base_instance_id: ConnectionBaseInstanceId::from(STD_ESP32_BLUETOOTH_BASE_INSTANCE_ID),
        pairing: BluetoothPairingState::Bonded,
        state: BluetoothLineState::Ready,
        negotiated_peer: Some(NegotiatedPeerIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        }),
        state_sign_id: SignId::from("bluetooth/line/esp32-ready"),
    };
    let line_offer = offer_ble_gatt_line(
        BleGattLineIdentity {
            line_id: LineId::from(STD_ESP32_BLUETOOTH_LINE_ID),
            binding_id: STD_ESP32_BLUETOOTH_BINDING_ID.into(),
            base_instance_id: observation.base_instance_id.clone(),
            source_host_id: source.host_id.clone(),
            source_boot_id: source.boot_id.clone(),
            source_endpoint_id: STD_ESP32_BLUETOOTH_SOURCE_ENDPOINT_ID.into(),
            sink_host_id: sink.host_id.clone(),
            sink_boot_id: sink.boot_id.clone(),
            sink_endpoint_id: STD_ESP32_BLUETOOTH_SINK_ENDPOINT_ID.into(),
            credential: LinkCredentialReference::Opaque("bond/esp32-controller".into()),
            authority: LinkAuthorityReference::Grant("grant/bluetooth/esp32-controller".into()),
        },
        &observation,
        BleGattProfile::FIRST,
        SignId::from("bluetooth/line/esp32-offer-ready"),
    )
    .map_err(|error| alloc::format!("Bluetooth Line offer: {error:?}"))?;
    let form = conduit_form::parse_with_startup(
        include_str!("../../../fixtures/forms/signal-demo.conduit"),
        &crate::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .map_err(|error| error.to_string())?;
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("signal-demo/pulse"),
                PlacementChoice {
                    host_id: source.host_id.clone(),
                    capability_id: "std-pico-pulse-1".into(),
                },
            ),
            (
                GearId::from("signal-demo/show"),
                PlacementChoice {
                    host_id: sink.host_id.clone(),
                    capability_id: capability_id.into(),
                },
            ),
        ]),
    };
    let plan = plan_with_line_offers(
        &form,
        &[source, sink],
        &placements,
        &[ConnectionBase::BluetoothLeGatt],
        DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
        SIGNAL_ENCODED_LEN,
        core::slice::from_ref(&line_offer),
    )
    .map_err(|error| error.to_string())?;
    Ok(ExactStdEsp32BluetoothPlan { plan, line_offer })
}

pub fn std_esp32_bluetooth_session_binding(
    runtime_boot: BootId,
) -> Result<SessionBinding, alloc::string::String> {
    std_esp32_bluetooth_session_binding_for_host(ESP32_WROOM_PHYSICAL_HOST_ID, runtime_boot)
}

pub fn std_esp32_bluetooth_session_binding_for_host(
    physical_host_id: &str,
    runtime_boot: BootId,
) -> Result<SessionBinding, alloc::string::String> {
    let exact = exact_std_esp32_bluetooth_plan_for_host([0; 6], physical_host_id)?;
    let source = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == STD_PICO_USB_SOURCE_HOST_ID)
        .ok_or_else(|| alloc::string::String::from("Bluetooth Plan lacks source fragment"))?;
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == physical_host_id)
        .ok_or_else(|| alloc::string::String::from("Bluetooth Plan lacks ESP32 fragment"))?;
    let connection = sink
        .connections
        .first()
        .ok_or_else(|| alloc::string::String::from("Bluetooth sink fragment lacks Cord"))?;
    let line = connection
        .selected_line
        .as_ref()
        .ok_or_else(|| alloc::string::String::from("Bluetooth Cord lacks selected Line"))?;
    SessionBinding {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        source_active_play_id: bind_active_play(
            &exact.plan.plan_id,
            &source.host_id,
            &source.boot_id,
            0,
        )
        .active_play_id,
        sink_active_play_id: bind_active_play(&exact.plan.plan_id, &sink.host_id, &sink.boot_id, 0)
            .active_play_id,
        plan_id: exact.plan.plan_id,
        source_fragment_id: source.fragment_id.clone(),
        sink_fragment_id: sink.fragment_id.clone(),
        connection_id: connection.connection_id.clone(),
        source: SessionEndpointIdentity {
            host_id: source.host_id.clone(),
            boot_id: source.boot_id.clone(),
        },
        sink: SessionEndpointIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        },
        value_kind: connection.value_kind.clone(),
        limits: SessionLimits {
            maximum_in_flight_items: connection.item_capacity,
            maximum_payload_bytes: connection.byte_capacity,
            maximum_buffered_bytes: connection.byte_capacity,
        },
        attachment: LineAttachment {
            line_id: line.line_id.clone(),
            link_binding_id: line.binding.binding_id.clone(),
            base: line.binding.base,
            base_instance_id: line.binding.base_instance_id.clone(),
            source_host_id: line.binding.source.host_id.clone(),
            source_boot_id: line.binding.source.boot_id.clone(),
            source_endpoint_id: line.binding.source.endpoint_id.clone(),
            sink_host_id: line.binding.sink.host_id.clone(),
            sink_boot_id: line.binding.sink.boot_id.clone(),
            sink_endpoint_id: line.binding.sink.endpoint_id.clone(),
            limits: line.binding.limits,
        },
    }
    .with_observed_boots(source.boot_id.clone(), runtime_boot)
    .map_err(|error| alloc::format!("runtime Bluetooth binding: {error:?}"))
}
