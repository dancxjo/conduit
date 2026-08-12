//! Explicit, boot-scoped hosted PCM playback discovery and realization.
//!
//! Discovery never opens a PCM handle. A selected observation becomes a Host
//! resource offer; an independent authority grant is still required before a
//! Plan can carry the playback operation.

mod alsa_aplay;
mod discovery;
#[cfg(test)]
mod fake;
mod proof;
#[cfg(test)]
pub(crate) use fake::FakePlaybackBehavior;

pub use alsa_aplay::{
    AlsaAplaySession, PlaybackFailure, PlaybackLifecycle, PlaybackMetrics, PlaybackReport,
};
pub use discovery::{discover_alsa_playback, AlsaPlaybackObservation, PlaybackDiscoveryError};
pub use proof::{run_playback_proof, ExplicitPlaybackAuthorization, PlaybackProofReceipt};

use conduit_core::{
    BootId, CapabilityId, HostId, OfferGeneration, RealizationAdvertisement,
    RealizationCharacteristic, RealizationCharacteristicId, RealizationCharacteristicValue,
    ResourceHealth, ResourceObservation, ResourcePoolId, SignId,
};

pub const SAMPLE_RATE_HZ: u32 = 48_000;
pub const CHANNELS: u8 = 2;
pub const PERIOD_FRAMES: u16 = conduit_std_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES;
pub const BUFFER_FRAMES: u16 = conduit_std_catalog::AUDIO_PLAY_ALSA_BUFFER_FRAMES;
pub const SOURCE_CLOCK_ID: u64 = 1;

/// Exact PCM profile of the reviewed direct ALSA hardware adapter. Device
/// presence, identity, and authority remain fresh observation facts.
pub fn compatibility_profile() -> conduit_std_catalog::SoundCompatibilityProfile {
    use conduit_std_catalog::*;
    SoundCompatibilityProfile {
        profile_id: AUDIO_PLAY_ALSA_HW_PROFILE.into(),
        seam: SoundSeam::PcmPlayback,
        minimum_pitch_millihertz: 0,
        maximum_pitch_millihertz: 0,
        maximum_polyphony: 0,
        maximum_events_per_second: 0,
        preserves_velocity: false,
        preserves_sustain: false,
        preserves_pitch_bend: false,
        maximum_pitch_bend_range_microcents: 0,
        preserves_modulation: false,
        accepts_microtonal_pitch: false,
        supports_subtractive_filter: false,
        pcm: Some(PcmCompatibilityProfile {
            representation: conduit_core::PcmSampleRepresentation::Signed16LittleEndian,
            sample_rate_hz: SAMPLE_RATE_HZ,
            layout: conduit_core::PcmChannelLayout::StereoLeftRight,
            maximum_frames_per_block: PERIOD_FRAMES,
            maximum_frame_bytes: AUDIO_PLAY_ALSA_PERIOD_FRAMES as u32 * AUDIO_PLAY_ALSA_FRAME_BYTES,
        }),
    }
}

/// One user-selected discovery result. The constructor is deliberately fed by
/// a current observation rather than an ambient default-device lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostedPlaybackSelection {
    pub observation: AlsaPlaybackObservation,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    #[cfg(test)]
    fake_behavior: Option<fake::FakePlaybackBehavior>,
}

impl HostedPlaybackSelection {
    pub fn from_observation(
        observation: AlsaPlaybackObservation,
        boot_id: BootId,
        offer_generation: OfferGeneration,
    ) -> Self {
        Self {
            observation,
            boot_id,
            offer_generation,
            #[cfg(test)]
            fake_behavior: None,
        }
    }

    pub fn pool_id(&self) -> ResourcePoolId {
        ResourcePoolId::from(format!(
            "std/audio/alsa/{}/card-{}/device-{}",
            self.observation.base_identity, self.observation.card_id, self.observation.device
        ))
    }

    pub fn alsa_target(&self) -> String {
        format!(
            "hw:CARD={},DEV={}",
            self.observation.card_id, self.observation.device
        )
    }

    pub fn resource_observation(&self, host_id: HostId, sign_id: SignId) -> ResourceObservation {
        ResourceObservation {
            host_id,
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            pool_id: self.pool_id(),
            class_id: conduit_core::ResourceClassId::from(
                conduit_std_catalog::AUDIO_PLAYBACK_RESOURCE_CLASS,
            ),
            health: ResourceHealth::Ready,
            unreserved_units: 1,
            utilized_units: 0,
            sign_id,
        }
    }

