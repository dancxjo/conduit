//! Boot-scoped truthful tone offer for the legacy x86 PC-speaker Base.

use alloc::{format, vec, vec::Vec};
use conduit_audio::TONE_INTENT_ENCODED_LEN;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostAdvertisement, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, KindContractRevision, resource_offer,
};

pub const PC_SPEAKER_IMPLEMENTATION: &str = "conduitos/pc-speaker-tone@1";
pub const PC_SPEAKER_EXECUTION_PROFILE: &str = "conduitos/pc-speaker-pit2-monophonic@1";
pub const PC_SPEAKER_HOST_OPERATION: &str = "conduitos.host/pc-speaker-tone@1";
pub const PC_SPEAKER_BASE_RESOURCE: &str = "conduitos.resource/pc-speaker-base@1";
pub const PC_SPEAKER_EVENT_RESOURCE: &str = "conduitos.resource/pc-speaker-event-slot@1";
pub const PC_SPEAKER_OPERATION_RESOURCE: &str = "conduitos.resource/pc-speaker-operation-slot@1";
pub const PC_SPEAKER_STATE_BYTES: u32 = 512;
pub const PC_SPEAKER_CAPABILITY: &str = "conduitos/sound-tone-pc-speaker@1";
pub const PC_SPEAKER_PIT_INPUT_HZ: u64 = 1_193_182;

