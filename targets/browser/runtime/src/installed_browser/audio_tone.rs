//! Pure bounded browser realization of portable `audio/tone` PCM synthesis.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_audio::{AudioToneTerminal, PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{
    Back, BackOfferBuilder, CapabilityId, ExecutionProfileId, ImplementationId, PlannedGear,
    Quantity, QuantityUnit,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    CanonicalValue, Failure, FailureCode, PortId,
};

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-audio-tone@1";
const SAMPLE_RATE: u32 = 48_000;
const FRAMES: u16 = conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_PCM_FRAMES;
const BLOCK_BYTES: usize = conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_PCM_BLOCK_BYTES as usize;
const CLOCK_ID: u64 = 0x746f_6e65;

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    BackOfferBuilder::new(
        conduit_semantic_catalog::audio_continuous_tone_semantic_contract(),
        Back {
            capability_id: CapabilityId::from("browser-audio-tone"),
            execution_profile_id: ExecutionProfileId::from(
                "browser/audio-tone-fixed-s16le-mono-48000-f2000@1",
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: "conduit-browser-runtime/audio-tone@1".into(),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

struct AudioToneBack {
    frequency: Option<i128>,
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
                input: PortId(2),
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
        if io.input(PortId(2)).is_some() {
            if !inputs.input(PortId(2)).is_some_and(<[u8]>::is_empty) {
                return fail(10);
            }
            io.consume(PortId(2)).expect("present cancellation request");
            let terminal = CanonicalValue::new(&AudioToneTerminal::Cancelled.encode())
                .expect("bounded terminal");
            return StepOutcome::Abnormal {
                port: PortId(0),
                terminal,
            };
        }
        if io.input(PortId(0)).is_some() {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(11);
            };
            let Some(frequency) = Quantity::decode(bytes).ok().and_then(frequency_millihertz)
            else {
                return fail(12);
            };
            io.consume(PortId(0)).expect("present frequency");
            self.frequency = Some(frequency);
            return StepOutcome::Progress;
        }
        if io.input(PortId(1)).is_some() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(bytes) = inputs.input(PortId(1)) else {
                return fail(14);
            };
            if conduit_time::decode_tick(bytes).is_err() {
                return fail(15);
            }
            let Some(frequency) = self.frequency else {
                return StepOutcome::Await;
            };
            let (block, phase) = match render(frequency, self.phase, self.start_frame) {
                Some(value) => value,
                None => return fail(13),
            };
            io.consume(PortId(1)).expect("present cadence");
            io.send_canonical(PortId(0), block)
                .expect("ready PCM output");
            self.phase = phase;
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

fn frequency_millihertz(value: Quantity) -> Option<i128> {
    match value.unit() {
        QuantityUnit::Millihertz => Some(i128::from(value.value())),
        QuantityUnit::Hertz => Some(i128::from(value.value()) * 1_000),
        _ => None,
    }
}
fn render(frequency: i128, phase: u32, start_frame: u64) -> Option<(CanonicalValue, u32)> {
    let increment =
        ((frequency << 32) / (i128::from(SAMPLE_RATE) * 1_000)).rem_euclid(1_i128 << 32) as u32;
    let header = PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        SAMPLE_RATE,
        PcmChannelLayout::Mono,
        FRAMES,
        CLOCK_ID,
        start_frame,
        false,
    )
    .ok()?;
    let mut encoded = [0_u8; BLOCK_BYTES];
    encoded[..conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN].copy_from_slice(&header.encode());
    let mut next = phase;
    for frame in 0..usize::from(FRAMES) {
        let quadrant = next >> 30;
        let ramp = ((next >> 14) & 0xffff) as i32;
        let triangle = match quadrant {
            0 => ramp,
            1 => 65_535 - ramp,
            2 => -ramp,
            _ => ramp - 65_535,
        };
        let offset = conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN + frame * 2;
        encoded[offset..offset + 2].copy_from_slice(&((triangle / 8) as i16).to_le_bytes());
        next = next.wrapping_add(increment);
    }
    Some((CanonicalValue::new(&encoded).ok()?, next))
}
const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    Ok(BrowserBack::installed_step(AudioToneBack {
        frequency: None,
        phase: 0,
        start_frame: 0,
        closed: false,
    }))
}
