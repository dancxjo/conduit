use super::{
    count, label, MidiEndpointDirection, RawMidiEndpointObservation, A4_REFERENCE_MILLIHERTZ,
    MAXIMUM_INPUT_BYTES_PER_POLL, MAXIMUM_INPUT_OBSERVATIONS_PER_KERNEL_STEP,
    MAXIMUM_INPUT_PENDING_BYTES, MAXIMUM_INPUT_PENDING_MESSAGES, MAXIMUM_MESSAGES_PER_KERNEL_STEP,
    MIDI_A4_REFERENCE_CHARACTERISTIC, MIDI_BACKEND_CHARACTERISTIC,
    MIDI_BYTES_PER_STEP_CHARACTERISTIC, MIDI_CANCEL_POLICY_CHARACTERISTIC,
    MIDI_CHANNEL_CHARACTERISTIC, MIDI_DIRECTION_CHARACTERISTIC,
    MIDI_MESSAGES_PER_STEP_CHARACTERISTIC, MIDI_PENDING_BYTES_CHARACTERISTIC,
    MIDI_PENDING_MESSAGES_CHARACTERISTIC, MIDI_PRESSURE_POLICY_CHARACTERISTIC,
    MIDI_READINESS_WAIT_MILLIS, MIDI_READINESS_WAIT_MILLIS_CHARACTERISTIC,
    MIDI_RESOURCE_CHARACTERISTIC, MIDI_TIMING_PROFILE_CHARACTERISTIC, OUTPUT_CHANNEL,
};
use conduit_core::{
    BootId, CapabilityId, HostId, OfferGeneration, RealizationAdvertisement, ResourceHealth,
    ResourceObservation, ResourcePoolId, SignId,
};

/// One explicitly selected ALSA RawMIDI byte endpoint from a fresh observation.
///
/// Selection records identity and generation only. It neither opens the device
/// node nor grants authority to use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedRawMidiSelection {
    observation: RawMidiEndpointObservation,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    #[cfg(test)]
    fake_input: Option<Vec<u8>>,
}

