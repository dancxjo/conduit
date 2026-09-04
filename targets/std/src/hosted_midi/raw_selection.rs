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
    #[cfg(test)]
    fake_input_stays_open: bool,
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
            #[cfg(test)]
            fake_input_stays_open: false,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_fake_input(mut self, bytes: Vec<u8>) -> Self {
        self.fake_input = Some(bytes);
        self.fake_input_stays_open = true;
        self
    }

    #[cfg(test)]
    pub(crate) fn with_fake_input_then_disconnect(mut self, bytes: Vec<u8>) -> Self {
        self.fake_input = Some(bytes);
        self.fake_input_stays_open = false;
        self
    }

    #[cfg(test)]
    pub(crate) fn fake_input(&self) -> Option<&[u8]> {
        self.fake_input.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn fake_input_stays_open(&self) -> bool {
        self.fake_input_stays_open
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
                    conduit_std_offers::MIDI_INPUT_RESOURCE_CLASS
                }
                MidiEndpointDirection::WritableDestination => {
                    conduit_std_offers::MIDI_OUTPUT_RESOURCE_CLASS
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
        let profile = conduit_semantic_catalog::SoundCompatibilityProfile {
            profile_id: conduit_std_offers::MUSIC_INPUT_MIDI_PROFILE.into(),
            seam: conduit_semantic_catalog::SoundSeam::MusicalEvents,
            minimum_pitch_millihertz: conduit_audio::MINIMUM_PITCH_MILLIHERTZ,
            maximum_pitch_millihertz: conduit_audio::MAXIMUM_PITCH_MILLIHERTZ,
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
        let mut characteristics = conduit_semantic_catalog::sound_profile_characteristics(&profile);
        characteristics.extend([
            label(MIDI_DIRECTION_CHARACTERISTIC, "readable-source"),
            label(
                MIDI_RESOURCE_CHARACTERISTIC,
                self.resource_pool_id().as_str(),
            ),
            label(MIDI_BACKEND_CHARACTERISTIC, "alsa-raw-midi1@1"),
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
            conduit_audio::MusicalPitch::from_equal_tempered(-69, A4_REFERENCE_MILLIHERTZ, 0)
                .map_err(|_| "MIDI minimum pitch profile is invalid")?;
        let maximum_pitch =
            conduit_audio::MusicalPitch::from_equal_tempered(58, A4_REFERENCE_MILLIHERTZ, 0)
                .map_err(|_| "MIDI maximum pitch profile is invalid")?;
        let profile = conduit_semantic_catalog::SoundCompatibilityProfile {
            profile_id: conduit_std_offers::MUSIC_PLAY_MIDI_PROFILE.into(),
            seam: conduit_semantic_catalog::SoundSeam::MusicalEvents,
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
        let mut characteristics = conduit_semantic_catalog::sound_profile_characteristics(&profile);
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
mod tests;
