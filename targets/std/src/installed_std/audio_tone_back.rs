//! Pure bounded realization of `current Frequency -> audio/tone -> PCM flow`.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_audio::{AudioToneTerminal, PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{PlannedGear, Quantity, QuantityUnit};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    CanonicalValue, Failure, FailureCode, PortId,
};

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: u16 = conduit_semantic_catalog::AUDIO_TONE_PCM_FRAMES;
const BLOCK_BYTES: usize = conduit_semantic_catalog::AUDIO_TONE_PCM_BLOCK_BYTES as usize;
const CLOCK_ID: u64 = 0x746f_6e65;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::AUDIO_TONE_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct AudioToneBack {
    phase: u32,
    start_frame: u64,
    closed: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for AudioToneBack {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        Some(AssignedTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::NotAccepted,
            abnormal: AssignedAbnormalTransduction::NotAccepted,
            cancellation: AssignedCancellationTransduction::Request {
                input: PortId(1),
                disposition_kind: conduit_core::semantic_digest(
                    "conduit/kind-identity",
                    conduit_audio::audio_tone_terminal_kind_id()
                        .as_str()
                        .as_bytes(),
                ),
            },
        })
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if io.input(PortId(1)).is_some() {
            let Some(bytes) = inputs.input(PortId(1)) else {
                return failure(10);
            };
            if !bytes.is_empty() {
                return failure(11);
            }
            io.consume(PortId(1)).expect("present cancellation request");
            let terminal = CanonicalValue::new(&AudioToneTerminal::Cancelled.encode())
                .expect("terminal info is bounded");
            return StepOutcome::Abnormal {
                port: PortId(0),
                terminal,
            };
        }
        if io.input(PortId(0)).is_some() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(bytes) = inputs.input(PortId(0)) else {
                return failure(12);
            };
            let frequency = match decode_frequency_input(bytes) {
                Some(value) => value,
                _ => return failure(13),
            };
            let (block, next_phase) = match render_block(frequency, self.phase, self.start_frame) {
                Ok(value) => value,
                Err(()) => return failure(14),
            };
            io.consume(PortId(0)).expect("present Frequency");
            io.send_canonical(PortId(0), block)
                .expect("ready PCM output");
            self.phase = next_phase;
            self.start_frame = self.start_frame.wrapping_add(u64::from(FRAMES));
            return StepOutcome::Progress;
        }
        if !self.closed && io.input_closed(PortId(0)) {
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.closed = true;
    }
}

fn frequency_millihertz(quantity: Quantity) -> Option<i128> {
    let value = i128::from(quantity.value());
    match quantity.unit() {
        QuantityUnit::Millihertz => Some(value),
        QuantityUnit::Hertz => Some(value * 1_000),
        _ => None,
    }
}

fn decode_frequency_input(encoded: &[u8]) -> Option<i128> {
    Quantity::decode(encoded)
        .ok()
        .and_then(frequency_millihertz)
}

fn render_block(
    frequency_millihertz: i128,
    phase: u32,
    start_frame: u64,
) -> Result<(CanonicalValue, u32), ()> {
    let turns = (frequency_millihertz << 32) / (i128::from(SAMPLE_RATE) * 1_000);
    let increment = turns.rem_euclid(1_i128 << 32) as u32;
    let header = PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        SAMPLE_RATE,
        PcmChannelLayout::Mono,
        FRAMES,
        CLOCK_ID,
        start_frame,
        false,
    )
    .map_err(|_| ())?;
    let mut encoded = [0_u8; BLOCK_BYTES];
    encoded[..conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN].copy_from_slice(&header.encode());
    let mut next = phase;
    for frame in 0..usize::from(FRAMES) {
        let sample = ((next >> 16) as u16).wrapping_sub(32_768) as i16;
        let offset = conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN + frame * 2;
        encoded[offset..offset + 2].copy_from_slice(&sample.to_le_bytes());
        next = next.wrapping_add(increment);
    }
    Ok((CanonicalValue::new(&encoded).map_err(|_| ())?, next))
}

const fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

fn budget(_: &PlannedGear) -> Result<BackBudget, String> {
    Ok(BackBudget {
        value_items: 1,
        value_bytes: BLOCK_BYTES as u32,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: BLOCK_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    let offer = conduit_std_offers::audio_tone_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.terminal_transductions
            != conduit_semantic_catalog::audio_tone_semantic_contract()
                .terminal_transductions()
                .cloned()
                .collect::<Vec<_>>()
        || !placement.configuration.is_empty()
        || !placement.host_calls.is_empty()
    {
        return Err("planned audio/tone does not match the exact pure std realization".into());
    }
    Ok(InstalledBack::AudioTone(AudioToneBack {
        phase: 0,
        start_frame: 0,
        closed: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_declares_the_exact_lowered_terminal_contract() {
        let profile = conduit_semantic_catalog::audio_tone_semantic_contract()
            .terminal_transductions()
            .next()
            .unwrap()
            .clone();
        let lowered = conduit_plan_lowering::lowering::LoweredTerminalTransduction {
            input: PortId(0),
            output: PortId(0),
            cancellation_input: Some(PortId(1)),
            profile,
        };
        let back = AudioToneBack {
            phase: 0,
            start_frame: 0,
            closed: false,
        };
        assert_eq!(
            StepBack::<2>::terminal_transduction(&back),
            Some(lowered.assigned())
        );
    }

    #[test]
    fn current_frequency_changes_subsequent_contiguous_pcm() {
        let (low, phase) = render_block(220_000, 0, 0).unwrap();
        let (high, _) = render_block(880_000, phase, u64::from(FRAMES)).unwrap();
        let (lh, lp) = PcmFrameHeader::decode_frame(low.as_slice()).unwrap();
        let (hh, hp) = PcmFrameHeader::decode_frame(high.as_slice()).unwrap();
        assert_eq!(lh.start_frame, 0);
        assert_eq!(hh.start_frame, u64::from(FRAMES));
        assert_ne!(lp, hp);
        assert_eq!(low.as_slice().len(), BLOCK_BYTES);
    }

    #[test]
    fn every_checked_frequency_encoding_crosses_the_back_boundary() {
        for quantity in [
            Quantity::new(i64::MIN, QuantityUnit::Hertz),
            Quantity::new(i64::MAX, QuantityUnit::Hertz),
            Quantity::new(i64::MIN, QuantityUnit::Millihertz),
            Quantity::new(i64::MAX, QuantityUnit::Millihertz),
            Quantity::new(0, QuantityUnit::Hertz),
        ] {
            let frequency = decode_frequency_input(&quantity.encode()).expect("checked Frequency");
            let (block, _) = render_block(frequency, u32::MAX, u64::MAX).unwrap();
            assert_eq!(block.as_slice().len(), BLOCK_BYTES);
        }
        let (_, positive) = render_block(440_000, 0, 0).unwrap();
        let (_, negative) = render_block(-440_000, 0, 0).unwrap();
        assert_eq!(negative, 0_u32.wrapping_sub(positive));
    }
}
