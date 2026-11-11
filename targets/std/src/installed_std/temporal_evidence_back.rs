//! One bounded retrieval window per ordinary committed kernel step.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_ai::{BoundedTemporalEvidenceFlow, TEMPORAL_EVIDENCE_FLOW_MAX_BYTES as MAX};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
const WINDOW: PortId = PortId(0);
const INTENT: PortId = PortId(1);
const OUTCOME: PortId = PortId(0);
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::TEMPORAL_EVIDENCE_FLOW_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct TemporalEvidenceBack {
    selector: BoundedTemporalEvidenceFlow,
    intent: Vec<u8>,
    candidate: Vec<u8>,
    intent_staged: bool,
    staged: bool,
    complete: bool,
}
impl TemporalEvidenceBack {
    fn prepare() -> Self {
        Self {
            selector: BoundedTemporalEvidenceFlow::prepare(),
            intent: Vec::with_capacity(MAX),
            candidate: Vec::with_capacity(MAX),
            intent_staged: false,
            staged: false,
            complete: false,
        }
    }
    pub(super) fn allocation_capacity(&self) -> usize {
        self.selector.allocation_capacity() + self.intent.capacity() + self.candidate.capacity()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for TemporalEvidenceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if bytes.input(WINDOW).is_none() && io.input_closed(WINDOW) {
            if io.consume_closed(WINDOW).is_err() {
                return failure(1);
            }
            if bytes.input(INTENT).is_some() && io.consume(INTENT).is_err() {
                return failure(1);
            }
            self.complete = true;
            return StepOutcome::Complete;
        }
        // Current packets are replacements, not values replayed per window.
        // Commit a simultaneous replacement first and leave the Flow untouched.
        if let Some(intent) = bytes.input(INTENT) {
            if io
                .input(INTENT)
                .is_none_or(|reference| reference.byte_len as usize != intent.len())
            {
                return failure(1);
            }
            if let Err(reason) = self.selector.validate_intent(intent) {
                return failure(refusal_detail(reason));
            }
            self.candidate.clear();
            self.candidate.extend_from_slice(intent);
            if io.consume(INTENT).is_err() {
                return failure(1);
            }
            self.intent_staged = true;
            return StepOutcome::Progress;
        }
        let Some(window) = bytes.input(WINDOW) else {
            return StepOutcome::Await;
        };
        if self.intent.is_empty() || !io.output_ready(OUTCOME) {
            return StepOutcome::Await;
        }
        let output = match self.selector.execute(window, &self.intent) {
            Ok(output) => output,
            Err(reason) => return failure(refusal_detail(reason)),
        };
        let length = output.len() as u32;
        if !io.output_ready_for(OUTCOME, length) {
            return StepOutcome::Await;
        }
        if io.consume(WINDOW).is_err() || io.send_prepared(OUTCOME, length).is_err() {
            return failure(3);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == OUTCOME).then(|| self.selector.output())
    }
    fn step_committed(&mut self) {
        if self.intent_staged {
            core::mem::swap(&mut self.intent, &mut self.candidate);
            self.candidate.clear();
            self.intent_staged = false;
        }
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.intent.clear();
        self.candidate.clear();
        self.intent_staged = false;
        self.staged = false;
        self.complete = true;
    }
}
fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
fn refusal_detail(reason: conduit_ai::TemporalEvidenceFlowRefusal) -> u16 {
    use conduit_ai::{TemporalEvidenceFlowRefusal as R, TemporalEvidenceSelectionRefusal as S};
    match reason {
        R::Malformed => 10,
        R::ByteBound => 11,
        R::Selection(reason) => {
            100 + match reason {
                S::EmptyCandidates => 1,
                S::TooManyCandidates => 2,
                S::EmptyIdentity => 3,
                S::IdentityTooLarge => 4,
                S::DuplicateIdentity => 5,
                S::InvalidReference => 6,
                S::InvalidIntent => 7,
                S::InvalidProvenance => 8,
                S::ReferenceMismatch => 9,
                S::MissingSourceTime => 10,
            }
        }
    }
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    super::episode_loop_back::exact_pure_gear(
        gear,
        &conduit_std_offers::temporal_evidence_flow_offer(),
    )?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: MAX as u32,
        maximum_value_bytes: MAX as u32,
        host_requests: 0,
        sign_items: 8,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::TemporalEvidence(
        TemporalEvidenceBack::prepare(),
    ))
}
#[cfg(test)]
mod tests;
