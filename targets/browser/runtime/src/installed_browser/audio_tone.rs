//! Pure bounded browser realization of portable `audio/tone` PCM synthesis.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_audio::{AudioToneTerminal, PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_core::{
    Back, BackOfferBuilder, CapabilityId, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId, PlannedGear, Quantity, QuantityUnit,
};
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    BoundedValueRef, CanonicalValue, Failure, FailureCode, HostCallDisposition, HostCallId, PortId,
    RequestId, ValueRef, ValueStorage,
};

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-audio-tone@1";
const SAMPLE_RATE: u32 = 48_000;
const FRAMES: u16 = conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_PCM_FRAMES;
const BLOCK_BYTES: usize = conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_PCM_BLOCK_BYTES as usize;
const CLOCK_ID: u64 = 0x746f_6e65;
pub(crate) const UPDATE_OPERATION: &str = "conduit.host/browser-continuous-tone-frequency@1";
pub(crate) const RENDER_OPERATION: &str = "conduit.host/browser-continuous-tone-render@1";

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
            host_calls: vec![
                HostCallRequirement {
                    contract_id: HostCallContractId::from(UPDATE_OPERATION),
                    target_kind: Some(conduit_core::kind_id(
                        conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_KIND,
                    )),
                    maximum_in_flight: 1,
                    maximum_input_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
                    maximum_output_bytes: 0,
                },
                HostCallRequirement {
                    contract_id: HostCallContractId::from(RENDER_OPERATION),
                    target_kind: Some(conduit_core::kind_id(
                        conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_KIND,
                    )),
                    maximum_in_flight: 1,
                    maximum_input_bytes: 0,
                    maximum_output_bytes: BLOCK_BYTES as u32,
                },
            ],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

struct AudioToneBack {
    empty: ValueRef,
    next_request: u32,
    pending: Option<ToneRequest>,
    frequency_ready: bool,
    closed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ToneRequest {
    Update(RequestId),
    Render(RequestId),
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
        if let Some((request, outcome)) = io.host_completion() {
            let expected = match self.pending {
                Some(ToneRequest::Update(expected)) if expected == request => {
                    ToneRequest::Update(expected)
                }
                Some(ToneRequest::Render(expected)) if expected == request => {
                    ToneRequest::Render(expected)
                }
                _ => return fail(16),
            };
            match expected {
                ToneRequest::Update(_) => {
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                    {
                        return fail(17);
                    }
                    io.consume_host_completion()
                        .expect("observed frequency update");
                    self.pending = None;
                    self.frequency_ready = true;
                    return StepOutcome::Progress;
                }
                ToneRequest::Render(_) => {
                    let Some(output) = outcome.output else {
                        return fail(18);
                    };
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                        || output.value.byte_len != BLOCK_BYTES as u32
                        || output.admitted_bytes != BLOCK_BYTES as u32
                    {
                        return fail(18);
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion().expect("observed PCM render");
                    io.send(PortId(0), output.value).expect("ready PCM output");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
            }
        }
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
            if self.pending.is_some() {
                return StepOutcome::Await;
            }
            let Some(value) = io.input(PortId(0)) else {
                return fail(11);
            };
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return fail(19);
            };
            io.consume(PortId(0)).expect("present frequency");
            io.request_host_call(
                request,
                HostCallId(0),
                BoundedValueRef::new(value, conduit_core::QUANTITY_ENCODED_LEN as u32)
                    .expect("Frequency is exactly bounded"),
            )
            .expect("frequency update Host Call");
            self.next_request = next;
            self.pending = Some(ToneRequest::Update(request));
            return StepOutcome::Progress;
        }
        if io.input(PortId(1)).is_some() {
            if self.pending.is_some() || !self.frequency_ready || !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(bytes) = inputs.input(PortId(1)) else {
                return fail(14);
            };
            if conduit_time::decode_tick(bytes).is_err() {
                return fail(15);
            }
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return fail(19);
            };
            io.consume(PortId(1)).expect("present cadence");
            io.request_host_call(
                request,
                HostCallId(1),
                BoundedValueRef::new(self.empty, 0).expect("empty render request"),
            )
            .expect("PCM render Host Call");
            self.next_request = next;
            self.pending = Some(ToneRequest::Render(request));
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
fn render(frequency: i128, phase: u32, start_frame: u64) -> Option<([u8; BLOCK_BYTES], u32)> {
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
    Some((encoded, next))
}
#[cfg(test)]
pub(crate) fn test_block() -> [u8; BLOCK_BYTES] {
    render(440_000, 0, 0).unwrap().0
}
const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    let empty = values
        .store(&[])
        .map_err(|error| format!("store continuous-tone render request: {error:?}"))?;
    Ok(BrowserBack::installed_step(AudioToneBack {
        empty,
        next_request: 0,
        pending: None,
        frequency_ready: false,
        closed: false,
    }))
}

pub(crate) struct PreparedAudioTone {
    frequency: Option<i128>,
    phase: u32,
    start_frame: u64,
}

impl PreparedAudioTone {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        Ok(Some(Self {
            frequency: None,
            phase: 0,
            start_frame: 0,
        }))
    }

    pub(crate) fn update(&mut self, encoded: &[u8]) -> Result<(), Failure> {
        self.frequency = Quantity::decode(encoded)
            .ok()
            .and_then(frequency_millihertz);
        self.frequency.map(|_| ()).ok_or(Failure {
            code: FailureCode::InvalidInput,
            detail: 12,
        })
    }

    pub(crate) fn render(&mut self) -> Result<[u8; BLOCK_BYTES], Failure> {
        let frequency = self.frequency.ok_or(Failure {
            code: FailureCode::InvalidInput,
            detail: 13,
        })?;
        let (block, phase) = render(frequency, self.phase, self.start_frame).ok_or(Failure {
            code: FailureCode::InvalidInput,
            detail: 14,
        })?;
        self.phase = phase;
        self.start_frame = self.start_frame.wrapping_add(u64::from(FRAMES));
        Ok(block)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admitted_renderer_keeps_phase_and_frame_continuity_in_hosted_pcm_storage() {
        let mut renderer = PreparedAudioTone {
            frequency: None,
            phase: 0,
            start_frame: 0,
        };
        renderer
            .update(&Quantity::new(440, QuantityUnit::Hertz).encode())
            .unwrap();
        let first = renderer.render().unwrap();
        let second = renderer.render().unwrap();
        let (first_header, first_payload) = PcmFrameHeader::decode_frame(&first).unwrap();
        let (second_header, second_payload) = PcmFrameHeader::decode_frame(&second).unwrap();

        assert_eq!(first_header.frame_count, FRAMES);
        assert_eq!(first_header.start_frame, 0);
        assert_eq!(second_header.start_frame, u64::from(FRAMES));
        assert_eq!(first_payload.len(), usize::from(FRAMES) * 2);
        assert_eq!(second_payload.len(), usize::from(FRAMES) * 2);
        assert_ne!(first_payload, second_payload);
    }
}
