//! Exact Create 1 Open Interface speaker contract.
//!
//! The constants below are pinned to the original Create OI v2 specification,
//! not the later Create 2 contract. Encoding is mechanism-side; portable Forms
//! continue to use `music/play` meaning.

use conduit_core::{
    kind_id, resource_offer, resource_requirement, ArtifactId, AuthorityContractId,
    AuthorityRequirement, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    OfferGeneration, RealizationAdvertisement, PROTOCOL_VERSION,
};

pub const SONG_OPCODE: u8 = 140;
pub const PLAY_SONG_OPCODE: u8 = 141;
pub const DRIVE_OPCODE: u8 = 137;
pub const DRIVE_DIRECT_OPCODE: u8 = 145;
pub const MINIMUM_NOTE: u8 = 31;
pub const MAXIMUM_NOTE: u8 = 127;
pub const MAXIMUM_SONGS: u8 = 16;
pub const MAXIMUM_NOTES: usize = 16;
pub const DURATION_TICKS_PER_SECOND: u16 = 64;
pub const MAXIMUM_DURATION_TICKS: u8 = 255;
pub const MAXIMUM_SONG_COMMAND_BYTES: usize = 3 + 2 * MAXIMUM_NOTES;
pub const MAXIMUM_PLAY_COMMAND_BYTES: usize = 2;
pub const MAXIMUM_ADMITTED_SERIAL_BYTES: usize =
    MAXIMUM_SONG_COMMAND_BYTES + MAXIMUM_PLAY_COMMAND_BYTES;

pub const SPEAKER_CAPABILITY: &str = "pete/create1-music-play@1";
pub const SPEAKER_PROFILE: &str = "pete/create1-oi-song-monophonic-64hz@1";
pub const SPEAKER_IMPLEMENTATION: &str = "pete/create1-oi-song-play@1";
pub const SPEAKER_ARTIFACT: &str = "pete/create1-oi-speaker@1";
pub const SPEAKER_OPERATION: &str = "pete.host/create1-oi-speaker-song-play@1";
pub const SPEAKER_AUTHORITY: &str = "pete.authority/create1-speaker-only@1";
pub const SPEAKER_RESOURCE: &str = "pete.resource/create1-speaker@1";
pub const SERIAL_OPERATION_RESOURCE: &str = "pete.resource/create1-serial-operation@1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OiMode {
    Off,
    Passive,
    Safe,
    Full,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateSpeakerObservation {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub serial_base_id: String,
    pub robot_identity: String,
    /// UART presence is not identity. This may be true only for an explicit
    /// current physical attestation or a stronger device identity mechanism.
    pub robot_identity_verified: bool,
    pub speaker_resource_id: String,
    pub mode: OiMode,
    pub currently_usable: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OiPitch {
    Note(u8),
    Rest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OiSongEvent {
    pub pitch: OiPitch,
    pub duration_ticks: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EncodedSong {
    pub define: Vec<u8>,
    pub play: [u8; MAXIMUM_PLAY_COMMAND_BYTES],
    pub admitted_serial_bytes: usize,
    pub maximum_completion_ticks: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpeakerRefusal {
    MissingIdentity,
    UnverifiedIdentity,
    NotCurrentlyUsable,
    UnsupportedMode,
    InvalidSongNumber,
    EmptySong,
    SongCapacityExceeded,
    PitchOutOfRange,
    DurationOutOfRange,
    SerialPressure,
    OpcodeOutsideSpeakerAuthority,
}

pub fn encode_song(
    song_number: u8,
    events: &[OiSongEvent],
    admitted_serial_bytes: usize,
) -> Result<EncodedSong, SpeakerRefusal> {
    if song_number >= MAXIMUM_SONGS {
        return Err(SpeakerRefusal::InvalidSongNumber);
    }
    if events.is_empty() {
        return Err(SpeakerRefusal::EmptySong);
    }
    if events.len() > MAXIMUM_NOTES {
        return Err(SpeakerRefusal::SongCapacityExceeded);
    }
    let required = 3 + events.len() * 2 + MAXIMUM_PLAY_COMMAND_BYTES;
    if required > admitted_serial_bytes {
        return Err(SpeakerRefusal::SerialPressure);
    }
    let mut define = Vec::with_capacity(3 + events.len() * 2);
    define.extend_from_slice(&[SONG_OPCODE, song_number, events.len() as u8]);
    let mut completion_ticks = 0_u16;
    for event in events {
        if event.duration_ticks == 0 {
            return Err(SpeakerRefusal::DurationOutOfRange);
        }
        let note = match event.pitch {
            OiPitch::Note(note) if (MINIMUM_NOTE..=MAXIMUM_NOTE).contains(&note) => note,
            OiPitch::Note(_) => return Err(SpeakerRefusal::PitchOutOfRange),
            // The Create contract interprets values outside 31..=127 as rests.
            // Zero is the one reviewed canonical rest encoding.
            OiPitch::Rest => 0,
        };
        define.extend_from_slice(&[note, event.duration_ticks]);
        completion_ticks = completion_ticks
            .checked_add(u16::from(event.duration_ticks))
            .ok_or(SpeakerRefusal::DurationOutOfRange)?;
    }
    Ok(EncodedSong {
        define,
        play: [PLAY_SONG_OPCODE, song_number],
        admitted_serial_bytes: required,
        maximum_completion_ticks: completion_ticks,
    })
}

/// Speaker authority admits only complete Song and Play Song operations. It
/// cannot be used as an arbitrary OI byte channel and cannot issue motion.
pub fn speaker_authority_admits(command: &[u8]) -> bool {
    match command.first().copied() {
        Some(SONG_OPCODE) => {
            command.len() >= 5
                && command.len() <= MAXIMUM_SONG_COMMAND_BYTES
                && command.get(2).copied().is_some_and(|count| {
                    (1..=MAXIMUM_NOTES as u8).contains(&count)
                        && command.len() == 3 + usize::from(count) * 2
                })
        }
        Some(PLAY_SONG_OPCODE) => command.len() == MAXIMUM_PLAY_COMMAND_BYTES,
        _ => false,
    }
}

pub fn live_speaker_advertisement(
    observation: &CreateSpeakerObservation,
) -> Result<HostAdvertisement, SpeakerRefusal> {
    if observation.serial_base_id.is_empty()
        || observation.robot_identity.is_empty()
        || observation.speaker_resource_id.is_empty()
    {
        return Err(SpeakerRefusal::MissingIdentity);
    }
    if !observation.robot_identity_verified {
        return Err(SpeakerRefusal::UnverifiedIdentity);
    }
    if !observation.currently_usable {
        return Err(SpeakerRefusal::NotCurrentlyUsable);
    }
    if !matches!(observation.mode, OiMode::Safe | OiMode::Full) {
        return Err(SpeakerRefusal::UnsupportedMode);
    }
    let contract = conduit_semantic_catalog::music_play_contract();
    let mut resources = vec![
        resource_offer(&observation.speaker_resource_id, SPEAKER_RESOURCE, 1),
        resource_offer(
            &format!("{}/speaker-operation", observation.serial_base_id),
            SERIAL_OPERATION_RESOURCE,
            1,
        ),
    ];
    resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let mut requirements = vec![
        resource_requirement(SPEAKER_RESOURCE, 1),
        resource_requirement(SERIAL_OPERATION_RESOURCE, 1),
    ];
    requirements.sort_by(|left, right| left.class_id.cmp(&right.class_id));
    Ok(HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: observation.host_id.clone(),
        boot_id: observation.boot_id.clone(),
        offer_generation: observation.offer_generation,
        profile: conduit_core::HostProfileId::from(SPEAKER_PROFILE),
        resources,
        capabilities: vec![CapabilityOffer {
            startup_parameters: Vec::new(),
            shorthand: None,
            capability_id: CapabilityId::from(SPEAKER_CAPABILITY),
            kind_id: contract.kind_id,
            kind_contract_revision: KindContractRevision::from(
                conduit_semantic_catalog::MUSIC_PLAY_REVISION,
            ),
            implementation: ImplementationOffer {
                execution_profile_id: ExecutionProfileId::from(SPEAKER_PROFILE),
                implementation_id: ImplementationId::from(SPEAKER_IMPLEMENTATION),
                artifact_id: ArtifactId::from(SPEAKER_ARTIFACT),
            },
            inputs: contract.inputs,
            outputs: contract.outputs,
            host_operations: vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(SPEAKER_OPERATION),
                target_kind: Some(kind_id(conduit_audio::MUSIC_NOTE_INFO_ID)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_ADMITTED_SERIAL_BYTES as u32,
                maximum_output_bytes: 0,
            }],
            resource_requirements: requirements,
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(SPEAKER_AUTHORITY),
                host_operation_contract_id: HostOperationContractId::from(SPEAKER_OPERATION),
                subject_kind: kind_id(conduit_audio::MUSIC_NOTE_INFO_ID),
            }],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: MAXIMUM_NOTES as u16,
                max_queue_bytes: MAXIMUM_ADMITTED_SERIAL_BYTES as u32,
            },
        }],
        planner_capabilities: vec![],
    })
}

