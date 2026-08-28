mod discovery;
mod host;
mod input;
#[cfg(test)]
mod input_fake;
mod output;
#[cfg(test)]
pub(crate) mod output_fake;
mod output_selection;
mod raw_discovery;
mod raw_selection;

pub use discovery::{
    discover_alsa_sequencer_endpoints, MidiDiscoveryError, MidiEndpointDirection,
    MidiEndpointObservation,
};
pub use input::{
    MidiInputFailure, MidiInputLifecycle, MidiInputPoll, MidiInputReport, MidiInputSession,
    MAXIMUM_INPUT_BYTES_PER_POLL, MAXIMUM_INPUT_BYTES_PER_SESSION, MAXIMUM_INPUT_PENDING_BYTES,
    MAXIMUM_INPUT_PENDING_MESSAGES,
};
pub(crate) use output::MidiOutputSession;
pub use output::{MidiOutputFailure, MidiOutputLifecycle, MidiOutputReport};
pub(crate) use output_selection::MidiOutputSelection;
pub use raw_discovery::{
    discover_raw_midi_endpoints, RawMidiDiscoveryError, RawMidiEndpointObservation,
    MAXIMUM_RAW_MIDI_ENDPOINTS,
};
pub use raw_selection::HostedRawMidiSelection;

use conduit_core::{
    BootId, CapabilityId, HostId, OfferGeneration, RealizationAdvertisement,
    RealizationCharacteristic, ResourceHealth, ResourceObservation, ResourcePoolId, SignId,
};

pub const OUTPUT_CHANNEL: u8 = 0;
pub const A4_REFERENCE_MILLIHERTZ: u64 = 440_000;
pub const MAXIMUM_PENDING_MESSAGES: u16 = 256;
pub const MAXIMUM_PENDING_BYTES: u16 = MAXIMUM_PENDING_MESSAGES * 3;
pub const MAXIMUM_MESSAGES_PER_KERNEL_STEP: u16 = 32;
pub const MAXIMUM_INPUT_OBSERVATIONS_PER_KERNEL_STEP: u16 = 1;
pub const MIDI_DIRECTION_CHARACTERISTIC: &str = "midi/direction@1";
pub const MIDI_RESOURCE_CHARACTERISTIC: &str = "midi/resource@1";
pub const MIDI_BACKEND_CHARACTERISTIC: &str = "midi/backend@1";
pub const MIDI_CHANNEL_CHARACTERISTIC: &str = "midi/channel@1";
pub const MIDI_A4_REFERENCE_CHARACTERISTIC: &str = "midi/a4-reference-millihertz@1";
pub const MIDI_PENDING_MESSAGES_CHARACTERISTIC: &str = "midi/maximum-pending-messages@1";
pub const MIDI_PENDING_BYTES_CHARACTERISTIC: &str = "midi/maximum-pending-bytes@1";
pub const MIDI_MESSAGES_PER_STEP_CHARACTERISTIC: &str = "midi/maximum-messages-per-step@1";
pub const MIDI_TIMING_PROFILE_CHARACTERISTIC: &str = "midi/timing-profile@1";
pub const MIDI_PRESSURE_POLICY_CHARACTERISTIC: &str = "midi/pressure-policy@1";
pub const MIDI_CANCEL_POLICY_CHARACTERISTIC: &str = "midi/cancel-policy@1";
pub const MIDI_BYTES_PER_STEP_CHARACTERISTIC: &str = "midi/maximum-bytes-per-step@1";
pub const MIDI_READINESS_WAIT_MILLIS_CHARACTERISTIC: &str = "midi/readiness-wait-millis@1";
pub const MIDI_READINESS_WAIT_MILLIS: u16 = 10;

