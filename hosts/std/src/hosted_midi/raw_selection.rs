use super::{
    count, label, MidiEndpointDirection, RawMidiEndpointObservation, A4_REFERENCE_MILLIHERTZ,
    MAXIMUM_MESSAGES_PER_KERNEL_STEP, MIDI_A4_REFERENCE_CHARACTERISTIC,
    MIDI_BACKEND_CHARACTERISTIC, MIDI_CANCEL_POLICY_CHARACTERISTIC, MIDI_CHANNEL_CHARACTERISTIC,
    MIDI_DIRECTION_CHARACTERISTIC, MIDI_MESSAGES_PER_STEP_CHARACTERISTIC,
    MIDI_PENDING_BYTES_CHARACTERISTIC, MIDI_PENDING_MESSAGES_CHARACTERISTIC,
    MIDI_PRESSURE_POLICY_CHARACTERISTIC, MIDI_RESOURCE_CHARACTERISTIC,
    MIDI_TIMING_PROFILE_CHARACTERISTIC, OUTPUT_CHANNEL,
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
        })
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
                MidiEndpointDirection::ReadableSource => "conduit.resource/midi-input@1",
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
}
