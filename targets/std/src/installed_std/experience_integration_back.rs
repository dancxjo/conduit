//! Transactional Experience state advanced only by ordinary committed Steps.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{
    BoundedExperienceIntegration, ExperienceIntegrationRefusal, EXPERIENCE_EVIDENCE_MAX_BYTES,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EXPERIENCE_INTEGRATION_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct ExperienceIntegrationBack {
    integration: BoundedExperienceIntegration,
    staged: bool,
    complete: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for ExperienceIntegrationBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(update) = bytes.input(PortId(0)) else {
            if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_err() {
                    return fail(FailureCode::InvalidLifecycle, 0);
                }
                self.complete = true;
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        };
        let length = match self.integration.execute(update) {
            Ok(output) => output.len() as u32,
            Err(refusal) => return fail(FailureCode::InvalidInput, detail(refusal)),
        };
        if !io.output_ready_for(PortId(0), length) {
            self.integration.cancel();
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err() || io.send_prepared(PortId(0), length).is_err() {
            self.integration.cancel();
            return fail(FailureCode::WorkBudgetExhausted, 0);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then(|| self.integration.output())
    }
    fn step_committed(&mut self) {
        if core::mem::take(&mut self.staged) {
            self.integration.commit();
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.integration.cancel();
        self.complete = true;
    }
}

fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
fn detail(refusal: ExperienceIntegrationRefusal) -> u16 {
    use ExperienceIntegrationRefusal::*;
    match refusal {
        Malformed => 1,
        ByteBound => 2,
        ItemBound => 3,
        StaleRevision => 4,
        FutureRevision => 5,
        RevisionOverflow => 6,
        UnknownIdentity => 7,
        PendingUpdate => 8,
        Evidence(reason) => 100 + super::experience_evidence_back::refusal_detail(reason),
    }
}

fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let expected = conduit_std_offers::experience_integration_offer();
    super::episode_loop_back::exact_pure_gear(gear, &expected)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: EXPERIENCE_EVIDENCE_MAX_BYTES as u32,
        maximum_value_bytes: EXPERIENCE_EVIDENCE_MAX_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::ExperienceIntegration(
        ExperienceIntegrationBack {
            integration: BoundedExperienceIntegration::prepare(),
            staged: false,
            complete: false,
        },
    ))
}
