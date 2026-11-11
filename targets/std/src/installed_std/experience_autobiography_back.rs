//! Pure episode retention; the autobiography itself travels through State Cords.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{
    BoundedExperienceAutobiographyRetainer, ExperienceAutobiographyRefusal,
    EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EXPERIENCE_AUTOBIOGRAPHY_RETAIN_IMPLEMENTATION,
    budget,
    prepare,
};
const PENDING: PortId = PortId(0);
const RESULT: PortId = PortId(1);
pub(super) struct ExperienceAutobiographyBack {
    retainer: BoundedExperienceAutobiographyRetainer,
    staged: bool,
    closed: [bool; 2],
    complete: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for ExperienceAutobiographyBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if let (Some(pending), Some(result)) = (bytes.input(PENDING), bytes.input(RESULT)) {
            let length = match self.retainer.execute(pending, result) {
                Ok(output) => output.len() as u32,
                Err(reason) => return failure(detail(reason)),
            };
            if !io.output_ready_for(PortId(0), length) {
                return StepOutcome::Await;
            }
            if io.consume(PENDING).is_err()
                || io.consume(RESULT).is_err()
                || io.send_prepared(PortId(0), length).is_err()
            {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::WorkBudgetExhausted,
                    detail: 0,
                });
            }
            self.staged = true;
            return StepOutcome::Progress;
        }
        for (index, port) in [PENDING, RESULT].into_iter().enumerate() {
            if !self.closed[index] && io.input_closed(port) {
                if io.consume_closed(port).is_err() {
                    return failure(20);
                }
                self.closed[index] = true;
                return StepOutcome::Progress;
            }
        }
        if (self.closed[0] && bytes.input(RESULT).is_some())
            || (self.closed[1] && bytes.input(PENDING).is_some())
        {
            return failure(21);
        }
        if self.closed.iter().all(|closed| *closed) {
            self.complete = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then(|| self.retainer.output())
    }
    fn step_committed(&mut self) {
        self.staged = false;
    }
    fn cancel(&mut self) {
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
fn detail(reason: ExperienceAutobiographyRefusal) -> u16 {
    use ExperienceAutobiographyRefusal::*;
    match reason {
        Malformed => 1,
        ByteBound => 2,
        StaleRevision => 3,
        FutureRevision => 4,
        RevisionOverflow => 5,
        InvalidHistory => 6,
        InvalidResult => 7,
        Evidence(reason) => 100 + super::experience_evidence_back::refusal_detail(reason),
    }
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    super::episode_loop_back::exact_pure_gear(
        gear,
        &conduit_std_offers::experience_autobiography_retain_offer(),
    )?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as u32,
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
    Ok(InstalledBack::ExperienceAutobiography(
        ExperienceAutobiographyBack {
            retainer: BoundedExperienceAutobiographyRetainer::prepare(),
            staged: false,
            closed: [false; 2],
            complete: false,
        },
    ))
}
