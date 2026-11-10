//! The explicit initialized memory cell of the bounded episode loop.
//!
//! It publishes the declared empty memory first, then exactly one Current per
//! completed episode record. Its `update` input is the planner's declared cut
//! for the loop's feedback; it never looks memory up ambiently.
use super::back::{BackBudget, BackFactory, InstalledBack};
use super::episode_loop_back::{exact_pure_gear, fail, refused};
use conduit_chat::{EpisodeMemoryCell, MAXIMUM_EPISODE_MEMORY_BYTES};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    FailureCode, PortId,
};

pub(super) static MEMORY_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EPISODE_MEMORY_IMPLEMENTATION,
    budget,
    prepare,
};

const UPDATE: PortId = PortId(0);
const CURRENT: PortId = PortId(0);

pub(super) struct EpisodeMemoryBack {
    cell: EpisodeMemoryCell,
    initial_sent: bool,
    staged: bool,
    complete: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for EpisodeMemoryBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if !self.initial_sent {
            let length = self.cell.current().len() as u32;
            if !io.output_ready_for(CURRENT, length) {
                return StepOutcome::Await;
            }
            if io.send_prepared(CURRENT, length).is_err() {
                return fail(FailureCode::WorkBudgetExhausted, 0);
            }
            self.staged = true;
            return StepOutcome::Progress;
        }
        if let Some(record) = bytes.input(UPDATE) {
            if !io.output_ready(CURRENT) {
                return StepOutcome::Await;
            }
            let length = match self.cell.apply(record) {
                Ok(current) => current.len() as u32,
                Err(refusal) => return refused(refusal),
            };
            if !io.output_ready_for(CURRENT, length) {
                self.cell.cancel();
                return StepOutcome::Await;
            }
            if io.consume(UPDATE).is_err() || io.send_prepared(CURRENT, length).is_err() {
                return fail(FailureCode::WorkBudgetExhausted, 0);
            }
            self.staged = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(UPDATE) {
            if io.consume_closed(UPDATE).is_err() {
                return fail(FailureCode::InvalidLifecycle, 0);
            }
            self.complete = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == CURRENT).then(|| self.cell.current())
    }

    fn step_committed(&mut self) {
        if core::mem::take(&mut self.staged) {
            self.initial_sent = true;
            self.cell.commit();
        }
    }

    fn cancel(&mut self) {
        self.staged = false;
        self.cell.cancel();
        self.complete = true;
    }
}

fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    exact_pure_gear(gear, &conduit_std_offers::episode_memory_offer())?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: MAXIMUM_EPISODE_MEMORY_BYTES as u32,
        maximum_value_bytes: MAXIMUM_EPISODE_MEMORY_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}

fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    let cell = EpisodeMemoryCell::new().map_err(|_| "empty episode memory is unencodable")?;
    Ok(InstalledBack::EpisodeMemory(EpisodeMemoryBack {
        cell,
        initial_sent: false,
        staged: false,
        complete: false,
    }))
}
