use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_audio::{
    AUDIO_RENDER_DEMAND_ENCODED_LEN, CONTROL_EVENT_ENCODED_LEN, NOTE_EVENT_ENCODED_LEN,
};
use conduit_core::{CapabilityOffer, ConfigurationValue, HostOperationRequirement, PlannedGear};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId,
};

pub(super) use super::synth_render::{execute, InstalledSynthState};

pub(super) const SYNTH_HOST_OPERATION: &str = conduit_std_catalog::MUSIC_SYNTH_HOST_OPERATION;
pub(super) const PCM_BLOCK_BYTES: u32 = conduit_std_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES;

pub(super) static MUSIC_SYNTH_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_synth::REFERENCE_SYNTH_IMPLEMENTATION_ID,
    budget,
    prepare,
};

pub(super) fn host_requirement() -> HostOperationRequirement {
    conduit_std_catalog::music_synth_reference_offer().host_operations[0].clone()
}

pub(crate) fn offer() -> CapabilityOffer {
    conduit_std_catalog::music_synth_reference_offer()
}

pub(super) struct MusicSynthOperation {
    pending: Option<RequestId>,
    input: Option<conduit_kernel::ValueRef>,
    next_request: u32,
    closed: [bool; 3],
    completed: bool,
}

impl MusicSynthOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if self.pending.is_none()
                    && self.input.is_none()
                    && ((port == PortId(0) && value.byte_len == NOTE_EVENT_ENCODED_LEN as u32)
                        || (port == PortId(1)
                            && value.byte_len == CONTROL_EVENT_ENCODED_LEN as u32)
                        || (port == PortId(2)
                            && value.byte_len == AUDIO_RENDER_DEMAND_ENCODED_LEN as u32)) =>
            {
                let request = RequestId(self.next_request);
                self.next_request = self.next_request.wrapping_add(1);
                self.pending = Some(request);
                self.input = Some(value);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(
                        value,
                        NOTE_EVENT_ENCODED_LEN
                            .max(CONTROL_EVENT_ENCODED_LEN)
                            .max(AUDIO_RENDER_DEMAND_ENCODED_LEN) as u32,
                    ) {
                        Ok(input) => input,
                        Err(_) => return InstalledOperation::fail(40),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.input = None;
                match outcome.output {
                    Some(output)
                        if output.admitted_bytes == PCM_BLOCK_BYTES
                            && output.value.byte_len <= PCM_BLOCK_BYTES =>
                    {
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    None => self.finish_or_await(),
                    _ => InstalledOperation::fail(41),
                }
            }
            OperationInput::Closed { port } if self.pending.is_none() && self.input.is_none() => {
                let index = usize::from(port.0);
                if index >= self.closed.len() || self.closed[index] {
                    return InstalledOperation::fail(42);
                }
                self.closed[index] = true;
                self.finish_or_await()
            }
            _ => InstalledOperation::fail(43),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.finish_or_await()
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.input = None;
        self.completed = true;
    }

    pub(super) fn retains_resumed_value(&self) -> bool {
        false
    }

    pub(super) fn take_released_value(&mut self) -> Option<conduit_kernel::ValueRef> {
        None
    }

    fn finish_or_await(&mut self) -> OperationAction {
        if self.closed == [true; 3] {
            self.completed = true;
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    profile(placement)?;
    Ok(OperationBudget {
        value_items: 3,
        value_bytes: PCM_BLOCK_BYTES * 3,
        host_requests: usize::from(conduit_std_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS)
            + usize::from(conduit_std_catalog::AUDIO_RENDER_MAXIMUM_BLOCKS),
        sign_items: 4_096,
        maximum_value_bytes: PCM_BLOCK_BYTES,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    budget(placement)?;
    Ok(InstalledOperation::MusicSynth(MusicSynthOperation {
        pending: None,
        input: None,
        next_request: 0,
        closed: [false; 3],
        completed: false,
    }))
}

pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let expected_configuration = conduit_std_catalog::music_synth_configuration();
    let configuration_is_exact = placement.configuration.len() == expected_configuration.len()
        && expected_configuration.iter().all(|field| {
            placement
                .configuration
                .iter()
                .filter(|entry| entry.key == field.key)
                .count()
                == 1
        });
    if placement.kind_id.as_str() != conduit_std_catalog::MUSIC_SYNTH_KIND
        || placement.kind_contract_revision.as_str() != conduit_std_catalog::MUSIC_SYNTH_REVISION
        || placement.execution_profile_id.as_str() != conduit_synth::REFERENCE_SYNTH_PROFILE_ID
        || placement.implementation_id.as_str() != conduit_synth::REFERENCE_SYNTH_IMPLEMENTATION_ID
        || placement.artifact_id.as_str() != conduit_synth::REFERENCE_SYNTH_ARTIFACT_ID
        || placement.inputs != offer().inputs
        || placement.outputs != offer().outputs
        || placement.host_operations != [host_requirement()]
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !configuration_is_exact
    {
        return Err(
            "planned music/synth placement does not match the installed reference profile"
                .to_string(),
        );
    }
    Ok(())
}

pub(super) fn profile(
    placement: &PlannedGear,
) -> Result<conduit_synth::ReferenceSynthProfile, String> {
    use conduit_std_catalog::*;

    let oscillator = match text(placement, SYNTH_OSCILLATOR_KEY)? {
        "sine" => conduit_synth::OscillatorShape::Sine,
        "triangle" => conduit_synth::OscillatorShape::Triangle,
        "saw" => conduit_synth::OscillatorShape::Saw,
        "pulse" => conduit_synth::OscillatorShape::Pulse,
        _ => return Err("planned synth oscillator is unsupported".into()),
    };
    let steal_policy = match text(placement, SYNTH_STEAL_POLICY_KEY)? {
        "oldest-released-then-oldest-active" => {
            conduit_synth::VoiceStealPolicy::OldestReleasedThenOldestActive
        }
        "refuse" => conduit_synth::VoiceStealPolicy::Refuse,
        _ => return Err("planned synth voice-steal policy is unsupported".into()),
    };
    let profile = conduit_synth::ReferenceSynthProfile {
        maximum_voices: integer(placement, SYNTH_MAXIMUM_VOICES_KEY)?,
        maximum_block_frames: conduit_synth::REFERENCE_MAXIMUM_BLOCK_FRAMES,
        oscillator,
        pulse_width_q16: integer(placement, SYNTH_PULSE_WIDTH_KEY)?,
        attack_micros: integer(placement, SYNTH_ATTACK_KEY)?,
        decay_micros: integer(placement, SYNTH_DECAY_KEY)?,
        sustain_level_q16: integer(placement, SYNTH_SUSTAIN_KEY)?,
        release_micros: integer(placement, SYNTH_RELEASE_KEY)?,
        filter_cutoff_q16: integer(placement, SYNTH_FILTER_CUTOFF_KEY)?,
        filter_resonance_q16: integer(placement, SYNTH_FILTER_RESONANCE_KEY)?,
        filter_envelope_amount_q16: signed_integer(placement, SYNTH_FILTER_ENVELOPE_KEY)?,
        lfo_rate_millihertz: integer(placement, SYNTH_LFO_RATE_KEY)?,
        lfo_depth_q16: integer(placement, SYNTH_LFO_DEPTH_KEY)?,
        master_gain_q16: integer(placement, SYNTH_MASTER_GAIN_KEY)?,
        steal_policy,
    };
    profile
        .validate()
        .map_err(|error| format!("planned reference synth profile is invalid: {error:?}"))
}

fn integer<T>(placement: &PlannedGear, key: &str) -> Result<T, String>
where
    T: TryFrom<u64>,
{
    let value = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::U64(value)) if found == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("planned synth configuration '{key}' is missing or invalid"))?;
    T::try_from(value).map_err(|_| format!("planned synth configuration '{key}' is out of range"))
}

fn signed_integer<T>(placement: &PlannedGear, key: &str) -> Result<T, String>
where
    T: TryFrom<i64>,
{
    let value = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::I64(value)) if found == key => Some(*value),
            _ => None,
        })
        .ok_or_else(|| format!("planned synth configuration '{key}' is missing or invalid"))?;
    T::try_from(value).map_err(|_| format!("planned synth configuration '{key}' is out of range"))
}

fn text<'a>(placement: &'a PlannedGear, key: &str) -> Result<&'a str, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (found, ConfigurationValue::Text(value)) if found == key => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(|| format!("planned synth configuration '{key}' is missing or invalid"))
}