    pub fn realization_advertisement(&self, host_id: HostId) -> RealizationAdvertisement {
        use conduit_std_catalog::*;
        let profile = compatibility_profile();
        let mut characteristics = sound_profile_characteristics(&profile);
        characteristics.extend([
            count(AUDIO_PERIOD_FRAMES_CHARACTERISTIC, u64::from(PERIOD_FRAMES)),
            count(AUDIO_BUFFER_FRAMES_CHARACTERISTIC, u64::from(BUFFER_FRAMES)),
            count(
                AUDIO_MAXIMUM_BLOCKS_CHARACTERISTIC,
                u64::from(AUDIO_PLAY_ALSA_MAXIMUM_BLOCKS),
            ),
            count(AUDIO_SOURCE_CLOCK_ID_CHARACTERISTIC, SOURCE_CLOCK_ID),
            label(
                AUDIO_DEVICE_CLOCK_CHARACTERISTIC,
                &format!("alsa-hw:{}/playback", self.pool_id().as_str()),
            ),
            label(
                AUDIO_PLAYBACK_RESOURCE_CHARACTERISTIC,
                self.pool_id().as_str(),
            ),
            label(AUDIO_BACKEND_CHARACTERISTIC, "alsa-aplay-direct-hw@1"),
            label(
                AUDIO_STARTUP_POLICY_CHARACTERISTIC,
                "start-on-first-committed-frame",
            ),
            label(
                AUDIO_DRAIN_POLICY_CHARACTERISTIC,
                "drain-on-input-close-stop-on-cancel",
            ),
            label(
                AUDIO_TIMING_CLASS_CHARACTERISTIC,
                "hosted-best-effort-measured",
            ),
            count(AUDIO_CONTROLLED_STAGING_BYTES_CHARACTERISTIC, 0),
        ]);
        characteristics.sort();
        RealizationAdvertisement {
            host_id,
            boot_id: self.boot_id.clone(),
            offer_generation: self.offer_generation,
            capability_id: CapabilityId::from("audio-play-alsa-hw"),
            characteristics,
        }
    }

    #[cfg(test)]
    pub(crate) fn deterministic_fake(
        observation: AlsaPlaybackObservation,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        behavior: fake::FakePlaybackBehavior,
    ) -> Self {
        Self {
            observation,
            boot_id,
            offer_generation,
            fake_behavior: Some(behavior),
        }
    }
}

fn count(id: &str, value: u64) -> RealizationCharacteristic {
    characteristic(id, RealizationCharacteristicValue::Count(value))
}

fn label(id: &str, value: &str) -> RealizationCharacteristic {
    characteristic(id, RealizationCharacteristicValue::Label(value.into()))
}

fn characteristic(id: &str, value: RealizationCharacteristicValue) -> RealizationCharacteristic {
    RealizationCharacteristic {
        characteristic_id: RealizationCharacteristicId::from(id),
        value,
    }
}

pub(crate) enum PlaybackSession {
    Alsa(AlsaAplaySession),
    #[cfg(test)]
    Fake(fake::FakePlaybackSession),
}

impl PlaybackSession {
    pub(crate) fn resolved(selection: HostedPlaybackSelection) -> Self {
        #[cfg(test)]
        if let Some(behavior) = selection.fake_behavior {
            return Self::Fake(fake::FakePlaybackSession::new(selection, behavior));
        }
        Self::Alsa(AlsaAplaySession::resolved(selection))
    }

    pub(crate) fn write_frame(&mut self, encoded: &[u8]) -> Result<(), PlaybackFailure> {
        match self {
            Self::Alsa(session) => session.write_frame(encoded),
            #[cfg(test)]
            Self::Fake(session) => session.write_frame(encoded),
        }
    }

    pub(crate) fn drain(&mut self) -> Result<(), PlaybackFailure> {
        match self {
            Self::Alsa(session) => session.drain(),
            #[cfg(test)]
            Self::Fake(session) => session.drain(),
        }
    }

    pub(crate) fn stop(&mut self) -> Result<(), PlaybackFailure> {
        match self {
            Self::Alsa(session) => session.stop(),
            #[cfg(test)]
            Self::Fake(session) => session.stop(),
        }
    }

    pub(crate) fn lifecycle(&self) -> PlaybackLifecycle {
        match self {
            Self::Alsa(session) => session.lifecycle(),
            #[cfg(test)]
            Self::Fake(session) => session.lifecycle(),
        }
    }

    pub(crate) fn report(&self) -> PlaybackReport {
        match self {
            Self::Alsa(session) => session.report(),
            #[cfg(test)]
            Self::Fake(session) => session.report(),
        }
    }
}

#[cfg(test)]
mod profile_tests {
    use super::*;

    #[test]
    fn playback_profile_is_reusable_without_claiming_a_device() {
        let profile = compatibility_profile();
        assert_eq!(profile.seam, conduit_std_catalog::SoundSeam::PcmPlayback);
        let pcm = profile.pcm.unwrap();
        assert_eq!(pcm.sample_rate_hz, SAMPLE_RATE_HZ);
        assert_eq!(pcm.maximum_frames_per_block, PERIOD_FRAMES);
    }
}
