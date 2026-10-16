//! A transport-only PCM packing Gear; acoustic policy stays in authored Source.
use conduit_audio::PreparedPcm16Packing;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub struct PreparedPcm16PackingBack {
    encoder: PreparedPcm16Packing,
    remaining: u16,
    staged: bool,
    closed: bool,
}
impl PreparedPcm16PackingBack {
    pub fn prepare(maximum_blocks: u16) -> Result<Self, String> {
        if !(1..=64).contains(&maximum_blocks) {
            return Err("PCM packing requires 1..=64 admitted inputs".into());
        }
        Ok(Self {
            encoder: PreparedPcm16Packing::prepare().map_err(|e| format!("PCM packing: {e:?}"))?,
            remaining: maximum_blocks,
            staged: false,
            closed: false,
        })
    }
}
impl<const PORTS: usize> StepBack<PORTS> for PreparedPcm16PackingBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        self.staged = false;
        if self.closed {
            return StepOutcome::Complete;
        }
        if io.input_abnormal(PortId(0)).is_some() {
            return fail(FailureCode::InvalidInput, 1650);
        }
        let Some(reference) = io.input(PortId(0)) else {
            return if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_err() {
                    return fail(FailureCode::InvalidInput, 1651);
                }
                StepOutcome::Complete
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if self.remaining == 0 {
            return fail(FailureCode::WorkBudgetExhausted, 1652);
        }
        let Some(input) = inputs.input(PortId(0)) else {
            return fail(FailureCode::InvalidInput, 1653);
        };
        if input.len() != reference.byte_len as usize {
            return fail(FailureCode::InvalidInput, 1654);
        }
        match self.encoder.pack(input) {
            Ok(Some(output)) => {
                if io.send_prepared(PortId(0), output.len() as u32).is_err() {
                    return fail(FailureCode::InvalidInput, 1655);
                }
            }
            Ok(None) => {}
            Err(_) => return fail(FailureCode::InvalidInput, 1656),
        }
        if io.consume(PortId(0)).is_err() {
            return fail(FailureCode::InvalidInput, 1657);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if self.staged && port == PortId(0) {
            self.encoder.output()
        } else {
            None
        }
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.remaining -= 1;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.closed = true;
        self.staged = false;
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
