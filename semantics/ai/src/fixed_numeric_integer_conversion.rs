//! Generic exact integer→f32 conversion; scaling and sample-rate meaning remain Source-owned.
use crate::{
    fixed_numeric_catalog::fixed_numeric_type, fixed_numeric_codec::FixedF32VectorCodec,
    fixed_numeric_dsp_catalog::*, fixed_numeric_i16_codec::FixedI16VectorCodec,
    fixed_numeric_preparation::verify_fixed_placement,
};
use alloc::{format, string::String};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub struct FixedIntegerConversionBack<const N: usize> {
    input: Option<FixedI16VectorCodec<N>>,
    raw: Option<FixedF32VectorCodec<N>>,
    output: FixedF32VectorCodec<N>,
    flow: bool,
    staged: bool,
    finished: bool,
    cancelled: bool,
    committed_frames: u64,
}
impl<const N: usize> FixedIntegerConversionBack<N> {
    pub fn prepare_planned<const PORTS: usize>(
        gear: &PlannedGear,
        fuel: u16,
        operation: IntegerConversion,
        flow: bool,
    ) -> Result<Self, String> {
        if N != operation.width() || PORTS < 1 || fuel < 2 {
            return Err("integer conversion shape/step bound".into());
        }
        verify_fixed_placement(gear, &fixed_integer_conversion_offer(operation, flow)?)
            .map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            input: if matches!(
                operation,
                IntegerConversion::I16Vector160 | IntegerConversion::I16Vector80
            ) {
                Some(
                    FixedI16VectorCodec::prepare(&fixed_numeric_type(&format!(
                        "NumericI16Vector{N}"
                    ))?)
                    .map_err(|error| format!("{error:?}"))?,
                )
            } else {
                None
            },
            raw: if matches!(operation, IntegerConversion::FiniteVector(_)) {
                Some(
                    FixedF32VectorCodec::prepare_raw(&fixed_numeric_type(&format!(
                        "NumericRawF32Vector{N}"
                    ))?)
                    .map_err(|error| format!("{error:?}"))?,
                )
            } else {
                None
            },
            output: FixedF32VectorCodec::prepare(&fixed_numeric_type(&format!(
                "NumericF32Vector{N}"
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
impl<const N: usize, const PORTS: usize> StepBack<PORTS> for FixedIntegerConversionBack<N> {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 2820,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) {
            return if self.flow {
                StepOutcome::Complete
            } else {
                fail(2821)
            };
        }
        if self.committed_frames == u64::MAX {
            return fail(2822);
        }
        if io.input(PortId(0)).is_none() || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(bytes) = inputs.input(PortId(0)) else {
            return fail(2823);
        };
        let mut values = [0.; N];
        if let Some(codec) = &self.input {
            let mut signed = [0; N];
            if codec.decode(bytes, &mut signed).is_err() {
                return fail(2824);
            }
            for (output, input) in values.iter_mut().zip(signed) {
                *output = f32::from(input);
            }
        } else if let Some(codec) = &self.raw {
            if codec.decode(bytes, &mut values).is_err() {
                return fail(2824);
            }
        } else {
            let Ok(raw) = <[u8; 2]>::try_from(bytes) else {
                return fail(2824);
            };
            values[0] = f32::from(u16::from_le_bytes(raw));
        }
        if self.output.encode(&values).is_err() {
            return fail(2825);
        }
        if io.consume(PortId(0)).is_err()
            || io
                .send_prepared(PortId(0), self.output.encoded().len() as u32)
                .is_err()
        {
            return fail(2826);
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
