//! Single-transaction generic three-frame window and next-history operation.
//! Inputs and next history are explicit; the caller owns recurrence and cadence.
use crate::fixed_numeric_catalog::fixed_numeric_type;
use crate::fixed_numeric_codec::{FixedCodecRefusal, FixedF32VectorCodec};
use crate::fixed_numeric_preparation::{
    fixed_window_offer, verify_fixed_placement, FixedPlannedRefusal,
};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixedWindowPreparationRefusal {
    Codec(FixedCodecRefusal),
    Planned(FixedPlannedRefusal),
    StepBudget,
}
pub struct FixedWindowBack {
    value: FixedF32VectorCodec<64>,
    history: FixedF32VectorCodec<128>,
    result: FixedF32VectorCodec<320>,
    value_port: PortId,
    history_port: PortId,
    staged: bool,
    finished: bool,
    cancelled: bool,
}
impl FixedWindowBack {
    pub fn prepare_planned<const PORTS: usize>(
        placement: &PlannedGear,
        fuel: u16,
    ) -> Result<Self, FixedWindowPreparationRefusal> {
        if PORTS < 2 || fuel < 3 {
            return Err(FixedWindowPreparationRefusal::StepBudget);
        }
        let expected = fixed_window_offer().map_err(|_| {
            FixedWindowPreparationRefusal::Planned(FixedPlannedRefusal::UnsupportedShape)
        })?;
        verify_fixed_placement(placement, &expected)
            .map_err(FixedWindowPreparationRefusal::Planned)?;
        let ordinal = |name: &str| {
            PortId(
                placement
                    .inputs
                    .iter()
                    .position(|p| p.port_id.as_str() == name)
                    .expect("verified port") as u16,
            )
        };
        Ok(Self {
            value: FixedF32VectorCodec::prepare(
                &fixed_numeric_type("NumericF32Vector64")
                    .map_err(|_| FixedWindowPreparationRefusal::Codec(FixedCodecRefusal::Shape))?,
            )
            .map_err(FixedWindowPreparationRefusal::Codec)?,
            history: FixedF32VectorCodec::prepare_history()
                .map_err(FixedWindowPreparationRefusal::Codec)?,
            result: FixedF32VectorCodec::prepare_window_result()
                .map_err(FixedWindowPreparationRefusal::Codec)?,
            value_port: ordinal("value"),
            history_port: ordinal("history"),
            staged: false,
            finished: false,
            cancelled: false,
        })
    }
    pub fn output(&self) -> &[u8] {
        self.result.encoded()
    }
    pub fn has_committed_result(&self) -> bool {
        self.finished
    }
    pub fn allocation_capacity(&self) -> usize {
        self.value.allocation_capacity()
            + self.history.allocation_capacity()
            + self.result.allocation_capacity()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for FixedWindowBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 1200,
            });
        }
        if self.finished {
            return StepOutcome::Complete;
        }
        if PORTS < 2 || (0..2).any(|p| io.input_closed(PortId(p))) {
            return fail(1201);
        }
        if (0..2).any(|p| io.input(PortId(p)).is_none()) || !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(value), Some(history)) = (
            inputs.input(self.value_port),
            inputs.input(self.history_port),
        ) else {
            return fail(1202);
        };
        let mut v = [0.; 64];
        let mut h = [0.; 128];
        let mut result = [0.; 320];
        if self.value.decode(value, &mut v).is_err()
            || self.history.decode(history, &mut h).is_err()
        {
            return fail(1203);
        }
        // Canonical record fields sort by name: next_history before window.
        result[..64].copy_from_slice(&h[64..]);
        result[64..128].copy_from_slice(&v);
        result[128..256].copy_from_slice(&h);
        result[256..].copy_from_slice(&v);
        if self.result.encode(&result).is_err() {
            return fail(1204);
        }
        for p in 0..2 {
            if io.consume(PortId(p)).is_err() {
                return fail(1205);
            }
        }
        if io
            .send_prepared(PortId(0), self.result.maximum_bytes() as u32)
            .is_err()
        {
            return fail(1206);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.finished = true;
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then(|| self.result.encoded())
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
    }
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
