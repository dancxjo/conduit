//! Bounded spectral rows through ordinary Play; the input remains queued until its last row.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_audio::{
    PcmSpectralGeometry, PcmSpectralProfile, PcmSpectralWindow,
    PreparedPcmSpectrumObservationEncoder,
};
use conduit_core::{ConfigurationValue, PlannedGear, Unit};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{Failure, FailureCode, PortId};
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::AUDIO_SPECTRAL_ANALYSIS_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct AudioSpectralAnalysisBack {
    encoder: PreparedPcmSpectrumObservationEncoder,
    remaining: u16,
    window_index: u16,
    closed: bool,
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
impl<const PORTS: usize> StepBack<PORTS> for AudioSpectralAnalysisBack {
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
            if let Err(reason) = self.encoder.encode_window(bytes, self.window_index) {
                return fail(FailureCode::InvalidInput, refusal_detail(reason));
            }
            io.send_prepared(PortId(0), self.encoder.encoded_len() as u32)
                .expect("ready spectral row");
            self.window_index += 1;
            if self.window_index == self.encoder.profile().geometry().windows() {
                io.consume(PortId(0)).expect("observed PCM input");
                self.remaining -= 1;
                self.window_index = 0;
            }
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
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
        self.window_index = 0;
    }
}
fn configured(placement: &PlannedGear) -> Result<AudioSpectralAnalysisBack, String> {
    let expected = conduit_std_offers::audio_spectral_analysis_offer();
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
    if placement.configuration.len() != 7 {
        return Err("spectral configuration count differs".into());
    }
    let value = |key: &str| {
        placement
            .configuration
            .iter()
            .find(|c| c.key == key)
            .map(|c| &c.value)
    };
    let rate = match value("sample-rate") {
        Some(ConfigurationValue::Quantity(q)) => q
            .value()
            .to_i64(Unit::Hertz)
            .map_err(|_| "spectral rate must be exact integer Hz")?,
        _ => return Err("spectral rate requires Frequency".into()),
    };
    if !(8000..=192000).contains(&rate) {
        return Err("spectral rate outside profile".into());
    }
    let window = match value("window") {
        Some(ConfigurationValue::Text(v)) if v == "rectangular" => PcmSpectralWindow::Rectangular,
        Some(ConfigurationValue::Text(v)) if v == "periodic-hann" => {
            PcmSpectralWindow::PeriodicHann
        }
        _ => return Err("spectral window outside profile".into()),
    };
    let geometry = PcmSpectralGeometry::new(
        config("source-frames", 16, 2048)? as u16,
        config("window-frames", 16, 1024)? as u16,
        config("stride-frames", 1, 2048)? as u16,
    )
    .map_err(|e| format!("spectral geometry: {e:?}"))?;
    let profile = PcmSpectralProfile::new(
        geometry,
        config("clock-id", 1, u64::MAX)?,
        rate as u32,
        window,
    )
    .map_err(|e| format!("spectral profile: {e:?}"))?;
    Ok(AudioSpectralAnalysisBack {
        encoder: PreparedPcmSpectrumObservationEncoder::prepare(profile)
            .map_err(|e| format!("spectral output: {e:?}"))?,
        remaining: config("maximum-blocks", 1, 64)? as u16,
        window_index: 0,
        closed: false,
    })
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let value = configured(placement)?;
    let maximum = value
        .encoder
        .encoded_len()
        .max(conduit_audio::PCM_FRAME_HEADER_ENCODED_LEN + 2048 * 4) as u32;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: maximum * 2,
        host_requests: 0,
        sign_items: 8192,
        maximum_value_bytes: maximum,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    configured(placement).map(InstalledBack::AudioSpectralAnalysis)
}

fn refusal_detail(reason: conduit_audio::PcmSpectralEncodingRefusal) -> u16 {
    use conduit_audio::{
        PcmSpectralEncodingRefusal as E, PcmSpectralRefusal as S, PcmTimeAnalysisRefusal as T,
    };
    match reason {
        E::Analysis(S::Input(input)) => match input {
            T::Frame(_) => 1,
            T::UnsupportedLayout => 2,
            T::WrongTimebase => 3,
            T::Discontinuous => 4,
            T::InsufficientWindow => 5,
            T::WindowOutsideFrame => 6,
            T::FrameIndexOverflow => 7,
            T::NonfiniteSample => 8,
        },
        E::Analysis(S::UnsupportedWindow) => 9,
        E::Analysis(S::InvalidStride) => 10,
        E::Analysis(S::WrongSourceFrames) => 11,
        E::Analysis(S::TooManyWindows) => 12,
        E::Analysis(S::WindowIndexOutsideGeometry) => 13,
        E::Analysis(S::ArithmeticOverflow) => 14,
        E::Native(_) | E::Structured(_) => 15,
    }
}

#[cfg(test)]
#[path = "audio_spectral_analysis_back_tests.rs"]
mod tests;
