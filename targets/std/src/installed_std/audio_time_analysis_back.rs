//! Exact finite PCM observation Back. No Host Call, device or speech inference.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_audio::{
    PcmTimeAnalysisRefusal as R, PreparedPcmTimeAnalysis, PreparedPcmTimeObservationEncoder,
};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{Failure, FailureCode, PortId};
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::AUDIO_TIME_ANALYSIS_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct AudioTimeAnalysisBack {
    encoder: PreparedPcmTimeObservationEncoder,
    clock: u64,
    rate: u32,
    offset: u16,
    frames: u16,
    remaining: u16,
    closed: bool,
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
impl<const PORTS: usize> StepBack<PORTS> for AudioTimeAnalysisBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.closed {
            return StepOutcome::Complete;
        }
        if io.input(PortId(0)).is_some() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            if self.remaining == 0 {
                return fail(FailureCode::WorkBudgetExhausted, 1);
            }
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(FailureCode::InvalidInput, 1);
            };
            let analysis = match PreparedPcmTimeAnalysis::prepare(
                bytes,
                self.clock,
                self.rate,
                self.offset,
                self.frames,
            ) {
                Ok(value) => value,
                Err(reason) => {
                    return fail(
                        FailureCode::InvalidInput,
                        match reason {
                            R::Frame(_) => 1,
                            R::UnsupportedLayout => 2,
                            R::WrongTimebase => 3,
                            R::Discontinuous => 4,
                            R::InsufficientWindow => 5,
                            R::WindowOutsideFrame => 6,
                            R::FrameIndexOverflow => 7,
                            R::NonfiniteSample => 8,
                        },
                    )
                }
            };
            if self.encoder.encode(&analysis).is_err() {
                return fail(FailureCode::InvalidInput, 9);
            }
            io.consume(PortId(0)).expect("observed PCM input");
            io.send_prepared(PortId(0), self.encoder.maximum_output_bytes() as u32)
                .expect("ready observation");
            self.remaining -= 1;
            return StepOutcome::Progress;
        }
        if !self.closed && io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).expect("observed PCM close");
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| self.encoder.encoded())
    }
    fn cancel(&mut self) {
        self.closed = true;
        self.remaining = 0;
    }
}
fn configured(placement: &PlannedGear) -> Result<AudioTimeAnalysisBack, String> {
    let expected = conduit_std_offers::audio_time_analysis_offer();
    if placement.kind_id != expected.kind_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || !placement.host_calls.is_empty()
    {
        return Err("PCM analysis planned identity differs from admitted profile".into());
    }
    let config = |key: &str, min, max| -> Result<u64, String> {
        match placement
            .configuration
            .iter()
            .find(|c| c.key == key)
            .map(|c| &c.value)
        {
            Some(ConfigurationValue::U64(v)) if (min..=max).contains(v) => Ok(*v),
            _ => Err(format!("PCM analysis invalid {key}")),
        }
    };
    if placement.configuration.len() != 5 {
        return Err("PCM analysis configuration count differs".into());
    }
    let frames = config("frames", 2, 2048)? as u16;
    let offset = config("offset", 0, 2046)? as u16;
    if u32::from(frames) + u32::from(offset) > 2048 {
        return Err("PCM analysis window exceeds admitted frame".into());
    }
    Ok(AudioTimeAnalysisBack {
        encoder: PreparedPcmTimeObservationEncoder::prepare(frames)
            .map_err(|e| format!("PCM analysis output preparation: {e:?}"))?,
        clock: config("clock-id", 1, u64::MAX)?,
        rate: config("sample-rate-hz", 8000, 192000)? as u32,
        offset,
        frames,
        remaining: config("maximum-blocks", 1, 64)? as u16,
        closed: false,
    })
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let value = configured(placement)?;
    let maximum = value
        .encoder
        .maximum_output_bytes()
        .max(conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN + 2048 * 4) as u32;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: maximum * 2,
        host_requests: 0,
        sign_items: 2048,
        maximum_value_bytes: maximum,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    configured(placement).map(InstalledBack::AudioTimeAnalysis)
}
