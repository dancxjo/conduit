use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_audio::{
    Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, PcmChannelLayout, PcmFrameHeader,
    PcmSampleRepresentation,
};
use conduit_core::{
    kind_id, resource_requirement, AuthorityContractId, AuthorityRequirement, CapabilityOffer,
    HostCallContractId, HostCallRequirement, PlannedGear, PortDirection, Quantity, QuantityUnit,
    QUANTITY_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, RequestId,
};

pub(crate) const IMPLEMENTATION: &str = "std/kernel-pitch-tone-fixed-q16@1";
const PROFILE: &str = "std/pitch-tone-s16le-48000-stereo-p256@1";
const ARTIFACT: &str = "conduit-std-host/pitch-tone-fixed-q16@1";
pub(super) const HOST_CALL: &str = "conduit.host/pitch-tone-playback@1";
pub(crate) const TONE_BLOCKS: u16 = 4;
const NOTE_OFF_BLOCK: u16 = 2;
const MAXIMUM_PCM_BYTES: usize = conduit_semantic_catalog::AUDIO_PLAY_ALSA_PCM_BLOCK_BYTES as usize;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct PitchToneBack {
    pending: bool,
    closed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for PitchToneBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(0) {
                return step_fail(90);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.output.is_some() {
                return step_fail(91);
            }
            io.consume_host_completion()
                .expect("observed pitch-tone completion");
            self.pending = false;
            return if self.closed {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            };
        }
        if let Some(value) = io.input(conduit_kernel::PortId(0)) {
            if self.pending || self.closed {
                return step_fail(92);
            }
            let Ok(input) = BoundedValueRef::new(value, QUANTITY_ENCODED_LEN as u32) else {
                return step_fail(93);
            };
            io.consume(conduit_kernel::PortId(0))
                .expect("present pitch-tone input");
            io.request_host_call(RequestId(0), HostCallId(0), input)
                .expect("pitch-tone Host Call");
            self.pending = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(conduit_kernel::PortId(0)) && !self.pending {
            io.consume_closed(conduit_kernel::PortId(0))
                .expect("observed pitch-tone closure");
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.closed = true;
    }
}

const fn step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 64,
        maximum_value_bytes: QUANTITY_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::PitchTone(PitchToneBack {
        pending: false,
        closed: false,
    }))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    let resource = placement.resources.first();
    let authority = placement.authority.first();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
        || placement.inputs.len() != 1
        || placement.inputs[0].port_id.as_str() != "pitch"
        || placement.inputs[0].direction != PortDirection::Input
        || !placement.outputs.is_empty()
        || placement.resources.len() != 1
        || resource.is_none_or(|binding| {
            binding.class_id.as_str() != conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS
                || binding.units != 1
                || binding.protected.is_some()
                || binding.compute.is_some()
        })
        || placement.authority.len() != 1
        || authority.is_none_or(|binding| {
            binding.contract_id.as_str() != conduit_std_offers::AUDIO_PLAYBACK_AUTHORITY_CONTRACT
                || binding.host_call_contract_id.as_str() != HOST_CALL
                || binding.subject_kind.as_str() != conduit_audio::AUDIO_PCM_INFO_ID
                || binding.host_id != placement.host_id
                || binding.boot_id != placement.boot_id
                || binding.capability_id != placement.capability_id
        })
        || !placement.configuration.is_empty()
    {
        return Err("planned sound/pitch-tone identity/resource/authority mismatch".into());
    }
    Ok(())
}

pub(crate) fn offer() -> CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::pitch_tone_contract(),
        conduit_semantic_catalog::PITCH_TONE_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: "pitch-tone-fixed-q16",
            execution_profile: PROFILE,
            implementation: IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(HOST_CALL),
            target_kind: Some(kind_id(conduit_audio::AUDIO_PCM_INFO_ID)),
            maximum_in_flight: 1,
            maximum_input_bytes: QUANTITY_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        vec![resource_requirement(
            conduit_std_offers::AUDIO_PLAYBACK_RESOURCE_CLASS,
            1,
        )],
        vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(
                conduit_std_offers::AUDIO_PLAYBACK_AUTHORITY_CONTRACT,
            ),
            host_call_contract_id: HostCallContractId::from(HOST_CALL),
            subject_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        }],
    )
}

pub(super) fn prepare_session(
    placement: &PlannedGear,
    selected: Option<&crate::hosted_audio::HostedPlaybackSelection>,
) -> Result<crate::hosted_audio::PlaybackSession, String> {
    validate(placement)?;
    let selected = selected.ok_or_else(|| {
        "planned sound/pitch-tone has no exact selected hosted playback resource".to_string()
    })?;
    let advertisement = selected.realization_advertisement(placement.host_id.clone());
    if selected.boot_id != placement.boot_id
        || selected.offer_generation != placement.offer_generation
        || placement.resources[0].pool_id != selected.pool_id()
        || (!placement.realization_characteristics.is_empty()
            && placement.realization_characteristics != advertisement.characteristics)
    {
        return Err("planned sound/pitch-tone resource is stale or differs from selection".into());
    }
    Ok(crate::hosted_audio::PlaybackSession::resolved(
        selected.clone(),
    ))
}

pub(super) fn execute(
    session: &mut crate::hosted_audio::PlaybackSession,
    input: &[u8],
) -> conduit_kernel::HostCallOutcome {
    let mut frame = [0_u8; MAXIMUM_PCM_BYTES];
    match render_tone(session, input, &mut frame) {
        Ok(()) => conduit_kernel::HostCallOutcome {
            disposition: HostCallDisposition::Completed,
            output: None,
            failure: None,
        },
        Err(PitchToneFailure::Playback(error)) => playback_failure(error),
        Err(PitchToneFailure::InvalidInput) => failed(FailureCode::InvalidInput, 94),
        Err(PitchToneFailure::Synthesis) => failed(FailureCode::HostCallFailed, 95),
    }
}

