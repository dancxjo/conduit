use std::collections::BTreeMap;

use conduit_bluetooth::{
    offer_ble_gatt_line, BleGattLineIdentity, BleGattProfile, BluetoothAddressKind,
    BluetoothControllerId, BluetoothDiscoveryCandidate, BluetoothLineObservation,
    BluetoothLineState, BluetoothPairingState, NegotiatedPeerIdentity,
};
use conduit_core::{
    BaseImplementationId, BaseInstanceId, GearId, LineAvailability, LineId, LinkAuthorityReference,
    LinkCredentialReference, SignId,
};
use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
use conduit_signal::{signal_profile_catalog, SIGNAL_ENCODED_LEN};
use conduit_signal_conformance::{triple, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS};

fn plan_for_browser_line(
    form: &conduit_form::CheckedForm,
    browser_line: conduit_core::LineOffer,
) -> conduit_core::Plan {
    let exact = triple::exact_plan().unwrap();
    let placements = PlacementChoices {
        by_gear: BTreeMap::from([
            (
                GearId::from("triple-signal/pulse"),
                PlacementChoice {
                    host_id: exact.source_advertisement.host_id.clone(),
                    capability_id: conduit_core::CapabilityId::from(triple::PULSE_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/local"),
                PlacementChoice {
                    host_id: exact.source_advertisement.host_id.clone(),
                    capability_id: conduit_core::CapabilityId::from(triple::STDOUT_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/web"),
                PlacementChoice {
                    host_id: exact.browser_advertisement.host_id.clone(),
                    capability_id: conduit_core::CapabilityId::from(triple::BROWSER_CAPABILITY_ID),
                },
            ),
            (
                GearId::from("triple-signal/light"),
                PlacementChoice {
                    host_id: exact.pico_advertisement.host_id.clone(),
                    capability_id: conduit_core::CapabilityId::from(triple::PICO_CAPABILITY_ID),
                },
            ),
        ]),
    };
    let browser_line_id = browser_line.line_id.clone();
    let line_candidates = BTreeMap::from([(
        (
            GearId::from("triple-signal/pulse"),
            GearId::from("triple-signal/web"),
        ),
        vec![browser_line_id],
    )]);
    plan_with_options(
        form,
        &[
            exact.source_advertisement,
            exact.browser_advertisement,
            exact.pico_advertisement,
        ],
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &line_candidates,
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[browser_line, exact.pico_line],
        },
    )
    .unwrap()
}

fn selected_browser_line(plan: &conduit_core::Plan) -> &conduit_core::AdmittedLine {
    plan.fragments
        .iter()
        .flat_map(|fragment| &fragment.connections)
        .find_map(|connection| {
            connection.selected_line.as_ref().filter(|line| {
                matches!(
                    line.binding.base.as_str(),
                    "conduit.base/bluetooth-le-gatt@1" | "conduit.base/websocket-rfc6455@1"
                )
            })
        })
        .unwrap()
}

#[test]
fn loss_preserves_plan_and_fresh_planning_selects_a_replacement_for_the_same_form() {
    let exact = triple::exact_plan().unwrap();
    let websocket = exact.browser_line.clone();
    let observation = BluetoothLineObservation {
        candidate: BluetoothDiscoveryCandidate {
            observation_sign_id: SignId::from("bluetooth/discovery/browser-peer"),
            controller_id: BluetoothControllerId::from("bluez/hci0"),
            address: [1, 2, 3, 4, 5, 6],
            address_kind: BluetoothAddressKind::ResolvablePrivate,
            advertises_conduit_service: true,
        },
        base_instance_id: BaseInstanceId::from("bluez/hci0/session-a"),
        pairing: BluetoothPairingState::Bonded,
        state: BluetoothLineState::Ready,
        negotiated_peer: Some(NegotiatedPeerIdentity {
            host_id: websocket.binding.sink.host_id.clone(),
            boot_id: websocket.binding.sink.boot_id.clone(),
        }),
        state_sign_id: SignId::from("bluetooth/session-a/ready"),
    };
    let bluetooth = offer_ble_gatt_line(
        BleGattLineIdentity {
            line_id: LineId::from("s4/line/triple-browser-bluetooth"),
            binding_id: "s4/triple-browser-bluetooth-binding".into(),
            base_instance_id: observation.base_instance_id.clone(),
            source_host_id: websocket.binding.source.host_id.clone(),
            source_boot_id: websocket.binding.source.boot_id.clone(),
            source_endpoint_id: "s4/triple-browser-bluetooth-egress".into(),
            sink_host_id: websocket.binding.sink.host_id.clone(),
            sink_boot_id: websocket.binding.sink.boot_id.clone(),
            sink_endpoint_id: "s4/triple-browser-bluetooth-ingress".into(),
            credential: LinkCredentialReference::Opaque("bond/browser-peer".into()),
            authority: LinkAuthorityReference::Grant("grant/bluetooth/browser-peer".into()),
        },
        &observation,
        BleGattProfile::FIRST,
        SignId::from("bluetooth/line-a/ready"),
    )
    .unwrap();
    let form = conduit_form::parse_with_startup(
        include_str!("../../../proof/fixtures/forms/triple-signal.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .unwrap();

    let plan_a = plan_for_browser_line(&form, bluetooth.clone());
    let immutable_a = plan_a.clone();
    let unavailable = bluetooth.availability_sign(
        LineAvailability::Unavailable,
        SignId::from("bluetooth/line-a/lost"),
    );
    assert_eq!(unavailable.availability, LineAvailability::Unavailable);
    assert_eq!(plan_a, immutable_a);
    assert_eq!(
        selected_browser_line(&plan_a).binding.base,
        BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
    );

    let plan_b = plan_for_browser_line(&form, websocket);
    assert_ne!(plan_a.plan_id, plan_b.plan_id);
    assert_eq!(
        selected_browser_line(&plan_b).binding.base,
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1")
    );
    assert_eq!(
        plan_a.checked_form_id, plan_b.checked_form_id,
        "replacement planning preserves authored/checked meaning"
    );
}
