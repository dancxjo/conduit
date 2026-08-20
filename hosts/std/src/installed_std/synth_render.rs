//! Exact queued-event rendering for the installed reference synth.

use conduit_core::{
    AudioRenderDemand, PlannedGear, AUDIO_RENDER_DEMAND_ENCODED_LEN, CONTROL_EVENT_ENCODED_LEN,
    NOTE_EVENT_ENCODED_LEN,
};
use std::collections::VecDeque;

#[derive(Debug, Copy, Clone)]
enum PendingMusicalEvent {
    Note(conduit_core::MusicalNoteEvent),
    Control(conduit_core::MusicalControlEvent),
}

impl PendingMusicalEvent {
    fn frame(self) -> u64 {
        let micros = match self {
            Self::Note(event) => event.event_time_micros,
            Self::Control(event) => event.event_time_micros,
        };
        micros.saturating_mul(u64::from(conduit_synth::REFERENCE_SAMPLE_RATE_HZ)) / 1_000_000
    }
}

pub(super) struct InstalledSynthState {
    synth: conduit_synth::ReferenceSynth,
    last_event: Option<(u64, u32)>,
    clock_origin_micros: u64,
    next_demand_sequence: u32,
    pending_events: VecDeque<PendingMusicalEvent>,
}

impl InstalledSynthState {
    pub(super) fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        super::synth_operation::validate(placement)?;
        Self::new(super::synth_operation::profile(placement)?)
    }

    fn new(profile: conduit_synth::ReferenceSynthProfile) -> Result<Self, String> {
        Ok(Self {
            synth: conduit_synth::ReferenceSynth::new(profile)
                .map_err(|error| format!("prepare reference synth: {error:?}"))?,
            last_event: None,
            clock_origin_micros: 0,
            next_demand_sequence: 0,
            pending_events: VecDeque::with_capacity(usize::from(
                conduit_std_catalog::MAXIMUM_MUSICAL_EVENT_ITEMS,
            )),
        })
    }

    pub(super) fn set_clock_origin(&mut self, clock_origin_micros: u64) {
        self.clock_origin_micros = clock_origin_micros;
    }

    pub(super) fn stop(&mut self) {
        self.synth.stop();
        self.pending_events.clear();
    }
}

pub(super) fn execute(
    state: &mut InstalledSynthState,
    input: &[u8],
    output: &mut Vec<u8>,
) -> Result<bool, String> {
    if input.len() == AUDIO_RENDER_DEMAND_ENCODED_LEN {
        return render_demand(
            state,
            AudioRenderDemand::decode(input)
                .map_err(|error| format!("decode audio render demand: {error:?}"))?,
            output,
        );
    }
    let mut event = if input.len() == NOTE_EVENT_ENCODED_LEN {
        PendingMusicalEvent::Note(
            conduit_core::MusicalNoteEvent::decode(input)
                .map_err(|error| format!("decode music note event: {error:?}"))?,
        )
    } else if input.len() == CONTROL_EVENT_ENCODED_LEN {
        PendingMusicalEvent::Control(
            conduit_core::MusicalControlEvent::decode(input)
                .map_err(|error| format!("decode music control event: {error:?}"))?,
        )
    } else {
        return Err("reference synth input has an unsupported exact length".to_string());
    };
    let key = match event {
        PendingMusicalEvent::Note(event) => (event.event_time_micros, event.order),
        PendingMusicalEvent::Control(event) => (event.event_time_micros, event.order),
    };
    if state.last_event.is_some_and(|last| key <= last) {
        return Err(
            "reference synth events are not in exact global timestamp/order sequence".to_string(),
        );
    }
    let relative_micros = key
        .0
        .checked_sub(state.clock_origin_micros)
        .ok_or_else(|| "reference synth event predates the admitted clock origin".to_string())?;
    match &mut event {
        PendingMusicalEvent::Note(event) => event.event_time_micros = relative_micros,
        PendingMusicalEvent::Control(event) => event.event_time_micros = relative_micros,
    }
    if event.frame() < state.synth.frame_cursor() {
        return Err("reference synth event is stale".to_string());
    }
    if state.pending_events.len() == state.pending_events.capacity() {
        return Err("reference synth pending-event capacity is exhausted".to_string());
    }
    state.pending_events.push_back(event);
    state.last_event = Some(key);
    output.clear();
    Ok(false)
}

