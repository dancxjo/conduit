use conduit_bluetooth::{
    offer_ble_gatt_line, AdmittedBluetoothLineBasis, BleGattLineIdentity, BleGattProfile,
    BleLineAdmissionError, BluetoothAddressKind, BluetoothControllerId,
    BluetoothDiscoveryCandidate, BluetoothLineObservation, BluetoothLineState,
    BluetoothObservationError, BluetoothPairingState, NegotiatedPeerIdentity,
};
use conduit_core::{
    BaseImplementationId, BaseInstanceId, BootId, HostId, LineAvailability, LinkAuthorityReference,
    LinkBindingId, LinkCredentialReference, LinkEndpointId, SignId,
};

fn observation(address: [u8; 6], boot: &str, base: &str) -> BluetoothLineObservation {
    BluetoothLineObservation {
        candidate: BluetoothDiscoveryCandidate {
            observation_sign_id: SignId::from("bluetooth/discovery/7"),
            controller_id: BluetoothControllerId::from("bluez/hci0"),
            address,
            address_kind: BluetoothAddressKind::ResolvablePrivate,
            advertises_conduit_service: true,
        },
        base_instance_id: BaseInstanceId::from(base),
        pairing: BluetoothPairingState::Bonded,
        state: BluetoothLineState::Ready,
        negotiated_peer: Some(NegotiatedPeerIdentity {
            host_id: HostId::from("peer-host"),
            boot_id: BootId::from(boot),
        }),
        state_sign_id: SignId::from("bluetooth/line/ready/7"),
    }
}

fn identity(boot: &str, base: &str) -> BleGattLineIdentity {
    BleGattLineIdentity {
        line_id: conduit_core::LineId::from("line/ble-gatt/7"),
        binding_id: LinkBindingId::from("binding/ble-gatt/7"),
        base_instance_id: BaseInstanceId::from(base),
        source_host_id: HostId::from("std-host"),
        source_boot_id: BootId::from("std-boot"),
        source_endpoint_id: LinkEndpointId::from("std/ble/egress"),
        sink_host_id: HostId::from("peer-host"),
        sink_boot_id: BootId::from(boot),
        sink_endpoint_id: LinkEndpointId::from("peer/ble/ingress"),
        credential: LinkCredentialReference::Opaque(conduit_core::CredentialReferenceId::from(
            "bond/peer-7",
        )),
        authority: LinkAuthorityReference::Grant(conduit_core::AuthorityGrantId::from(
            "grant/bluetooth/peer-7",
        )),
    }
}

#[test]
fn ready_line_is_exact_finite_and_below_cord_meaning() {
    let observed = observation([1, 2, 3, 4, 5, 6], "peer-boot-1", "bluez/hci0/session-7");
    let line = offer_ble_gatt_line(
        identity("peer-boot-1", "bluez/hci0/session-7"),
        &observed,
        BleGattProfile::FIRST,
        SignId::from("line/ble-gatt/7/ready"),
    )
    .unwrap();

    assert_eq!(
        line.binding.base,
        BaseImplementationId::from("conduit.base/bluetooth-le-gatt@1")
    );
    assert_eq!(line.availability.availability, LineAvailability::Ready);
    assert_eq!(line.binding.limits.maximum_in_flight_items, 1);
    assert_eq!(line.binding.limits.maximum_payload_bytes, 96);
    assert_eq!(line.binding.limits.maximum_frame_bytes, 2_048);
    assert_eq!(line.binding.limits.maximum_buffered_bytes, 4_096);
    assert!(line.validate_sign_identity());
    assert_eq!(line.contract, BleGattProfile::line_contract());

    let json = serde_json::to_string(&line).unwrap();
    assert!(!json.contains("01:02:03:04:05:06"));
    assert!(!json.contains("ResolvablePrivate"));
}

