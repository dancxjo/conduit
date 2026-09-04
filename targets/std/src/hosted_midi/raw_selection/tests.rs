use super::*;

fn observation(direction: MidiEndpointDirection, subdevice: u16) -> RawMidiEndpointObservation {
    RawMidiEndpointObservation {
        card: 2,
        device: 1,
        subdevice,
        name: "Controller".into(),
        direction,
    }
}

#[test]
fn selection_pins_exact_direction_boot_generation_and_coordinates() {
    let observations = [
        observation(MidiEndpointDirection::ReadableSource, 0),
        observation(MidiEndpointDirection::WritableDestination, 0),
    ];
    let selected = HostedRawMidiSelection::select(
        &observations,
        MidiEndpointDirection::WritableDestination,
        2,
        1,
        0,
        BootId::from("boot-a"),
        OfferGeneration(7),
    )
    .unwrap();
    assert_eq!(
        selected.resource_pool_id().as_str(),
        "std/midi/alsa-raw/boot-a/7/writable-destination/card-2/device-1/subdevice-0"
    );
    assert_eq!(selected.observation().alsa_device_name(), "hw:2,1,0");
    assert_eq!(selected.boot_id().as_str(), "boot-a");
    assert_eq!(selected.offer_generation(), OfferGeneration(7));
}

#[test]
fn absent_or_wrong_direction_endpoint_refuses_selection() {
    let observations = [observation(MidiEndpointDirection::ReadableSource, 0)];
    assert!(HostedRawMidiSelection::select(
        &observations,
        MidiEndpointDirection::WritableDestination,
        2,
        1,
        0,
        BootId::from("boot-a"),
        OfferGeneration(1),
    )
    .is_err());
}

#[test]
fn advertisement_names_raw_backend_and_refuses_unopenable_subdevice() {
    let selected = HostedRawMidiSelection::select(
        &[observation(MidiEndpointDirection::WritableDestination, 0)],
        MidiEndpointDirection::WritableDestination,
        2,
        1,
        0,
        BootId::from("boot-a"),
        OfferGeneration(3),
    )
    .unwrap();
    let advertisement = selected
        .output_realization_advertisement(HostId::from("host-a"))
        .unwrap();
    assert!(advertisement.characteristics.iter().any(|characteristic| {
        characteristic.definition.characteristic_id.as_str() == MIDI_BACKEND_CHARACTERISTIC
            && characteristic.value
                == conduit_core::CharacteristicValue::Categorical("alsa-raw-midi1@1".into())
    }));
    for id in [
        MIDI_PENDING_MESSAGES_CHARACTERISTIC,
        MIDI_PENDING_BYTES_CHARACTERISTIC,
    ] {
        assert!(advertisement.characteristics.iter().any(|characteristic| {
            characteristic.definition.characteristic_id.as_str() == id
                && characteristic.value
                    == conduit_core::CharacteristicValue::UnsignedQuantity {
                        value: 0,
                        unit: conduit_semantic_catalog::sound_characteristic_unit(id),
                    }
        }));
    }

    let subdevice = HostedRawMidiSelection::select(
        &[observation(MidiEndpointDirection::WritableDestination, 4)],
        MidiEndpointDirection::WritableDestination,
        2,
        1,
        4,
        BootId::from("boot-a"),
        OfferGeneration(3),
    )
    .unwrap();
    assert_eq!(
        subdevice.output_realization_advertisement(HostId::from("host-a")),
        Err("raw MIDI subdevice has no exact direct device node")
    );
}

#[test]
fn readable_advertisement_is_exact_bounded_and_distinct_from_output() {
    let selected = HostedRawMidiSelection::select(
        &[observation(MidiEndpointDirection::ReadableSource, 0)],
        MidiEndpointDirection::ReadableSource,
        2,
        1,
        0,
        BootId::from("boot-input"),
        OfferGeneration(8),
    )
    .unwrap();
    let advertisement = selected
        .input_realization_advertisement(HostId::from("host-a"))
        .unwrap();
    assert_eq!(advertisement.capability_id.as_str(), "music-input-midi1");
    assert!(advertisement.characteristics.iter().any(|characteristic| {
        characteristic.definition.characteristic_id.as_str() == MIDI_TIMING_PROFILE_CHARACTERISTIC
            && characteristic.value
                == conduit_core::CharacteristicValue::Categorical(
                    "monotonic-read-completion-us@1".into(),
                )
    }));
    assert!(selected
        .output_realization_advertisement(HostId::from("host-a"))
        .is_err());

    let output = HostedRawMidiSelection::select(
        &[observation(MidiEndpointDirection::WritableDestination, 0)],
        MidiEndpointDirection::WritableDestination,
        2,
        1,
        0,
        BootId::from("boot-input"),
        OfferGeneration(8),
    )
    .unwrap();
    assert!(output
        .input_realization_advertisement(HostId::from("host-a"))
        .is_err());

    let subdevice = HostedRawMidiSelection::select(
        &[observation(MidiEndpointDirection::ReadableSource, 2)],
        MidiEndpointDirection::ReadableSource,
        2,
        1,
        2,
        BootId::from("boot-input"),
        OfferGeneration(8),
    )
    .unwrap();
    assert_eq!(
        subdevice.input_realization_advertisement(HostId::from("host-a")),
        Err("raw MIDI subdevice has no exact direct device node")
    );
}

