//! Explicit Value and closing-Flow DSP owners, each bound to its exact selected Fore.
use crate::{
    fixed_numeric_catalog::fixed_numeric_type,
    fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_dsp::{self, FixedDspRefusal},
    fixed_numeric_dsp_catalog::*,
    fixed_numeric_preparation::verify_fixed_placement,
};
use alloc::{format, string::String};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub struct FixedDspBack<const IN: usize, const OUT: usize> {
    operation: FixedDspOperation,
    input: FixedF32VectorCodec<IN>,
    right: Option<FixedF32VectorCodec<IN>>,
    output: FixedF32VectorCodec<OUT>,
    flow: bool,
    staged: bool,
    finished: bool,
    cancelled: bool,
    committed_frames: u64,
}
impl<const IN: usize, const OUT: usize> FixedDspBack<IN, OUT> {
    pub fn prepare_planned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        operation: FixedDspOperation,
        flow: bool,
    ) -> Result<Self, String> {
        let inputs = if operation == FixedDspOperation::Dot160 {
            2
        } else {
            1
        };
        if operation.shape() != (IN, OUT) || PORTS < inputs || fuel < (inputs + 1) as u16 {
            return Err("DSP shape/step bound".into());
        }
        verify_fixed_placement(gear, &fixed_dsp_offer(operation, flow)?)
            .map_err(|error| format!("{error:?}"))?;
        let codec = || {
            FixedF32VectorCodec::prepare(&fixed_numeric_type(&format!("NumericF32Vector{IN}"))?)
                .map_err(|error| format!("{error:?}"))
        };
        Ok(Self {
            operation,
            input: codec()?,
            right: if inputs == 2 { Some(codec()?) } else { None },
            output: FixedF32VectorCodec::prepare(&fixed_numeric_type(&format!(
                "NumericF32Vector{OUT}"
            ))?)
            .map_err(|error| format!("{error:?}"))?,
            flow,
            staged: false,
            finished: false,
            cancelled: false,
            committed_frames: 0,
        })
    }
    pub fn committed_frames(&self) -> u64 {
        self.committed_frames
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const IN: usize, const OUT: usize, const PORTS: usize> StepBack<PORTS>
    for FixedDspBack<IN, OUT>
{
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2800,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        let count = if self.right.is_some() { 2 } else { 1 };
        let closed = (0..count)
            .filter(|&i| io.input_closed(PortId(i as u16)))
            .count();
        if closed > 0 {
            if !self.flow {
                return fail(2801);
            }
            if closed == count {
                return StepOutcome::Complete;
            }
            if (0..count).any(|i| io.input(PortId(i as u16)).is_some()) {
                return fail(2802);
            }
            return StepOutcome::Await;
        }
        if self.committed_frames == u64::MAX {
            return fail(2803);
        }
        if (0..count).any(|i| io.input(PortId(i as u16)).is_none()) || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let mut left = [0.; IN];
        let mut right = [0.; IN];
        let mut result = [0.; OUT];
        if inputs
            .input(PortId(0))
            .is_none_or(|bytes| self.input.decode(bytes, &mut left).is_err())
        {
            return fail(2804);
        }
        if let Some(codec) = &self.right {
            if inputs
                .input(PortId(1))
                .is_none_or(|bytes| codec.decode(bytes, &mut right).is_err())
            {
                return fail(2804);
            }
        }
        let computed = match self.operation {
            FixedDspOperation::RealDft320 => fixed_numeric_dsp::real_dft(&left, &mut result),
            FixedDspOperation::MagnitudeSquared161 => {
                fixed_numeric_dsp::magnitude_squared(&left, &mut result)
            }
            FixedDspOperation::Log10_18 | FixedDspOperation::Log10_1 => {
                let mut temp = [0.; IN];
                fixed_numeric_dsp::log10(&left, &mut temp).map(|()| result.copy_from_slice(&temp))
            }
            FixedDspOperation::Dot160 => {
                fixed_numeric_dsp::dot(&left, &right).map(|value| result[0] = value)
            }
            FixedDspOperation::Sqrt1 => {
                fixed_numeric_dsp::sqrt(left[0]).map(|value| result[0] = value)
            }
        };
        if let Err(reason) = computed {
            return fail(match reason {
                FixedDspRefusal::Shape => 2810,
                FixedDspRefusal::Nonfinite => 2811,
                FixedDspRefusal::Nonpositive => 2812,
                FixedDspRefusal::Negative => 2813,
            });
        }
        if self.output.encode(&result).is_err() {
            return fail(2814);
        }
        for i in 0..count {
            if io.consume(PortId(i as u16)).is_err() {
                return fail(2815);
            }
        }
        if io
            .send_prepared(PortId(0), self.output.encoded().len() as u32)
            .is_err()
        {
            return fail(2816);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then(|| self.output.encoded())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.committed_frames += 1;
            self.finished = !self.flow;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
