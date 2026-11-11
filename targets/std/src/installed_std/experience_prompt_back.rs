//! Finite wording of explicit evidence; the model remains an independent Gear.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{
    BoundedExperiencePrompt, ExperiencePromptRefusal, EXPERIENCE_PROMPT_MAX_BYTES,
};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EXPERIENCE_PROMPT_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct ExperiencePromptBack {
    prompt: BoundedExperiencePrompt,
    staged: bool,
    complete: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for ExperiencePromptBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let Some(context) = bytes.input(PortId(0)) else {
            if io.input_closed(PortId(0)) {
                if io.consume_closed(PortId(0)).is_err() {
                    return fail(FailureCode::InvalidLifecycle, 0);
                }
                self.complete = true;
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        };
        let length = match self.prompt.execute(context) {
            Ok(output) => output.len() as u32,
            Err(refusal) => {
                return fail(
                    FailureCode::InvalidInput,
                    match refusal {
                        ExperiencePromptRefusal::Malformed => 1,
                        ExperiencePromptRefusal::ByteBound => 2,
                        ExperiencePromptRefusal::Evidence(reason) => {
                            100 + super::experience_evidence_back::refusal_detail(reason)
                        }
                    },
                )
            }
        };
        if !io.output_ready_for(PortId(0), length) {
            return StepOutcome::Await;
        }
        if io.consume(PortId(0)).is_err() || io.send_prepared(PortId(0), length).is_err() {
            return fail(FailureCode::WorkBudgetExhausted, 0);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then(|| self.prompt.output())
    }
    fn step_committed(&mut self) {
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.complete = true;
    }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    super::episode_loop_back::exact_pure_gear(
        gear,
        &conduit_std_offers::experience_prompt_offer(),
    )?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: EXPERIENCE_PROMPT_MAX_BYTES as u32,
        maximum_value_bytes: EXPERIENCE_PROMPT_MAX_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::ExperiencePrompt(ExperiencePromptBack {
        prompt: BoundedExperiencePrompt::prepare(),
        staged: false,
        complete: false,
    }))
}