impl HostedRawMidiSelection {
    pub fn select(
        observations: &[RawMidiEndpointObservation],
        direction: MidiEndpointDirection,
        card: u16,
        device: u16,
        subdevice: u16,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, String> {
        let observation = observations
            .iter()
            .find(|observation| {
                observation.direction == direction
                    && observation.card == card
                    && observation.device == device
                    && observation.subdevice == subdevice
            })
            .cloned()
            .ok_or_else(|| {
                format!(
                    "fresh raw MIDI {:?} endpoint hw:{card},{device},{subdevice} is absent",
                    direction
                )
            })?;
        Ok(Self {
            observation,
            boot_id,
            offer_generation,
            #[cfg(test)]
            fake_input: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_fake_input(mut self, bytes: Vec<u8>) -> Self {
        self.fake_input = Some(bytes);
        self
    }

    #[cfg(test)]
    pub(crate) fn fake_input(&self) -> Option<&[u8]> {
        self.fake_input.as_deref()
    }

    pub const fn observation(&self) -> &RawMidiEndpointObservation {
        &self.observation
    }

    pub const fn boot_id(&self) -> &BootId {
        &self.boot_id
    }

    pub const fn offer_generation(&self) -> OfferGeneration {
        self.offer_generation
    }

    pub fn resource_pool_id(&self) -> ResourcePoolId {
        ResourcePoolId::from(format!(
            "std/midi/alsa-raw/{}/{}/{}/card-{}/device-{}/subdevice-{}",
            self.boot_id.as_str(),
            self.offer_generation.0,
            self.observation.direction.identity_segment(),
            self.observation.card,
            self.observation.device,
            self.observation.subdevice,
        ))
    }

    pub fn resource_observation(&self, host_id: HostId, sign_id: SignId) -> ResourceObservation {
        ResourceObservation {
            host_id,
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            pool_id: self.resource_pool_id(),
            class_id: conduit_core::ResourceClassId::from(match self.observation.direction {
                MidiEndpointDirection::ReadableSource => {
                    conduit_std_catalog::MIDI_INPUT_RESOURCE_CLASS
                }
                MidiEndpointDirection::WritableDestination => {
                    conduit_std_catalog::MIDI_OUTPUT_RESOURCE_CLASS
                }
            }),
            health: ResourceHealth::Ready,
            unreserved_units: 1,
            utilized_units: 0,
            sign_id,
        }
    }

    /// Advertises the portable musical source only for one exact independently
    /// openable readable RawMIDI node. Selection and advertisement do not open
    /// the node or create authority.
    pub fn input_realization_advertisement(
        &self,
        host_id: HostId,
    ) -> Result<RealizationAdvertisement, &'static str> {
        if self.observation.direction != MidiEndpointDirection::ReadableSource {
            return Err("a writable raw MIDI destination cannot realize music/input");
        }
        if self.observation.direct_device_path().is_none() {
            return Err("raw MIDI subdevice has no exact direct device node");
        }
        let minimum_pitch =
            conduit_core::MusicalPitch::from_equal_tempered(-69, A4_REFERENCE_MILLIHERTZ, 0)
                .map_err(|_| "MIDI minimum pitch profile is invalid")?;
        let maximum_pitch =
            conduit_core::MusicalPitch::from_equal_tempered(58, A4_REFERENCE_MILLIHERTZ, 0)
                .map_err(|_| "MIDI maximum pitch profile is invalid")?;
        let profile = conduit_std_catalog::SoundCompatibilityProfile {
            profile_id: conduit_std_catalog::MUSIC_INPUT_MIDI_PROFILE.into(),
            seam: conduit_std_catalog::SoundSeam::MusicalEvents,
            minimum_pitch_millihertz: minimum_pitch.frequency_millihertz,
            maximum_pitch_millihertz: maximum_pitch.frequency_millihertz,
            maximum_polyphony: 128,
            maximum_events_per_second: 1_000,
            preserves_velocity: true,
            preserves_sustain: true,
            preserves_pitch_bend: true,
            maximum_pitch_bend_range_microcents: conduit_midi::MIDI_PITCH_BEND_RANGE_MICROCENTS,
            preserves_modulation: true,
            accepts_microtonal_pitch: false,
            supports_subtractive_filter: false,
            pcm: None,
        };
        let mut characteristics = conduit_std_catalog::sound_profile_characteristics(&profile);
        characteristics.extend([
            label(MIDI_DIRECTION_CHARACTERISTIC, "readable-source"),
            label(
                MIDI_RESOURCE_CHARACTERISTIC,
                self.resource_pool_id().as_str(),
            ),
            label(MIDI_BACKEND_CHARACTERISTIC, "alsa-raw-midi1@1"),
            count(MIDI_A4_REFERENCE_CHARACTERISTIC, A4_REFERENCE_MILLIHERTZ),
            count(
                MIDI_PENDING_MESSAGES_CHARACTERISTIC,
                u64::from(MAXIMUM_INPUT_PENDING_MESSAGES),
            ),
            count(
                MIDI_PENDING_BYTES_CHARACTERISTIC,
                u64::from(MAXIMUM_INPUT_PENDING_BYTES),
            ),
            count(
                MIDI_MESSAGES_PER_STEP_CHARACTERISTIC,
                u64::from(MAXIMUM_INPUT_OBSERVATIONS_PER_KERNEL_STEP),
            ),
            count(
                MIDI_BYTES_PER_STEP_CHARACTERISTIC,
                MAXIMUM_INPUT_BYTES_PER_POLL as u64,
            ),
            count(
                MIDI_READINESS_WAIT_MILLIS_CHARACTERISTIC,
                u64::from(MIDI_READINESS_WAIT_MILLIS),
            ),
            label(
                MIDI_TIMING_PROFILE_CHARACTERISTIC,
                "monotonic-read-completion-us@1",
            ),
            label(
                MIDI_PRESSURE_POLICY_CHARACTERISTIC,
                "nonblocking-read-pending",
            ),
            label(
                MIDI_CANCEL_POLICY_CHARACTERISTIC,
                "clear-parser-buffer-and-close",
            ),
        ]);
        characteristics.sort();
        Ok(RealizationAdvertisement {
            host_id,
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            capability_id: CapabilityId::from("music-input-midi1"),
            characteristics,
        })
    }

    /// Advertises the portable output contract only when this exact RawMIDI
    /// observation maps to an independently openable direct device node.
    pub fn output_realization_advertisement(
        &self,
        host_id: HostId,
    ) -> Result<RealizationAdvertisement, &'static str> {
        if self.observation.direction != MidiEndpointDirection::WritableDestination {
            return Err("a readable raw MIDI source cannot realize music/play output");
        }
        if self.observation.direct_device_path().is_none() {
            return Err("raw MIDI subdevice has no exact direct device node");
        }
        let minimum_pitch =
            conduit_core::MusicalPitch::from_equal_tempered(-69, A4_REFERENCE_MILLIHERTZ, 0)
                .map_err(|_| "MIDI minimum pitch profile is invalid")?;
        let maximum_pitch =
            conduit_core::MusicalPitch::from_equal_tempered(58, A4_REFERENCE_MILLIHERTZ, 0)
                .map_err(|_| "MIDI maximum pitch profile is invalid")?;
        let profile = conduit_std_catalog::SoundCompatibilityProfile {
            profile_id: conduit_std_catalog::MUSIC_PLAY_MIDI_PROFILE.into(),
            seam: conduit_std_catalog::SoundSeam::MusicalEvents,
            minimum_pitch_millihertz: minimum_pitch.frequency_millihertz,
            maximum_pitch_millihertz: maximum_pitch.frequency_millihertz,
            maximum_polyphony: 128,
            maximum_events_per_second: 1_000,
            preserves_velocity: true,
            preserves_sustain: true,
            preserves_pitch_bend: true,
            maximum_pitch_bend_range_microcents: conduit_midi::MIDI_PITCH_BEND_RANGE_MICROCENTS,
            preserves_modulation: true,
            accepts_microtonal_pitch: false,
            supports_subtractive_filter: false,
            pcm: None,
        };
        let mut characteristics = conduit_std_catalog::sound_profile_characteristics(&profile);
        characteristics.extend([
            label(MIDI_DIRECTION_CHARACTERISTIC, "writable-destination"),
            label(
                MIDI_RESOURCE_CHARACTERISTIC,
                self.resource_pool_id().as_str(),
            ),
            label(MIDI_BACKEND_CHARACTERISTIC, "alsa-raw-midi1@1"),
            count(MIDI_CHANNEL_CHARACTERISTIC, u64::from(OUTPUT_CHANNEL)),
            count(MIDI_A4_REFERENCE_CHARACTERISTIC, A4_REFERENCE_MILLIHERTZ),
            count(MIDI_PENDING_MESSAGES_CHARACTERISTIC, 0),
            count(MIDI_PENDING_BYTES_CHARACTERISTIC, 0),
            count(
                MIDI_MESSAGES_PER_STEP_CHARACTERISTIC,
                u64::from(MAXIMUM_MESSAGES_PER_KERNEL_STEP),
            ),
            label(
                MIDI_TIMING_PROFILE_CHARACTERISTIC,
                "plan-order-no-send-timestamp@1",
            ),
            label(
                MIDI_PRESSURE_POLICY_CHARACTERISTIC,
                "direct-write-would-block",
            ),
            label(MIDI_CANCEL_POLICY_CHARACTERISTIC, "cc123-then-close"),
        ]);
        characteristics.sort();
        Ok(RealizationAdvertisement {
            host_id,
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            capability_id: CapabilityId::from("music-play-midi1"),
            characteristics,
        })
    }
}

#[cfg(test)]
mod tests {
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
            characteristic.characteristic_id.as_str() == MIDI_BACKEND_CHARACTERISTIC
                && characteristic.value
                    == conduit_core::RealizationCharacteristicValue::Label(
                        "alsa-raw-midi1@1".into(),
                    )
        }));
        for id in [
            MIDI_PENDING_MESSAGES_CHARACTERISTIC,
            MIDI_PENDING_BYTES_CHARACTERISTIC,
        ] {
            assert!(advertisement.characteristics.iter().any(|characteristic| {
                characteristic.characteristic_id.as_str() == id
                    && characteristic.value
                        == conduit_core::RealizationCharacteristicValue::Count(0)
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
            characteristic.characteristic_id.as_str() == MIDI_TIMING_PROFILE_CHARACTERISTIC
                && characteristic.value
                    == conduit_core::RealizationCharacteristicValue::Label(
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
            conduit_std_catalog::MUSIC_INPUT_MIDI_IMPLEMENTATION
        );
        let resource = host
            .advertisement()
            .resources
            .iter()
            .find(|offer| offer.pool_id == expected_pool)
            .unwrap();
        assert_eq!(
            resource.class_id.as_str(),
            conduit_std_catalog::MIDI_INPUT_RESOURCE_CLASS
        );
        assert_eq!(resource.capacity_units, 1);

        let grant = host.midi_input_authority_grant("allow-controller").unwrap();
        assert_eq!(
            grant.contract_id.as_str(),
            conduit_std_catalog::MIDI_INPUT_AUTHORITY_CONTRACT
        );
        assert_eq!(
            grant.host_operation_contract_id.as_str(),
            conduit_std_catalog::MUSIC_INPUT_MIDI_OPERATION
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
}