#[test]
fn discovery_pairing_identity_and_readiness_are_independent_gates() {
    let mut observed = observation([1, 2, 3, 4, 5, 6], "peer-boot-1", "bluez/hci0/session-7");
    observed.state = BluetoothLineState::Discovered;
    assert_eq!(
        offer_ble_gatt_line(
            identity("peer-boot-1", "bluez/hci0/session-7"),
            &observed,
            BleGattProfile::FIRST,
            SignId::from("ready"),
        ),
        Err(BleLineAdmissionError::NotReady)
    );

    observed.state = BluetoothLineState::Ready;
    observed.candidate.advertises_conduit_service = false;
    assert_eq!(
        offer_ble_gatt_line(
            identity("peer-boot-1", "bluez/hci0/session-7"),
            &observed,
            BleGattProfile::FIRST,
            SignId::from("ready"),
        ),
        Err(BleLineAdmissionError::MissingConduitService)
    );

    observed.candidate.advertises_conduit_service = true;
    observed.pairing = BluetoothPairingState::Unpaired;
    assert_eq!(
        offer_ble_gatt_line(
            identity("peer-boot-1", "bluez/hci0/session-7"),
            &observed,
            BleGattProfile::FIRST,
            SignId::from("ready"),
        ),
        Err(BleLineAdmissionError::NotPaired)
    );

    observed.pairing = BluetoothPairingState::Paired;
    observed.negotiated_peer = None;
    assert_eq!(
        offer_ble_gatt_line(
            identity("peer-boot-1", "bluez/hci0/session-7"),
            &observed,
            BleGattProfile::FIRST,
            SignId::from("ready"),
        ),
        Err(BleLineAdmissionError::MissingPeerIdentity)
    );
}

#[test]
fn address_and_boot_changes_cannot_rebind_a_stale_base_instance() {
    let first = observation([1, 2, 3, 4, 5, 6], "peer-boot-1", "bluez/hci0/session-7");
    let offer = offer_ble_gatt_line(
        identity("peer-boot-1", "bluez/hci0/session-7"),
        &first,
        BleGattProfile::FIRST,
        SignId::from("ready"),
    )
    .unwrap();
    let admitted =
        AdmittedBluetoothLineBasis::from_offer(&offer, &first, BleGattProfile::FIRST).unwrap();

    let changed_address = observation([6, 5, 4, 3, 2, 1], "peer-boot-1", "bluez/hci0/session-7");
    assert_eq!(
        first.validate_successor(&changed_address),
        Err(BluetoothObservationError::AddressChangedWithoutFreshBase)
    );
    assert_eq!(
        admitted.validate_current(&changed_address),
        Err(BluetoothObservationError::AddressMismatch)
    );

    let changed_boot = observation([1, 2, 3, 4, 5, 6], "peer-boot-2", "bluez/hci0/session-7");
    assert_eq!(
        first.validate_successor(&changed_boot),
        Err(BluetoothObservationError::PeerBootChangedWithoutFreshBase)
    );

    let mut changed_host = first.clone();
    changed_host.negotiated_peer.as_mut().unwrap().host_id = HostId::from("different-peer-host");
    assert_eq!(
        first.validate_successor(&changed_host),
        Err(BluetoothObservationError::PeerHostChangedWithoutFreshBase)
    );

    let fresh = observation([6, 5, 4, 3, 2, 1], "peer-boot-2", "bluez/hci0/session-8");
    first.validate_successor(&fresh).unwrap();
    assert_eq!(
        admitted.validate_current(&fresh),
        Err(BluetoothObservationError::StaleBaseInstance)
    );
    let replacement = offer_ble_gatt_line(
        identity("peer-boot-2", "bluez/hci0/session-8"),
        &fresh,
        BleGattProfile::FIRST,
        SignId::from("replacement-ready"),
    )
    .unwrap();
    assert_ne!(
        offer.binding.base_instance_id,
        replacement.binding.base_instance_id
    );

    let mut lost = first.clone();
    lost.state = BluetoothLineState::Lost;
    let mut illicit_reconnect = lost.clone();
    illicit_reconnect.state = BluetoothLineState::Ready;
    assert_eq!(
        lost.validate_successor(&illicit_reconnect),
        Err(BluetoothObservationError::LostRealizationReused)
    );

    let mut unavailable = first.clone();
    unavailable.state = BluetoothLineState::Connecting;
    assert_eq!(
        admitted.validate_current(&unavailable),
        Err(BluetoothObservationError::NotReady)
    );
}
