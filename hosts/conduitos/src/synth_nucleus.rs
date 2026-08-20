//! ConduitOS binding for the shared fixed-storage reference synthesizer.

use conduit_core::{AudioRenderDemand, MusicalControlEvent, MusicalNoteEvent, PcmChannelLayout};

const MAXIMUM_PCM_BYTES: usize = conduit_std_catalog::MUSIC_SYNTH_PCM_BLOCK_BYTES as usize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SynthNucleusError {
    InvalidProfile,
    InvalidEvent,
    StaleDemand,
    OversizedDemand,
}

pub struct SynthOutput {
    bytes: [u8; MAXIMUM_PCM_BYTES],
    len: usize,
}

impl SynthOutput {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub struct SynthNucleus {
    synth: conduit_synth::ReferenceSynth,
    next_sequence: u32,
}

impl SynthNucleus {
    pub fn new() -> Result<Self, SynthNucleusError> {
        Ok(Self {
            synth: conduit_synth::ReferenceSynth::new(
                conduit_synth::ReferenceSynthProfile::musician_reference(),
            )
            .map_err(|_| SynthNucleusError::InvalidProfile)?,
            next_sequence: 0,
        })
    }

    pub fn apply_note(&mut self, event: MusicalNoteEvent) -> Result<(), SynthNucleusError> {
        self.synth
            .apply_note(event)
            .map(|_| ())
            .map_err(|_| SynthNucleusError::InvalidEvent)
    }

    pub fn apply_control(&mut self, event: MusicalControlEvent) -> Result<(), SynthNucleusError> {
        self.synth
            .apply_control(event)
            .map(|_| ())
            .map_err(|_| SynthNucleusError::InvalidEvent)
    }

    pub fn render(&mut self, demand: AudioRenderDemand) -> Result<SynthOutput, SynthNucleusError> {
        if demand.clock_id != conduit_std_catalog::AUDIO_RENDER_CLOCK_ID
            || demand.sequence != self.next_sequence
            || demand.start_frame != self.synth.frame_cursor()
        {
            return Err(SynthNucleusError::StaleDemand);
        }
        if demand.frame_count > conduit_synth::REFERENCE_MAXIMUM_BLOCK_FRAMES {
            return Err(SynthNucleusError::OversizedDemand);
        }
        let mut samples = [0_i16; conduit_synth::REFERENCE_MAXIMUM_BLOCK_FRAMES as usize];
        self.synth
            .render(&mut samples[..usize::from(demand.frame_count)]);
        let header = conduit_core::PcmFrameHeader::new(
            conduit_core::PcmSampleRepresentation::Signed16LittleEndian,
            conduit_synth::REFERENCE_SAMPLE_RATE_HZ,
            PcmChannelLayout::StereoLeftRight,
            demand.frame_count,
            demand.clock_id,
            demand.start_frame,
            false,
        )
        .map_err(|_| SynthNucleusError::InvalidProfile)?;
        let mut output = SynthOutput {
            bytes: [0; MAXIMUM_PCM_BYTES],
            len: conduit_core::PCM_FRAME_HEADER_ENCODED_LEN + usize::from(demand.frame_count) * 4,
        };
        output.bytes[..conduit_core::PCM_FRAME_HEADER_ENCODED_LEN]
            .copy_from_slice(&header.encode());
        for (encoded, sample) in output.bytes
            [conduit_core::PCM_FRAME_HEADER_ENCODED_LEN..output.len]
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(samples.iter())
        {
            let sample = sample.to_le_bytes();
            encoded[..2].copy_from_slice(&sample);
            encoded[2..].copy_from_slice(&sample);
        }
        self.next_sequence = self.next_sequence.wrapping_add(1);
        Ok(output)
    }

    pub fn stop(&mut self) {
        self.synth.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{Gate, MusicalPitch, NoteOccurrenceId};

    #[test]
    fn renders_bounded_stereo_and_refuses_stale_demand() {
        let mut synth = SynthNucleus::new().unwrap();
        synth
            .apply_note(
                MusicalNoteEvent::new(
                    NoteOccurrenceId(1),
                    MusicalPitch::new(440_000, 440_000, 0).unwrap(),
                    Gate::On,
                    52_428,
                    0,
                    0,
                )
                .unwrap(),
            )
            .unwrap();
        let demand = AudioRenderDemand::new(1, 0, 64, 0).unwrap();
        let output = synth.render(demand).unwrap();
        assert_eq!(
            output.as_bytes().len(),
            conduit_core::PCM_FRAME_HEADER_ENCODED_LEN + 256
        );
        assert!(
            output.as_bytes()[conduit_core::PCM_FRAME_HEADER_ENCODED_LEN..]
                .iter()
                .any(|byte| *byte != 0)
        );
        assert_eq!(
            synth.render(demand).err(),
            Some(SynthNucleusError::StaleDemand)
        );
    }

    #[test]
    fn stop_makes_the_next_interval_silent() {
        let mut synth = SynthNucleus::new().unwrap();
        synth.stop();
        let output = synth
            .render(AudioRenderDemand::new(1, 0, 32, 0).unwrap())
            .unwrap();
        assert!(
            output.as_bytes()[conduit_core::PCM_FRAME_HEADER_ENCODED_LEN..]
                .iter()
                .all(|byte| *byte == 0)
        );
    }
}
