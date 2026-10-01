//! Finite PCM amplitude scaling through exact admitted browser Host Calls.
use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{HostCallContractId, HostCallRequirement, PlannedGear, Scalar};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-audio-apply-gain@1";
pub(crate) const UPDATE_OPERATION: &str = "conduit.host/browser-audio-gain-update@1";
pub(crate) const SCALE_OPERATION: &str = "conduit.host/browser-audio-gain-scale@1";
const BLOCK_BYTES: usize = conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_PCM_BLOCK_BYTES as usize;
pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::audio_gain_contract(),
        conduit_semantic_catalog::AUDIO_GAIN_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/audio-apply-gain@1",
        },
        vec![
            HostCallRequirement {
                contract_id: HostCallContractId::from(SCALE_OPERATION),
                target_kind: Some(conduit_core::kind_id(
                    conduit_semantic_catalog::AUDIO_GAIN_KIND,
                )),
                maximum_in_flight: 1,
                maximum_input_bytes: BLOCK_BYTES as u32,
                maximum_output_bytes: BLOCK_BYTES as u32,
            },
            HostCallRequirement {
                contract_id: HostCallContractId::from(UPDATE_OPERATION),
                target_kind: Some(conduit_core::kind_id(
                    conduit_semantic_catalog::AUDIO_GAIN_KIND,
                )),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
                maximum_output_bytes: 0,
            },
        ],
        Vec::new(),
        Vec::new(),
    )
}

struct GainBack {
    next_request: u32,
    pending: Option<GainRequest>,
    amplitude_ready: bool,
    closed: bool,
}
#[derive(Clone, Copy)]
enum GainRequest {
    Update(RequestId),
    Scale(RequestId),
}
impl<const PORTS: usize> StepBack<PORTS> for GainBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            match self.pending {
                Some(GainRequest::Update(expected)) if expected == request => {
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                    {
                        return fail(8);
                    }
                    io.consume_host_completion().expect("observed gain update");
                    self.pending = None;
                    self.amplitude_ready = true;
                    return StepOutcome::Progress;
                }
                Some(GainRequest::Scale(expected)) if expected == request => {
                    let Some(output) = outcome.output else {
                        return fail(9);
                    };
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                        || output.value.byte_len > BLOCK_BYTES as u32
                        || output.admitted_bytes != BLOCK_BYTES as u32
                    {
                        return fail(9);
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion().expect("observed scaled PCM");
                    io.send(PortId(0), output.value).expect("ready scaled PCM");
                    self.pending = None;
                    return StepOutcome::Progress;
                }
                _ => return fail(10),
            }
        }
        if let Some(value) = io.input(PortId(1)) {
            if self.pending.is_some() {
                return StepOutcome::Await;
            }
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return fail(11);
            };
            io.consume(PortId(1)).expect("present amplitude");
            io.request_host_call(
                request,
                HostCallId(1),
                BoundedValueRef::new(value, conduit_core::SCALAR_ENCODED_LEN as u32)
                    .expect("bounded Scalar"),
            )
            .expect("gain update Host Call");
            self.next_request = next;
            self.pending = Some(GainRequest::Update(request));
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || !self.amplitude_ready || !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let request = RequestId(self.next_request);
            let Some(next) = self.next_request.checked_add(1) else {
                return fail(11);
            };
            io.consume(PortId(0)).expect("present PCM");
            io.request_host_call(
                request,
                HostCallId(0),
                BoundedValueRef::new(value, BLOCK_BYTES as u32).expect("bounded PCM"),
            )
            .expect("gain scale Host Call");
            self.next_request = next;
            self.pending = Some(GainRequest::Scale(request));
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
    Ok(BrowserBack::installed_step(GainBack {
        next_request: 0,
        pending: None,
        amplitude_ready: false,
        closed: false,
    }))
}

pub(crate) struct PreparedAudioGain {
    amplitude: Option<i64>,
}
impl PreparedAudioGain {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        validate_placement(placement, &offer())?;
        Ok(Some(Self { amplitude: None }))
    }
    pub(crate) fn update(&mut self, encoded: &[u8]) -> Result<(), Failure> {
        let amplitude = Scalar::decode(encoded)
            .map_err(|_| failure(2))?
            .raw_microunits();
        if !(0..=1_000_000).contains(&amplitude) {
            return Err(failure(3));
        }
        self.amplitude = Some(amplitude);
        Ok(())
    }
    pub(crate) fn scale(&self, bytes: &[u8]) -> Result<([u8; BLOCK_BYTES], usize), Failure> {
        let amplitude = self.amplitude.ok_or(failure(4))?;
        let (header, payload) =
            conduit_audio::PcmFrameHeader::decode_frame(bytes).map_err(|_| failure(5))?;
        if header.representation != conduit_audio::PcmSampleRepresentation::Signed16LittleEndian
            || header.validate_payload(payload).is_err()
            || bytes.len() > BLOCK_BYTES
        {
            return Err(failure(6));
        }
        let mut encoded = [0_u8; BLOCK_BYTES];
        encoded[..bytes.len()].copy_from_slice(bytes);
        for sample in encoded[conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN..bytes.len()]
            .as_chunks_mut::<2>()
            .0
        {
            let value = i16::from_le_bytes([sample[0], sample[1]]);
            let scaled = (i64::from(value) * amplitude / 1_000_000)
                .clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16;
            sample.copy_from_slice(&scaled.to_le_bytes());
        }
        Ok((encoded, bytes.len()))
    }
}
const fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admitted_scaler_handles_a_continuous_pcm_block() {
        let mut gain = PreparedAudioGain { amplitude: None };
        gain.update(&Scalar::from_raw_microunits(500_000).encode())
            .unwrap();
        let tone = crate::installed_browser::audio_tone::test_block();
        let (scaled, length) = gain.scale(&tone).unwrap();
        assert_eq!(length, BLOCK_BYTES);
        assert_ne!(scaled, tone);
    }
}