fn render_demand(
    state: &mut InstalledSynthState,
    demand: AudioRenderDemand,
    output: &mut Vec<u8>,
) -> Result<bool, String> {
    if demand.clock_id != conduit_std_catalog::AUDIO_RENDER_CLOCK_ID
        || demand.sequence != state.next_demand_sequence
        || demand.start_frame != state.synth.frame_cursor()
        || demand.frame_count > conduit_synth::REFERENCE_MAXIMUM_BLOCK_FRAMES
    {
        return Err("reference synth render demand is stale or outside its exact profile".into());
    }
    let end_frame = demand
        .start_frame
        .checked_add(u64::from(demand.frame_count))
        .ok_or_else(|| "reference synth render interval overflows".to_string())?;
    let mut samples = [0_i16; conduit_synth::REFERENCE_MAXIMUM_BLOCK_FRAMES as usize];
    let mut rendered = 0_usize;
    while state
        .pending_events
        .front()
        .is_some_and(|event| event.frame() <= end_frame)
    {
        let event = state.pending_events.pop_front().unwrap();
        let event_frame = event.frame();
        if event_frame < state.synth.frame_cursor() {
            return Err("reference synth queued event became stale".into());
        }
        let segment = usize::try_from(event_frame - state.synth.frame_cursor())
            .map_err(|_| "reference synth render segment is too large".to_string())?;
        state
            .synth
            .render(&mut samples[rendered..rendered + segment]);
        rendered += segment;
        match event {
            PendingMusicalEvent::Note(event) => state.synth.apply_note(event),
            PendingMusicalEvent::Control(event) => state.synth.apply_control(event),
        }
        .map_err(|error| format!("apply reference synth event: {error:?}"))?;
    }
    let remaining = usize::from(demand.frame_count) - rendered;
    state
        .synth
        .render(&mut samples[rendered..rendered + remaining]);

    output.clear();
    let header = conduit_core::PcmFrameHeader::new(
        conduit_core::PcmSampleRepresentation::Signed16LittleEndian,
        conduit_synth::REFERENCE_SAMPLE_RATE_HZ,
        conduit_core::PcmChannelLayout::StereoLeftRight,
        demand.frame_count,
        demand.clock_id,
        demand.start_frame,
        false,
    )
    .map_err(|error| format!("frame reference synth PCM: {error:?}"))?;
    output.extend_from_slice(&header.encode());
    let payload_start = output.len();
    output.resize(payload_start + usize::from(demand.frame_count) * 4, 0);
    for (encoded, sample) in output[payload_start..]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
        .zip(samples.iter())
    {
        let sample = sample.to_le_bytes();
        encoded[..2].copy_from_slice(&sample);
        encoded[2..].copy_from_slice(&sample);
    }
    state.next_demand_sequence = state.next_demand_sequence.wrapping_add(1);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{Gate, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, PcmFrameHeader};

    fn note(occurrence: u64, gate: Gate, micros: u64, order: u32) -> MusicalNoteEvent {
        MusicalNoteEvent::new(
            NoteOccurrenceId(occurrence),
            MusicalPitch::new(440_000, 440_000, 0).unwrap(),
            gate,
            u16::MAX,
            micros,
            order,
        )
        .unwrap()
    }

    #[test]
    fn exact_render_demand_applies_queued_events_at_sample_offsets() {
        let mut state =
            InstalledSynthState::new(conduit_synth::ReferenceSynthProfile::musician_reference())
                .unwrap();
        let mut output =
            Vec::with_capacity(conduit_std_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES as usize);
        assert!(!execute(&mut state, &note(1, Gate::On, 0, 0).encode(), &mut output).unwrap());
        assert!(!execute(
            &mut state,
            &note(1, Gate::Off, 5_000, 1).encode(),
            &mut output
        )
        .unwrap());
        let demand = AudioRenderDemand::new(1, 0, 240, 0).unwrap();
        assert!(execute(&mut state, &demand.encode(), &mut output).unwrap());
        let (header, payload) = PcmFrameHeader::decode_frame(&output).unwrap();
        assert_eq!(header.frame_count, 240);
        assert_eq!(header.start_frame, 0);
        assert_eq!(payload.len(), 960);
        assert!(payload
            .as_chunks::<4>()
            .0
            .iter()
            .all(|frame| frame[..2] == frame[2..]));
        assert!(payload.iter().any(|byte| *byte != 0));
        assert!(output.len() <= conduit_std_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES as usize);
    }

    #[test]
    fn clock_origin_global_order_and_render_continuity_are_exact() {
        let mut state =
            InstalledSynthState::new(conduit_synth::ReferenceSynthProfile::musician_reference())
                .unwrap();
        state.set_clock_origin(9_000_000);
        let mut output =
            Vec::with_capacity(conduit_std_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES as usize);
        execute(
            &mut state,
            &note(1, Gate::On, 9_000_000, 2).encode(),
            &mut output,
        )
        .unwrap();
        assert!(execute(
            &mut state,
            &note(1, Gate::Off, 9_000_000, 1).encode(),
            &mut output
        )
        .unwrap_err()
        .contains("global timestamp/order"));
        assert!(execute(
            &mut state,
            &AudioRenderDemand::new(1, 1, 240, 0).unwrap().encode(),
            &mut output,
        )
        .unwrap_err()
        .contains("stale or outside"));
        assert!(execute(
            &mut state,
            &note(2, Gate::On, 10_000_000, 3).encode(),
            &mut output,
        )
        .is_ok());
    }

    #[test]
    fn event_inside_a_block_starts_at_its_exact_sample_offset() {
        let mut state =
            InstalledSynthState::new(conduit_synth::ReferenceSynthProfile::musician_reference())
                .unwrap();
        let mut output =
            Vec::with_capacity(conduit_std_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES as usize);
        execute(
            &mut state,
            &note(1, Gate::On, 2_500, 0).encode(),
            &mut output,
        )
        .unwrap();
        execute(
            &mut state,
            &AudioRenderDemand::new(1, 0, 240, 0).unwrap().encode(),
            &mut output,
        )
        .unwrap();
        let (_, payload) = PcmFrameHeader::decode_frame(&output).unwrap();
        assert!(payload[..120 * 4].iter().all(|byte| *byte == 0));
        assert!(payload[120 * 4..].iter().any(|byte| *byte != 0));
    }
}