enum PitchToneFailure {
    InvalidInput,
    Synthesis,
    Playback(crate::hosted_audio::PlaybackFailure),
}

fn render_tone(
    session: &mut crate::hosted_audio::PlaybackSession,
    input: &[u8],
    encoded: &mut [u8; MAXIMUM_PCM_BYTES],
) -> Result<(), PitchToneFailure> {
    let quantity = Quantity::decode(input).map_err(|_| PitchToneFailure::InvalidInput)?;
    let pitch = MusicalPitch::from_quantities(quantity, Quantity::new(440, QuantityUnit::Hertz), 0)
        .map_err(|_| PitchToneFailure::InvalidInput)?;
    let mut synth = conduit_synth::ReferenceSynth::new(
        conduit_synth::ReferenceSynthProfile::musician_reference(),
    )
    .map_err(|_| PitchToneFailure::Synthesis)?;
    synth
        .apply_note(note_event(pitch, Gate::On, 0, 0, u16::MAX)?)
        .map_err(|_| PitchToneFailure::Synthesis)?;
    let note_off_micros = frames_to_micros(
        u64::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES)
            * u64::from(NOTE_OFF_BLOCK),
    );
    let note_off = note_event(pitch, Gate::Off, note_off_micros, 1, 0)?;
    let mut note_off_sent = false;
    let mut mono = [0_i16; conduit_semantic_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES as usize];
    for block in 0..TONE_BLOCKS {
        if !note_off_sent && block == NOTE_OFF_BLOCK {
            synth
                .apply_note(note_off)
                .map_err(|_| PitchToneFailure::Synthesis)?;
            note_off_sent = true;
        }
        let start_frame =
            u64::from(block) * u64::from(conduit_semantic_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES);
        synth.render(&mut mono);
        let encoded_len = encode_block(encoded, &mono, start_frame, block == 0)?;
        session
            .write_frame(&encoded[..encoded_len])
            .map_err(PitchToneFailure::Playback)?;
    }
    Ok(())
}

fn note_event(
    pitch: MusicalPitch,
    gate: Gate,
    event_time_micros: u64,
    order: u32,
    velocity: u16,
) -> Result<MusicalNoteEvent, PitchToneFailure> {
    MusicalNoteEvent::new(
        NoteOccurrenceId(1),
        pitch,
        gate,
        velocity,
        event_time_micros,
        order,
    )
    .map_err(|_| PitchToneFailure::Synthesis)
}

fn encode_block(
    encoded: &mut [u8; MAXIMUM_PCM_BYTES],
    mono: &[i16; conduit_semantic_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES as usize],
    start_frame: u64,
    discontinuity: bool,
) -> Result<usize, PitchToneFailure> {
    let header = PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        crate::hosted_audio::SAMPLE_RATE_HZ,
        PcmChannelLayout::StereoLeftRight,
        conduit_semantic_catalog::AUDIO_PLAY_ALSA_PERIOD_FRAMES,
        crate::hosted_audio::SOURCE_CLOCK_ID,
        start_frame,
        discontinuity,
    )
    .map_err(|_| PitchToneFailure::Synthesis)?;
    let header_bytes = header.encode();
    let encoded_len = header_bytes.len() + mono.len() * 4;
    encoded[..header_bytes.len()].copy_from_slice(&header_bytes);
    let payload = &mut encoded[header_bytes.len()..encoded_len];
    for (frame, sample) in payload.chunks_exact_mut(4).zip(mono.iter()) {
        let sample = sample.to_le_bytes();
        frame[..2].copy_from_slice(&sample);
        frame[2..].copy_from_slice(&sample);
    }
    Ok(encoded_len)
}

fn frames_to_micros(frames: u64) -> u64 {
    frames
        .saturating_mul(1_000_000)
        .div_ceil(u64::from(crate::hosted_audio::SAMPLE_RATE_HZ))
}

fn playback_failure(
    error: crate::hosted_audio::PlaybackFailure,
) -> conduit_kernel::HostCallOutcome {
    use crate::hosted_audio::PlaybackFailure;
    match error {
        PlaybackFailure::StaleObservation => failed(FailureCode::HostCallDenied, 70),
        PlaybackFailure::DeviceBusy => failed(FailureCode::HostCallDenied, 71),
        PlaybackFailure::OpenFailed => failed(FailureCode::HostCallFailed, 72),
        PlaybackFailure::InvalidPcm | PlaybackFailure::DiscontinuousInput => {
            failed(FailureCode::InvalidInput, 73)
        }
        PlaybackFailure::Underrun => failed(FailureCode::HostCallFailed, 74),
        PlaybackFailure::ProviderLost => failed(FailureCode::HostCallFailed, 75),
        PlaybackFailure::WriteFailed => failed(FailureCode::HostCallFailed, 76),
        PlaybackFailure::DrainFailed
        | PlaybackFailure::CloseFailed
        | PlaybackFailure::InvalidLifecycle => failed(FailureCode::HostCallFailed, 77),
    }
}

fn failed(code: FailureCode, detail: u16) -> conduit_kernel::HostCallOutcome {
    conduit_kernel::HostCallOutcome {
        disposition: if code == FailureCode::HostCallDenied {
            HostCallDisposition::Denied
        } else {
            HostCallDisposition::Failed
        },
        output: None,
        failure: Some(Failure { code, detail }),
    }
}
