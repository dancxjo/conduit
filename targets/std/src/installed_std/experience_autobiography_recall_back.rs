//! One event waits for the committed autobiography from the prior episode.
//! The only retained private fact is the last issued episode number; records
//! themselves arrive through the ordinary typed State Cord.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{validate_canonical_structured_value, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{
    BoundedExperienceAutobiographyRecall, ExperienceAutobiographyRecallRefusal,
    EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EXPERIENCE_AUTOBIOGRAPHY_RECALL_IMPLEMENTATION,
    budget,
    prepare,
};
const EVENT: PortId = PortId(0);
const MEMORY: PortId = PortId(1);
const UPDATE: PortId = PortId(0);
const PENDING: PortId = PortId(1);
pub(super) struct ExperienceAutobiographyRecallBack {
    recall: BoundedExperienceAutobiographyRecall,
    state_type: Vec<u8>,
    issued: u64,
    staged: bool,
    complete: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for ExperienceAutobiographyRecallBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        let Some(memory) = bytes.input(MEMORY) else {
            return StepOutcome::Await;
        };
        let completed = match completed(memory, &self.state_type) {
            Some(value) => value,
            None => return failure(20),
        };
        if completed < self.issued {
            return StepOutcome::Await;
        }
        if completed > self.issued {
            return failure(21);
        }
        let Some(event) = bytes.input(EVENT) else {
            if io.input_closed(EVENT) {
                if io.consume_closed(EVENT).is_err() || io.consume(MEMORY).is_err() {
                    return failure(22);
                }
                self.complete = true;
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        };
        if !io.output_ready(UPDATE) || !io.output_ready(PENDING) {
            return StepOutcome::Await;
        }
        if self.issued == u64::MAX {
            return failure(23);
        }
        if let Err(reason) = self.recall.execute(event, memory) {
            return failure(detail(reason));
        }
        let update = self.recall.update().len() as u32;
        let pending = self.recall.pending().len() as u32;
        if !io.output_ready_for(UPDATE, update) || !io.output_ready_for(PENDING, pending) {
            return StepOutcome::Await;
        }
        if io.consume(EVENT).is_err()
            || io.consume(MEMORY).is_err()
            || io.send_prepared(UPDATE, update).is_err()
            || io.send_prepared(PENDING, pending).is_err()
        {
            return StepOutcome::Fail(Failure {
                code: FailureCode::WorkBudgetExhausted,
                detail: 0,
            });
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if !self.staged {
            return None;
        }
        match port {
            UPDATE => Some(self.recall.update()),
            PENDING => Some(self.recall.pending()),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        if core::mem::take(&mut self.staged) {
            self.issued += 1;
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.complete = true;
    }
}
fn completed(input: &[u8], expected: &[u8]) -> Option<u64> {
    if input.len() > EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES {
        return None;
    }
    let state = validate_canonical_structured_value(input).ok()?;
    if state.type_bytes() != expected {
        return None;
    }
    let count = state.record_field("completed").ok()??;
    let node = count.value_node();
    if node.len() != 13 || node[..5] != [0, 8, 0, 0, 0] {
        return None;
    }
    Some(u64::from_le_bytes(node[5..].try_into().ok()?))
}
fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
fn detail(reason: ExperienceAutobiographyRecallRefusal) -> u16 {
    use ExperienceAutobiographyRecallRefusal::*;
    match reason {
        Malformed => 1,
        ByteBound => 2,
        ItemBound => 3,
        SourceBound => 4,
        IdentityBound => 5,
        InvalidHistory => 6,
        RevisionOverflow => 7,
        Evidence(reason) => 100 + super::experience_evidence_back::refusal_detail(reason),
    }
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    super::episode_loop_back::exact_pure_gear(
        gear,
        &conduit_std_offers::experience_autobiography_recall_offer(),
    )?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES * 2) as u32,
        maximum_value_bytes: EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::ExperienceAutobiographyRecall(
        ExperienceAutobiographyRecallBack {
            recall: BoundedExperienceAutobiographyRecall::prepare(),
            state_type: conduit_semantic_catalog::experience_autobiography_state_type()
                .canonical_bytes()
                .unwrap(),
            issued: 0,
            staged: false,
            complete: false,
        },
    ))
}