pub fn compatibility_profile() -> conduit_semantic_catalog::SoundCompatibilityProfile {
    let minimum = conduit_audio::MusicalPitch::from_equal_tempered(-38, 440_000, 0)
        .expect("Create note 31 is representable");
    let maximum = conduit_audio::MusicalPitch::from_equal_tempered(58, 440_000, 0)
        .expect("Create note 127 is representable");
    conduit_semantic_catalog::SoundCompatibilityProfile {
        profile_id: SPEAKER_PROFILE.into(),
        seam: conduit_semantic_catalog::SoundSeam::MusicalEvents,
        minimum_pitch_millihertz: minimum.frequency_millihertz,
        maximum_pitch_millihertz: maximum.frequency_millihertz,
        maximum_polyphony: 1,
        maximum_events_per_second: DURATION_TICKS_PER_SECOND as u32,
        preserves_velocity: false,
        preserves_sustain: false,
        preserves_pitch_bend: false,
        maximum_pitch_bend_range_microcents: 0,
        preserves_modulation: false,
        accepts_microtonal_pitch: false,
        supports_subtractive_filter: false,
        pcm: None,
    }
}

pub fn live_speaker_realization(
    observation: &CreateSpeakerObservation,
) -> Result<RealizationAdvertisement, SpeakerRefusal> {
    let host = live_speaker_advertisement(observation)?;
    Ok(RealizationAdvertisement {
        host_id: host.host_id,
        boot_id: host.boot_id,
        offer_generation: host.offer_generation,
        capability_id: CapabilityId::from(SPEAKER_CAPABILITY),
        characteristics: conduit_semantic_catalog::sound_profile_characteristics(
            &compatibility_profile(),
        ),
    })
}

#[cfg(test)]
mod tests;