/// Exact portable-musical subset preserved by the reviewed MIDI 1.0 output
/// adapter. Endpoint availability and authority remain observation facts.
pub fn output_compatibility_profile(
) -> Result<conduit_semantic_catalog::SoundCompatibilityProfile, &'static str> {
    let minimum_pitch =
        conduit_audio::MusicalPitch::from_equal_tempered(-69, A4_REFERENCE_MILLIHERTZ, 0)
            .map_err(|_| "MIDI minimum pitch profile is invalid")?;
    let maximum_pitch =
        conduit_audio::MusicalPitch::from_equal_tempered(58, A4_REFERENCE_MILLIHERTZ, 0)
            .map_err(|_| "MIDI maximum pitch profile is invalid")?;
    Ok(conduit_semantic_catalog::SoundCompatibilityProfile {
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
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedMidiSelection {
    observation: MidiEndpointObservation,
    boot_id: BootId,
    offer_generation: OfferGeneration,
    #[cfg(test)]
    fake_output: Option<output_fake::FakeMidiOutputBehavior>,
}

impl HostedMidiSelection {
    pub fn select(
        observations: &[MidiEndpointObservation],
        direction: MidiEndpointDirection,
        client: u16,
        port: u16,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Result<Self, String> {
        let observation = observations
            .iter()
            .find(|observation| {
                observation.direction == direction
                    && observation.client == client
                    && observation.port == port
            })
            .cloned()
            .ok_or_else(|| {
                format!(
                    "fresh MIDI {:?} endpoint {client}:{port} is absent",
                    direction
                )
            })?;
        Ok(Self {
            observation,
            boot_id,
            offer_generation,
            #[cfg(test)]
            fake_output: None,
        })
    }

    #[cfg(test)]
    pub(crate) fn with_fake_output(
        mut self,
        behavior: output_fake::FakeMidiOutputBehavior,
    ) -> Self {
        self.fake_output = Some(behavior);
        self
    }

    pub const fn observation(&self) -> &MidiEndpointObservation {
        &self.observation
    }

    pub fn sequencer_address(&self) -> String {
        format!("{}:{}", self.observation.client, self.observation.port)
    }

    pub fn resource_pool_id(&self) -> ResourcePoolId {
        ResourcePoolId::from(format!(
            "std/midi/alsa-seq/{}/{}/{}/client-{}/port-{}",
            self.boot_id.as_str(),
            self.offer_generation.0,
            self.observation.direction.identity_segment(),
            self.observation.client,
            self.observation.port,
        ))
    }

    pub const fn boot_id(&self) -> &BootId {
        &self.boot_id
    }

    pub const fn offer_generation(&self) -> OfferGeneration {
        self.offer_generation
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
                    conduit_std_offers::MIDI_OUTPUT_RESOURCE_CLASS
                }
            }),
            health: ResourceHealth::Ready,
            unreserved_units: 1,
            utilized_units: 0,
            sign_id,
        }
    }

    /// Advertises only the already-defined portable `music/play` output
    /// contract. Readable endpoints need a separately reviewed portable source
    /// kind and therefore fail closed here.
    pub fn output_realization_advertisement(
        &self,
        host_id: HostId,
    ) -> Result<RealizationAdvertisement, &'static str> {
        if self.observation.direction != MidiEndpointDirection::WritableDestination {
            return Err("a readable MIDI source cannot realize music/play output");
        }
        let profile = output_compatibility_profile()?;
        let mut characteristics = conduit_semantic_catalog::sound_profile_characteristics(&profile);
        characteristics.extend([
            label(MIDI_DIRECTION_CHARACTERISTIC, "writable-destination"),
            label(
                MIDI_RESOURCE_CHARACTERISTIC,
                self.resource_pool_id().as_str(),
            ),
            label(MIDI_BACKEND_CHARACTERISTIC, "alsa-sequencer-midi1@1"),
            count(MIDI_CHANNEL_CHARACTERISTIC, u64::from(OUTPUT_CHANNEL)),
            count(MIDI_A4_REFERENCE_CHARACTERISTIC, A4_REFERENCE_MILLIHERTZ),
            count(
                MIDI_PENDING_MESSAGES_CHARACTERISTIC,
                u64::from(MAXIMUM_PENDING_MESSAGES),
            ),
            count(
                MIDI_PENDING_BYTES_CHARACTERISTIC,
                u64::from(MAXIMUM_PENDING_BYTES),
            ),
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
                "wait-without-consumption",
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

fn count(id: &str, value: u64) -> RealizationCharacteristic {
    conduit_core::stable_realization_quantity(
        id,
        id,
        "Stable reviewed MIDI realization quantity.",
        conduit_semantic_catalog::sound_characteristic_unit(id),
        u64::MAX,
        value,
    )
}

fn label(id: &str, value: &str) -> RealizationCharacteristic {
    conduit_core::stable_realization_category(
        id,
        id,
        "Stable reviewed MIDI realization category.",
        vec![value.into()],
        false,
        value,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_profile_is_reusable_without_claiming_an_endpoint() {
        let profile = output_compatibility_profile().unwrap();
        assert_eq!(
            profile.seam,
            conduit_semantic_catalog::SoundSeam::MusicalEvents
        );
        assert_eq!(profile.maximum_polyphony, 128);
        assert!(profile.preserves_velocity);
        assert!(!profile.accepts_microtonal_pitch);
    }

    #[test]
    fn selection_is_exact_directional_and_boot_scoped() {
        let observations = [
            MidiEndpointObservation {
                client: 20,
                port: 1,
                client_name: "Controller".into(),
                port_name: "Port".into(),
                client_type: "kernel".into(),
                direction: MidiEndpointDirection::ReadableSource,
            },
            MidiEndpointObservation {
                client: 20,
                port: 1,
                client_name: "Controller".into(),
                port_name: "Port".into(),
                client_type: "kernel".into(),
                direction: MidiEndpointDirection::WritableDestination,
            },
        ];
        let selected = HostedMidiSelection::select(
            &observations,
            MidiEndpointDirection::ReadableSource,
            20,
            1,
            BootId::from("boot-a"),
            OfferGeneration(4),
        )
        .unwrap();
        assert_eq!(selected.sequencer_address(), "20:1");
        assert_eq!(
            selected.resource_pool_id().as_str(),
            "std/midi/alsa-seq/boot-a/4/readable-source/client-20/port-1"
        );
        assert!(HostedMidiSelection::select(
            &observations,
            MidiEndpointDirection::WritableDestination,
            20,
            2,
            BootId::from("boot-a"),
            OfferGeneration(4),
        )
        .is_err());

        let advertisement = HostedMidiSelection::select(
            &observations,
            MidiEndpointDirection::WritableDestination,
            20,
            1,
            BootId::from("boot-a"),
            OfferGeneration(4),
        )
        .unwrap()
        .output_realization_advertisement(HostId::from("host-a"))
        .unwrap();
        assert_eq!(advertisement.capability_id.as_str(), "music-play-midi1");
        assert!(advertisement.characteristics.iter().any(|fact| {
            fact.definition.characteristic_id.as_str() == MIDI_RESOURCE_CHARACTERISTIC
                && fact.value
                    == conduit_core::CharacteristicValue::Categorical(
                        "std/midi/alsa-seq/boot-a/4/writable-destination/client-20/port-1".into(),
                    )
        }));
        assert!(selected
            .output_realization_advertisement(HostId::from("host-a"))
            .is_err());

        let output = HostedMidiSelection::select(
            &observations,
            MidiEndpointDirection::WritableDestination,
            20,
            1,
            BootId::from("boot-a"),
            OfferGeneration(4),
        )
        .unwrap();
        let host = crate::StdHost::new_with_midi_output(
            crate::StdHostConfig {
                host_id: HostId::from("host-a"),
                boot_id: BootId::from("boot-a"),
                offer_generation: OfferGeneration(4),
            },
            crate::StdHostComposition::minimal(),
            output.clone(),
        )
        .unwrap();
        assert!(host.advertisement().capabilities.iter().any(|capability| {
            capability.implementation.execution_profile_id.as_str()
                == conduit_std_offers::MUSIC_PLAY_MIDI_PROFILE
        }));
        assert!(host.advertisement().resources.iter().any(|resource| {
            resource.pool_id == output.resource_pool_id()
                && resource.class_id.as_str() == conduit_std_offers::MIDI_OUTPUT_RESOURCE_CLASS
        }));
        assert!(crate::StdHost::new_with_midi_output(
            crate::StdHostConfig {
                host_id: HostId::from("host-a"),
                boot_id: BootId::from("boot-new"),
                offer_generation: OfferGeneration(4),
            },
            crate::StdHostComposition::minimal(),
            output,
        )
        .is_err());
    }
}
