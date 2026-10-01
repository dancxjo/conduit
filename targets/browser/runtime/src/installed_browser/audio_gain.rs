//! Pure finite PCM amplitude scaling for browser plans.
use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{PlannedGear, Scalar};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, Failure, FailureCode, PortId,
};

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-audio-apply-gain@1";
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
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}
struct GainBack {
    amplitude: Option<i64>,
    closed: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for GainBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if io.input(PortId(1)).is_some() {
            let Some(bytes) = inputs.input(PortId(1)) else {
                return fail(1);
            };
            let Ok(value) = Scalar::decode(bytes) else {
                return fail(2);
            };
            if !(0..=1_000_000).contains(&value.raw_microunits()) {
                return fail(3);
            }
            self.amplitude = Some(value.raw_microunits());
            io.consume(PortId(1)).expect("present amplitude");
            return StepOutcome::Progress;
        }
        if io.input(PortId(0)).is_some() {
            let Some(amplitude) = self.amplitude else {
                return StepOutcome::Await;
            };
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(4);
            };
            let Ok((header, payload)) = conduit_audio::PcmFrameHeader::decode_frame(bytes) else {
                return fail(5);
            };
            if header.representation != conduit_audio::PcmSampleRepresentation::Signed16LittleEndian
                || header.validate_payload(payload).is_err()
                || bytes.len()
                    > conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_PCM_BLOCK_BYTES as usize
            {
                return fail(6);
            }
            let mut encoded =
                [0_u8; conduit_semantic_catalog::AUDIO_CONTINUOUS_TONE_PCM_BLOCK_BYTES as usize];
            encoded[..bytes.len()].copy_from_slice(bytes);
            for sample in encoded[conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN..bytes.len()]
                .as_chunks_mut::<2>()
                .0
            {
                let value = i16::from_le_bytes([sample[0], sample[1]]);
                let scaled = (i64::from(value) * amplitude / 1_000_000)
                    .clamp(i64::from(i16::MIN), i64::from(i16::MAX))
                    as i16;
                sample.copy_from_slice(&scaled.to_le_bytes());
            }
            let Ok(value) = CanonicalValue::new(&encoded[..bytes.len()]) else {
                return fail(7);
            };
            io.consume(PortId(0)).expect("present PCM");
            io.send_canonical(PortId(0), value)
                .expect("ready scaled PCM");
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
        amplitude: None,
        closed: false,
    }))
}