pub fn compatibility_profile(
    realization: PcSpeakerRealization,
) -> Result<conduit_semantic_catalog::SoundCompatibilityProfile, PcSpeakerOfferError> {
    realization.validate()?;
    let clock_millihertz = realization
        .pit_input_hz
        .checked_mul(1_000)
        .ok_or(PcSpeakerOfferError::InvalidClock)?;
    let minimum_pitch_millihertz = clock_millihertz
        .div_ceil(u64::from(realization.maximum_divisor))
        .max(conduit_audio::MINIMUM_PITCH_MILLIHERTZ);
    let maximum_pitch_millihertz = (clock_millihertz / u64::from(realization.minimum_divisor))
        .min(conduit_audio::MAXIMUM_PITCH_MILLIHERTZ);
    Ok(conduit_semantic_catalog::SoundCompatibilityProfile {
        profile_id: PC_SPEAKER_EXECUTION_PROFILE.into(),
        seam: conduit_semantic_catalog::SoundSeam::Tone,
        minimum_pitch_millihertz,
        maximum_pitch_millihertz,
        maximum_polyphony: 1,
        maximum_events_per_second: 0,
        preserves_velocity: false,
        preserves_sustain: false,
        preserves_pitch_bend: false,
        maximum_pitch_bend_range_microcents: 0,
        preserves_modulation: false,
        accepts_microtonal_pitch: false,
        supports_subtractive_filter: false,
        pcm: None,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcSpeakerRealization {
    pub base_id: [u8; 32],
    pub pit_input_hz: u64,
    pub minimum_divisor: u16,
    pub maximum_divisor: u16,
    pub maximum_error_parts_per_million: u32,
    pub event_slots: u16,
    pub operation_slots: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcSpeakerOffer<'a> {
    pub artifact_build: &'a str,
    pub realization: PcSpeakerRealization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcSpeakerOfferError {
    EmptyIdentity,
    ArtifactMismatch,
    InvalidClock,
    InvalidDivisorRange,
    InvalidCapacity,
}

impl PcSpeakerRealization {
    pub fn validate(self) -> Result<(), PcSpeakerOfferError> {
        if self.base_id == [0; 32] {
            return Err(PcSpeakerOfferError::EmptyIdentity);
        }
        if self.pit_input_hz != PC_SPEAKER_PIT_INPUT_HZ {
            return Err(PcSpeakerOfferError::InvalidClock);
        }
        if self.minimum_divisor == 0 || self.minimum_divisor > self.maximum_divisor {
            return Err(PcSpeakerOfferError::InvalidDivisorRange);
        }
        if self.maximum_error_parts_per_million == 0
            || self.event_slots == 0
            || self.operation_slots != 1
        {
            return Err(PcSpeakerOfferError::InvalidCapacity);
        }
        Ok(())
    }
}

impl PcSpeakerOffer<'_> {
    pub fn validate(self, expected_build: &str) -> Result<(), PcSpeakerOfferError> {
        if self.artifact_build.is_empty() || self.artifact_build != expected_build {
            return Err(PcSpeakerOfferError::ArtifactMismatch);
        }
        self.realization.validate()
    }
}

pub(crate) fn append_to_advertisement(
    advertisement: &mut HostAdvertisement,
    offer: PcSpeakerOffer<'_>,
    build_id: &str,
) -> Result<(), PcSpeakerOfferError> {
    offer.validate(build_id)?;
    let realization = offer.realization;
    let base = crate::identity::hex(&realization.base_id);
    for (suffix, class, capacity) in [
        ("base", PC_SPEAKER_BASE_RESOURCE, 1_u32),
        (
            "events",
            PC_SPEAKER_EVENT_RESOURCE,
            u32::from(realization.event_slots),
        ),
        (
            "operation",
            PC_SPEAKER_OPERATION_RESOURCE,
            u32::from(realization.operation_slots),
        ),
    ] {
        advertisement.resources.push(resource_offer(
            &format!("conduitos-pc-speaker-{suffix}-{base}"),
            class,
            capacity,
        ));
    }
    advertisement
        .resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let contract = conduit_semantic_catalog::sound_tone_play_contract();
    advertisement.capabilities.push(CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(PC_SPEAKER_CAPABILITY),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::SOUND_TONE_PLAY_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PC_SPEAKER_EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(PC_SPEAKER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(format!("conduitos-build/{build_id}")),
        },
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(PC_SPEAKER_HOST_OPERATION),
            target_kind: Some(conduit_core::kind_id(conduit_audio::SOUND_TONE_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: TONE_INTENT_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![
            conduit_core::resource_requirement(
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
                PC_SPEAKER_STATE_BYTES,
            ),
            conduit_core::resource_requirement(PC_SPEAKER_BASE_RESOURCE, 1),
            conduit_core::resource_requirement(
                PC_SPEAKER_EVENT_RESOURCE,
                u32::from(realization.event_slots),
            ),
            conduit_core::resource_requirement(PC_SPEAKER_OPERATION_RESOURCE, 1),
        ],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: realization.event_slots,
            max_queue_bytes: u32::from(realization.event_slots) * TONE_INTENT_ENCODED_LEN as u32,
        },
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn realization() -> PcSpeakerRealization {
        PcSpeakerRealization {
            base_id: [7; 32],
            pit_input_hz: 1_193_182,
            minimum_divisor: 19,
            maximum_divisor: u16::MAX,
            maximum_error_parts_per_million: 2_500,
            event_slots: 8,
            operation_slots: 1,
        }
    }

    #[test]
    fn exact_base_clock_divisor_tolerance_and_capacity_are_required() {
        assert_eq!(realization().validate(), Ok(()));
        let mut invalid = realization();
        invalid.base_id = [0; 32];
        assert_eq!(invalid.validate(), Err(PcSpeakerOfferError::EmptyIdentity));
        let mut invalid = realization();
        invalid.pit_input_hz = 1_000_000;
        assert_eq!(invalid.validate(), Err(PcSpeakerOfferError::InvalidClock));
        let mut invalid = realization();
        invalid.operation_slots = 2;
        assert_eq!(
            invalid.validate(),
            Err(PcSpeakerOfferError::InvalidCapacity)
        );
    }

    #[test]
    fn tone_profile_is_derived_from_the_validated_exact_divisor_envelope() {
        let profile = compatibility_profile(realization()).unwrap();
        assert_eq!(profile.seam, conduit_semantic_catalog::SoundSeam::Tone);
        assert_eq!(profile.minimum_pitch_millihertz, 18_207);
        assert_eq!(
            profile.maximum_pitch_millihertz,
            conduit_audio::MAXIMUM_PITCH_MILLIHERTZ
        );
        assert_eq!(profile.maximum_polyphony, 1);
        assert!(!profile.preserves_velocity);
        assert!(profile.pcm.is_none());
    }
}
