//! Boot-scoped truthful direct-musical offer for an exact YM3812 Base.

use alloc::{format, vec, vec::Vec};
use conduit_audio::NOTE_EVENT_ENCODED_LEN;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostAdvertisement, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, resource_offer,
};

pub const OPL2_IMPLEMENTATION: &str = "conduitos/opl2-fixed-fm-music@1";
pub const OPL2_EXECUTION_PROFILE: &str = "conduitos/opl2-nine-voice-fixed-patch@1";
pub const OPL2_PATCH_PROFILE: &str = "conduitos/opl2-patch-bright-organ@1";
pub const OPL2_HOST_OPERATION: &str = "conduitos.host/opl2-note@1";
pub const OPL2_BASE_RESOURCE: &str = "conduitos.resource/opl2-base@1";
pub const OPL2_VOICE_RESOURCE: &str = "conduitos.resource/opl2-voice@1";
pub const OPL2_EVENT_RESOURCE: &str = "conduitos.resource/opl2-event-slot@1";
pub const OPL2_WRITE_RESOURCE: &str = "conduitos.resource/opl2-register-write@1";
pub const OPL2_STATE_BYTES: u32 = 2_048;
pub const OPL2_CAPABILITY: &str = "conduitos/music-play-opl2@1";
pub const OPL2_CLOCK_HZ: u64 = 3_579_545;
pub const OPL2_CHANNELS: u16 = 9;
pub const OPL2_WRITES_PER_NOTE: u16 = 2;
pub const OPL2_PATCH_WRITES_PER_CHANNEL: u16 = 11;
pub const OPL2_RESET_WRITES: u16 = 245;
pub const OPL2_MINIMUM_PITCH_MILLIHERTZ: u64 = 16_000;
pub const OPL2_MAXIMUM_PITCH_MILLIHERTZ: u64 = 6_200_000;
pub const OPL2_MAXIMUM_EVENTS_PER_SECOND: u32 = 1_000;

