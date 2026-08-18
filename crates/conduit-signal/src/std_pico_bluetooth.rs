//! Canonical std-to-Pico Signal plan over the finite BLE GATT Line profile.

use alloc::collections::BTreeMap;

use conduit_bluetooth::{
    offer_ble_gatt_line, BleGattLineIdentity, BleGattProfile, BluetoothAddressKind,
    BluetoothControllerId, BluetoothDiscoveryCandidate, BluetoothLineObservation,
    BluetoothLineState, BluetoothPairingState, NegotiatedPeerIdentity,
};
use conduit_core::{
    bind_active_play, ConnectionBase, ConnectionBaseInstanceId, GearId, LineId,
    LinkAuthorityReference, LinkCredentialReference, Plan, SignId,
};
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_wire::{LineAttachment, SessionBinding, SessionEndpointIdentity, SessionLimits};

use crate::{
    signal_profile_catalog, std_pico_usb_sink_advertisement, std_pico_usb_source_advertisement,
    DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS, SIGNAL_ENCODED_LEN, STD_PICO_USB_SINK_HOST_ID,
};

pub const STD_PICO_BLUETOOTH_LINE_ID: &str = "bluetooth/line/std-pico-gatt";
pub const STD_PICO_BLUETOOTH_BINDING_ID: &str = "bluetooth/binding/std-pico-gatt";
pub const STD_PICO_BLUETOOTH_BASE_INSTANCE_ID: &str = "pico/cyw43/ble-session-0";
pub const STD_PICO_BLUETOOTH_SOURCE_ENDPOINT_ID: &str = "bluetooth/std-source-write";
pub const STD_PICO_BLUETOOTH_SINK_ENDPOINT_ID: &str = "bluetooth/pico-sink-notify";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactStdPicoBluetoothPlan {
    pub plan: Plan,
    pub line_offer: conduit_core::LineOffer,
}

/// Build the immutable capstone Plan from one exact, already-paired current
/// observation. The Bluetooth address remains observation truth and is not an
/// input to Form, Host, Boot, or Body identity.
pub fn exact_std_pico_bluetooth_plan(
    peer_address: [u8; 6],
) -> Result<ExactStdPicoBluetoothPlan, alloc::string::String> {
    let source = std_pico_usb_source_advertisement();
    let sink = std_pico_usb_sink_advertisement();
    let observation = BluetoothLineObservation {
        candidate: BluetoothDiscoveryCandidate {
            observation_sign_id: SignId::from("bluetooth/discovery/pico"),
            controller_id: BluetoothControllerId::from("pico/cyw43"),
            address: peer_address,
            address_kind: BluetoothAddressKind::RandomStatic,
            advertises_conduit_service: true,
        },
        base_instance_id: ConnectionBaseInstanceId::from(STD_PICO_BLUETOOTH_BASE_INSTANCE_ID),
        pairing: BluetoothPairingState::Bonded,
        state: BluetoothLineState::Ready,
        negotiated_peer: Some(NegotiatedPeerIdentity {
            host_id: sink.host_id.clone(),
            boot_id: sink.boot_id.clone(),
        }),
        state_sign_id: SignId::from("bluetooth/line/pico-ready"),
    };
    let line_offer = offer_ble_gatt_line(
        BleGattLineIdentity {
            line_id: LineId::from(STD_PICO_BLUETOOTH_LINE_ID),
            binding_id: STD_PICO_BLUETOOTH_BINDING_ID.into(),
            base_instance_id: observation.base_instance_id.clone(),
            source_host_id: source.host_id.clone(),
            source_boot_id: source.boot_id.clone(),
            source_endpoint_id: STD_PICO_BLUETOOTH_SOURCE_ENDPOINT_ID.into(),
            sink_host_id: sink.host_id.clone(),
            sink_boot_id: sink.boot_id.clone(),
            sink_endpoint_id: STD_PICO_BLUETOOTH_SINK_ENDPOINT_ID.into(),
            credential: LinkCredentialReference::Opaque("bond/pico-cyw43".into()),
            authority: LinkAuthorityReference::Grant("grant/bluetooth/pico-cyw43".into()),
        },
        &observation,
        BleGattProfile::FIRST,
        SignId::from("bluetooth/line/pico-offer-ready"),
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
                    capability_id: "pico-cyw43-show-1".into(),
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
    debug_assert!(plan
        .fragments
        .iter()
        .any(|fragment| fragment.host_id.as_str() == STD_PICO_USB_SINK_HOST_ID));
    Ok(ExactStdPicoBluetoothPlan { plan, line_offer })
}

pub fn std_pico_bluetooth_session_binding() -> Result<SessionBinding, alloc::string::String> {
    let exact = exact_std_pico_bluetooth_plan([0; 6])?;
    let source = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() != STD_PICO_USB_SINK_HOST_ID)
        .ok_or_else(|| alloc::string::String::from("Bluetooth Plan lacks source fragment"))?;
    let sink = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == STD_PICO_USB_SINK_HOST_ID)
        .ok_or_else(|| alloc::string::String::from("Bluetooth Plan lacks sink fragment"))?;
    let connection = sink
        .connections
        .first()
        .ok_or_else(|| alloc::string::String::from("Bluetooth sink fragment lacks Cord"))?;
    let line = connection
        .selected_line
        .as_ref()
        .ok_or_else(|| alloc::string::String::from("Bluetooth Cord lacks selected Line"))?;
    Ok(SessionBinding {
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
    })
}
