//! Host-scoped registration of a finite pure application Back.
use super::back::BackBudget;
use conduit_core::{CapabilityOffer, PlannedGear, PortTemporal};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

const PORT: PortId = PortId(0);
type Transform = fn(&[u8]) -> Result<Vec<u8>, u16>;

/// A trusted native byte transform installed for one exact typed Flow offer.
/// The Host owns its selection and bounds. The scheduler owns every step.
pub struct HostedFrameBack {
    offer: CapabilityOffer,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
    transform: Transform,
}

impl HostedFrameBack {
    pub fn new(
        offer: CapabilityOffer,
        maximum_input_bytes: u32,
        maximum_output_bytes: u32,
        transform: Transform,
    ) -> Result<Self, String> {
        let flow = PortTemporal::Flow { closes: true };
        if super::catalog::factory(&offer.implementation.implementation_id).is_some()
            || offer.inputs.len() != 1
            || offer.outputs.len() != 1
            || offer.inputs[0].temporal != flow
            || offer.outputs[0].temporal != flow
            || offer.inputs[0].abnormal_kind.is_some()
            || offer.outputs[0].abnormal_kind.is_some()
            || maximum_input_bytes == 0
            || maximum_output_bytes == 0
            || maximum_input_bytes.max(maximum_output_bytes) > offer.limits.max_queue_bytes
            || maximum_input_bytes
                .checked_add(maximum_output_bytes)
                .is_none()
            || !offer.host_calls.is_empty()
            || !offer.resource_requirements.is_empty()
            || !offer.authority_requirements.is_empty()
        {
            return Err(
                "hosted frame Back requires a finite pure one-input one-output Flow".into(),
            );
        }
        Ok(Self {
            offer,
            maximum_input_bytes,
            maximum_output_bytes,
            transform,
        })
    }

    pub(crate) fn offer(&self) -> &CapabilityOffer {
        &self.offer
    }

    pub(crate) fn matches(&self, gear: &PlannedGear) -> bool {
        super::episode_loop_back::exact_pure_gear(gear, &self.offer).is_ok()
    }

    pub(crate) fn budget(&self) -> BackBudget {
        BackBudget {
            value_items: 2,
            value_bytes: self.maximum_input_bytes + self.maximum_output_bytes,
            maximum_value_bytes: self.maximum_input_bytes.max(self.maximum_output_bytes),
            host_requests: 0,
            sign_items: 16,
        }
    }

    pub(super) fn prepare(&self) -> FrameStepBack {
        FrameStepBack {
            maximum_input_bytes: self.maximum_input_bytes as usize,
            maximum_output_bytes: self.maximum_output_bytes as usize,
            transform: self.transform,
            staged: None,
        }
    }
}

pub(super) struct FrameStepBack {
    maximum_input_bytes: usize,
    maximum_output_bytes: usize,
    transform: Transform,
    staged: Option<Vec<u8>>,
}

impl<const PORTS: usize> StepBack<PORTS> for FrameStepBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        let Some(input) = bytes.input(PORT) else {
            if io.input_closed(PORT) {
                return if io.consume_closed(PORT).is_ok() {
                    StepOutcome::Complete
                } else {
                    failure(FailureCode::InvalidLifecycle, 0)
                };
            }
            return StepOutcome::Await;
        };
        if !io.output_ready(PORT) {
            return StepOutcome::Await;
        }
        if input.len() > self.maximum_input_bytes {
            return failure(FailureCode::InvalidInput, 1);
        }
        let output = match (self.transform)(input) {
            Ok(output) => output,
            Err(detail) => return failure(FailureCode::InvalidInput, detail),
        };
        if output.len() > self.maximum_output_bytes {
            return failure(FailureCode::InvalidInput, 5);
        }
        if !io.output_ready_for(PORT, output.len() as u32) {
            return StepOutcome::Await;
        }
        if io.consume(PORT).is_err() || io.send_prepared(PORT, output.len() as u32).is_err() {
            return failure(FailureCode::WorkBudgetExhausted, 0);
        }
        self.staged = Some(output);
        StepOutcome::Progress
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if port == PORT {
            self.staged.as_deref()
        } else {
            None
        }
    }

    fn step_committed(&mut self) {
        self.staged = None;
    }

    fn cancel(&mut self) {
        self.staged = None;
    }
}

fn failure(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
