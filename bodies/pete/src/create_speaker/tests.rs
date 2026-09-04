use super::*;

fn observation(mode: OiMode) -> CreateSpeakerObservation {
    CreateSpeakerObservation {
        host_id: HostId::from("pete-std-live"),
        boot_id: BootId::from("pete-std-live-boot"),
        offer_generation: OfferGeneration(4),
        serial_base_id: "pete/create1/serial/0".into(),
        robot_identity: "pete/create1/observed-robot".into(),
        robot_identity_verified: true,
        speaker_resource_id: "pete/create1/speaker".into(),
        mode,
        currently_usable: true,
    }
}

#[test]
fn exact_create1_song_is_finite_and_preserves_notes_and_rests() {
    let events = [
        OiSongEvent {
            pitch: OiPitch::Note(69),
            duration_ticks: 32,
        },
        OiSongEvent {
            pitch: OiPitch::Rest,
            duration_ticks: 8,
        },
        OiSongEvent {
            pitch: OiPitch::Note(72),
            duration_ticks: 64,
        },
    ];
    let encoded = encode_song(3, &events, MAXIMUM_ADMITTED_SERIAL_BYTES).unwrap();
    assert_eq!(encoded.define, [140, 3, 3, 69, 32, 0, 8, 72, 64]);
    assert_eq!(encoded.play, [141, 3]);
    assert_eq!(encoded.admitted_serial_bytes, 11);
    assert_eq!(encoded.maximum_completion_ticks, 104);
    assert!(speaker_authority_admits(&encoded.define));
    assert!(speaker_authority_admits(&encoded.play));
}

#[test]
fn bounds_pressure_and_unrepresentable_pitch_refuse_specifically() {
    let note = OiSongEvent {
        pitch: OiPitch::Note(69),
        duration_ticks: 1,
    };
    assert_eq!(
        encode_song(16, &[note], 64),
        Err(SpeakerRefusal::InvalidSongNumber)
    );
    assert_eq!(encode_song(0, &[], 64), Err(SpeakerRefusal::EmptySong));
    assert_eq!(
        encode_song(0, &[note; 17], 64),
        Err(SpeakerRefusal::SongCapacityExceeded)
    );
    assert_eq!(
        encode_song(
            0,
            &[OiSongEvent {
                pitch: OiPitch::Note(30),
                duration_ticks: 1
            }],
            64
        ),
        Err(SpeakerRefusal::PitchOutOfRange)
    );
    assert_eq!(
        encode_song(
            0,
            &[OiSongEvent {
                pitch: OiPitch::Note(69),
                duration_ticks: 0
            }],
            64
        ),
        Err(SpeakerRefusal::DurationOutOfRange)
    );
    assert_eq!(
        encode_song(0, &[note], 6),
        Err(SpeakerRefusal::SerialPressure)
    );
}

#[test]
fn authority_excludes_motion_and_arbitrary_or_malformed_oi_bytes() {
    for command in [
        &[DRIVE_OPCODE, 0, 0, 0, 0][..],
        &[DRIVE_DIRECT_OPCODE, 0, 0, 0, 0],
        &[128],
        &[SONG_OPCODE, 0, 16],
    ] {
        assert!(!speaker_authority_admits(command));
    }
}

#[test]
fn live_safe_or_full_truth_can_offer_but_describe_passive_or_stale_cannot() {
    for mode in [OiMode::Safe, OiMode::Full] {
        let host = live_speaker_advertisement(&observation(mode)).unwrap();
        assert_eq!(host.capabilities.len(), 1);
        let offer = &host.capabilities[0];
        assert_eq!(
            offer.kind_id.as_str(),
            conduit_semantic_catalog::MUSIC_PLAY_KIND
        );
        assert_eq!(offer.authority_requirements.len(), 1);
        assert!(offer
            .resource_requirements
            .iter()
            .all(|requirement| requirement.units == 1));
        let realization = live_speaker_realization(&observation(mode)).unwrap();
        assert_eq!(realization.capability_id.as_str(), SPEAKER_CAPABILITY);
    }
    assert_eq!(
        live_speaker_advertisement(&observation(OiMode::Passive)),
        Err(SpeakerRefusal::UnsupportedMode)
    );
    let mut stale = observation(OiMode::Safe);
    stale.currently_usable = false;
    assert_eq!(
        live_speaker_advertisement(&stale),
        Err(SpeakerRefusal::NotCurrentlyUsable)
    );
    let mut unverified = observation(OiMode::Safe);
    unverified.robot_identity_verified = false;
    assert_eq!(
        live_speaker_advertisement(&unverified),
        Err(SpeakerRefusal::UnverifiedIdentity)
    );
}

#[test]
fn profile_rejects_polyphony_velocity_expression_microtones_and_pcm() {
    let offered = compatibility_profile();
    let mut required = offered.clone();
    required.maximum_polyphony = 2;
    assert_eq!(
        conduit_semantic_catalog::compatibility(&required, &offered),
        Err(conduit_semantic_catalog::IncompatibilityReason::PolyphonyExceedsOffer)
    );
    required = offered.clone();
    required.preserves_velocity = true;
    assert_eq!(
        conduit_semantic_catalog::compatibility(&required, &offered),
        Err(conduit_semantic_catalog::IncompatibilityReason::VelocityUnsupported)
    );
    required = offered.clone();
    required.supports_subtractive_filter = true;
    assert_eq!(
        conduit_semantic_catalog::compatibility(&required, &offered),
        Err(conduit_semantic_catalog::IncompatibilityReason::SubtractiveFilterUnsupported)
    );
    required = offered.clone();
    required.accepts_microtonal_pitch = true;
    assert_eq!(
        conduit_semantic_catalog::compatibility(&required, &offered),
        Err(conduit_semantic_catalog::IncompatibilityReason::MicrotonalPitchUnsupported)
    );
    required = offered.clone();
    required.seam = conduit_semantic_catalog::SoundSeam::PcmPlayback;
    assert_eq!(
        conduit_semantic_catalog::compatibility(&required, &offered),
        Err(conduit_semantic_catalog::IncompatibilityReason::WrongSemanticSeam)
    );
}