#[test]
fn host_construction_advertises_without_opening_the_observed_device() {
    let boot_id = BootId::from("boot-no-open");
    let generation = OfferGeneration(4);
    let selected = HostedRawMidiSelection::select(
        &[RawMidiEndpointObservation {
            card: u16::MAX,
            device: u16::MAX,
            subdevice: 0,
            name: "Absent proof endpoint".into(),
            direction: MidiEndpointDirection::WritableDestination,
        }],
        MidiEndpointDirection::WritableDestination,
        u16::MAX,
        u16::MAX,
        0,
        boot_id.clone(),
        generation,
    )
    .unwrap();
    let host = crate::StdHost::new_with_raw_midi_output(
        crate::StdHostConfig {
            host_id: HostId::from("host-no-open"),
            boot_id,
            offer_generation: generation,
        },
        crate::StdHostComposition::minimal(),
        selected,
    )
    .expect("Host construction must not open the selected device");
    assert_eq!(
        host.raw_midi_output_selection()
            .unwrap()
            .observation()
            .alsa_device_name(),
        format!("hw:{0},{0},0", u16::MAX)
    );
}

#[test]
fn input_host_construction_offers_exact_resource_and_independent_authority() {
    let boot_id = BootId::from("boot-input-host");
    let generation = OfferGeneration(9);
    let selected = HostedRawMidiSelection::select(
        &[RawMidiEndpointObservation {
            card: u16::MAX,
            device: u16::MAX,
            subdevice: 0,
            name: "Absent input proof endpoint".into(),
            direction: MidiEndpointDirection::ReadableSource,
        }],
        MidiEndpointDirection::ReadableSource,
        u16::MAX,
        u16::MAX,
        0,
        boot_id.clone(),
        generation,
    )
    .unwrap();
    let expected_pool = selected.resource_pool_id();
    let host = crate::StdHost::new_with_raw_midi_input(
        crate::StdHostConfig {
            host_id: HostId::from("host-input"),
            boot_id: boot_id.clone(),
            offer_generation: generation,
        },
        crate::StdHostComposition::minimal(),
        selected,
    )
    .expect("Host construction must not open the selected input device");

    assert_eq!(
        host.raw_midi_input_selection().unwrap().resource_pool_id(),
        expected_pool
    );
    let capability = host
        .advertisement()
        .capabilities
        .iter()
        .find(|offer| offer.capability_id.as_str() == "music-input-midi1")
        .unwrap();
    assert_eq!(
        capability.implementation.implementation_id.as_str(),
        conduit_std_offers::MUSIC_INPUT_MIDI_IMPLEMENTATION
    );
    let resource = host
        .advertisement()
        .resources
        .iter()
        .find(|offer| offer.pool_id == expected_pool)
        .unwrap();
    assert_eq!(
        resource.class_id.as_str(),
        conduit_std_offers::MIDI_INPUT_RESOURCE_CLASS
    );
    assert_eq!(resource.capacity_units, 1);

    let grant = host.midi_input_authority_grant("allow-controller").unwrap();
    assert_eq!(
        grant.contract_id.as_str(),
        conduit_std_offers::MIDI_INPUT_AUTHORITY_CONTRACT
    );
    assert_eq!(
        grant.host_operation_contract_id.as_str(),
        conduit_std_offers::MUSIC_INPUT_MIDI_OPERATION
    );
    assert_eq!(grant.host_id.as_str(), "host-input");
    assert_eq!(grant.boot_id, boot_id);
    assert_eq!(grant.capability_id.as_str(), "music-input-midi1");
}

#[test]
fn input_host_refuses_output_direction_or_stale_identity() {
    let boot_id = BootId::from("boot-input-host");
    let selected = HostedRawMidiSelection::select(
        &[observation(MidiEndpointDirection::WritableDestination, 0)],
        MidiEndpointDirection::WritableDestination,
        2,
        1,
        0,
        boot_id.clone(),
        OfferGeneration(3),
    )
    .unwrap();
    assert!(crate::StdHost::new_with_raw_midi_input(
        crate::StdHostConfig {
            host_id: HostId::from("host-input"),
            boot_id,
            offer_generation: OfferGeneration(3),
        },
        crate::StdHostComposition::minimal(),
        selected,
    )
    .is_err());

    let stale = HostedRawMidiSelection::select(
        &[observation(MidiEndpointDirection::ReadableSource, 0)],
        MidiEndpointDirection::ReadableSource,
        2,
        1,
        0,
        BootId::from("old-boot"),
        OfferGeneration(2),
    )
    .unwrap();
    assert!(crate::StdHost::new_with_raw_midi_input(
        crate::StdHostConfig {
            host_id: HostId::from("host-input"),
            boot_id: BootId::from("new-boot"),
            offer_generation: OfferGeneration(2),
        },
        crate::StdHostComposition::minimal(),
        stale,
    )
    .is_err());
}

#[test]
fn reconnect_requires_a_fresh_generation_scoped_resource_identity() {
    let observation = observation(MidiEndpointDirection::ReadableSource, 0);
    let boot_id = BootId::from("boot-reconnect");
    let before = HostedRawMidiSelection::select(
        std::slice::from_ref(&observation),
        MidiEndpointDirection::ReadableSource,
        2,
        1,
        0,
        boot_id.clone(),
        OfferGeneration(4),
    )
    .unwrap();
    let after = HostedRawMidiSelection::select(
        &[observation],
        MidiEndpointDirection::ReadableSource,
        2,
        1,
        0,
        boot_id.clone(),
        OfferGeneration(5),
    )
    .unwrap();

    assert_ne!(before.resource_pool_id(), after.resource_pool_id());
    assert!(crate::StdHost::new_with_raw_midi_input(
        crate::StdHostConfig {
            host_id: HostId::from("host-reconnect"),
            boot_id,
            offer_generation: OfferGeneration(5),
        },
        crate::StdHostComposition::minimal(),
        before,
    )
    .is_err());
}