pub fn compatibility_profile() -> conduit_semantic_catalog::SoundCompatibilityProfile {
    conduit_semantic_catalog::SoundCompatibilityProfile {
        profile_id: OPL2_EXECUTION_PROFILE.into(),
        seam: conduit_semantic_catalog::SoundSeam::MusicalEvents,
        minimum_pitch_millihertz: OPL2_MINIMUM_PITCH_MILLIHERTZ,
        maximum_pitch_millihertz: OPL2_MAXIMUM_PITCH_MILLIHERTZ,
        maximum_polyphony: OPL2_CHANNELS,
        maximum_events_per_second: OPL2_MAXIMUM_EVENTS_PER_SECOND,
        preserves_velocity: false,
        preserves_sustain: false,
        preserves_pitch_bend: false,
        maximum_pitch_bend_range_microcents: 0,
        preserves_modulation: false,
        accepts_microtonal_pitch: true,
        supports_subtractive_filter: false,
        pcm: None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Opl2Realization {
    pub base_id: [u8; 32],
    pub clock_hz: u64,
    pub channels: u16,
    pub maximum_error_parts_per_million: u32,
    pub event_slots: u16,
    pub register_write_slots: u16,
    pub patch_profile: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Opl2Offer<'a> {
    pub artifact_build: &'a str,
    pub realization: Opl2Realization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Opl2OfferError {
    EmptyIdentity,
    ArtifactMismatch,
    InvalidClock,
    InvalidProfile,
    InvalidCapacity,
}

impl Opl2Realization {
    pub fn validate(self) -> Result<(), Opl2OfferError> {
        if self.base_id == [0; 32] {
            return Err(Opl2OfferError::EmptyIdentity);
        }
        if self.clock_hz != OPL2_CLOCK_HZ {
            return Err(Opl2OfferError::InvalidClock);
        }
        if self.patch_profile != OPL2_PATCH_PROFILE {
            return Err(Opl2OfferError::InvalidProfile);
        }
        let mandatory_writes = OPL2_RESET_WRITES
            + OPL2_CHANNELS * OPL2_PATCH_WRITES_PER_CHANNEL
            + self.event_slots * OPL2_WRITES_PER_NOTE
            + OPL2_CHANNELS;
        if self.channels != OPL2_CHANNELS
            || self.event_slots == 0
            || self.maximum_error_parts_per_million == 0
            || self.register_write_slots < mandatory_writes
        {
            return Err(Opl2OfferError::InvalidCapacity);
        }
        Ok(())
    }
}

impl Opl2Offer<'_> {
    pub fn validate(self, expected_build: &str) -> Result<(), Opl2OfferError> {
        if self.artifact_build.is_empty() || self.artifact_build != expected_build {
            return Err(Opl2OfferError::ArtifactMismatch);
        }
        self.realization.validate()
    }
}

pub fn append_to_advertisement(
    advertisement: &mut HostAdvertisement,
    offer: Opl2Offer<'_>,
    build_id: &str,
) -> Result<(), Opl2OfferError> {
    offer.validate(build_id)?;
    let realization = offer.realization;
    let base = crate::identity::hex(&realization.base_id);
    for (suffix, class, capacity) in [
        ("base", OPL2_BASE_RESOURCE, 1_u32),
        (
            "voices",
            OPL2_VOICE_RESOURCE,
            u32::from(realization.channels),
        ),
        (
            "events",
            OPL2_EVENT_RESOURCE,
            u32::from(realization.event_slots),
        ),
        (
            "writes",
            OPL2_WRITE_RESOURCE,
            u32::from(realization.register_write_slots),
        ),
    ] {
        advertisement.resources.push(resource_offer(
            &format!("conduitos-opl2-{suffix}-{base}"),
            class,
            capacity,
        ));
    }
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let contract = conduit_semantic_catalog::music_play_contract();
    let mut requirements = vec![
        conduit_core::resource_requirement(
            conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
            OPL2_STATE_BYTES,
        ),
        conduit_core::resource_requirement(OPL2_BASE_RESOURCE, 1),
        conduit_core::resource_requirement(OPL2_VOICE_RESOURCE, u32::from(realization.channels)),
        conduit_core::resource_requirement(OPL2_EVENT_RESOURCE, u32::from(realization.event_slots)),
        conduit_core::resource_requirement(
            OPL2_WRITE_RESOURCE,
            u32::from(realization.register_write_slots),
        ),
    ];
    requirements.sort();
    advertisement.capabilities.push(CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(OPL2_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::MUSIC_PLAY_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(OPL2_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(OPL2_IMPLEMENTATION),
            artifact_id: ArtifactId::from(format!("conduitos-build/{build_id}")),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(OPL2_HOST_OPERATION),
            target_kind: Some(conduit_core::kind_id(conduit_audio::MUSIC_NOTE_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: NOTE_EVENT_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: requirements,
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: realization.event_slots,
            max_queue_bytes: u32::from(realization.event_slots) * NOTE_EVENT_ENCODED_LEN as u32,
        },
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn realization() -> Opl2Realization {
        Opl2Realization {
            base_id: [7; 32],
            clock_hz: OPL2_CLOCK_HZ,
            channels: OPL2_CHANNELS,
            maximum_error_parts_per_million: 2_500,
            event_slots: 32,
            register_write_slots: 512,
            patch_profile: OPL2_PATCH_PROFILE,
        }
    }

    #[test]
    fn exact_clock_patch_channels_and_work_budget_are_required() {
        assert_eq!(realization().validate(), Ok(()));
        let mut invalid = realization();
        invalid.base_id = [0; 32];
        assert_eq!(invalid.validate(), Err(Opl2OfferError::EmptyIdentity));
        let mut invalid = realization();
        invalid.channels = 8;
        assert_eq!(invalid.validate(), Err(Opl2OfferError::InvalidCapacity));
        let mut invalid = realization();
        invalid.register_write_slots = 400;
        assert_eq!(invalid.validate(), Err(Opl2OfferError::InvalidCapacity));
        let mut invalid = realization();
        invalid.patch_profile = "raw-registers";
        assert_eq!(invalid.validate(), Err(Opl2OfferError::InvalidProfile));
        let profile = compatibility_profile();
        assert_eq!(profile.maximum_polyphony, 9);
        assert!(!profile.preserves_velocity);
        assert!(!profile.supports_subtractive_filter);
    }
}
