//! One captured reflection request. Only the kernel admits and schedules it.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_chat::{
    PreparedEpisodeRetrievalProducer, MAXIMUM_EPISODE_RETRIEVAL_CAPTURE_BYTES as MAX,
};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
const EXPERIENCE: PortId = PortId(0);
const MEMORY: PortId = PortId(1);
const CONTEXT: PortId = PortId(2);
const INTENT: PortId = PortId(3);
const POLICY: PortId = PortId(4);
const CLOCK: PortId = PortId(5);
const WINDOW: PortId = PortId(0);
const CAPTURE: PortId = PortId(1);
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EPISODE_RETRIEVAL_PRODUCER_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct EpisodeRetrievalProducerBack {
    producer: PreparedEpisodeRetrievalProducer,
    staged: bool,
    context_closed: bool,
    experience_closed: bool,
    complete: bool,
}
fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl<const PORTS: usize> StepBack<PORTS> for EpisodeRetrievalProducerBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some(context) = bytes.input(CONTEXT) {
            if self.producer.admit_context(context).is_err() {
                return failure(1);
            }
            return if io.consume(CONTEXT).is_ok() {
                StepOutcome::Progress
            } else {
                failure(2)
            };
        }
        if io.input_closed(CONTEXT) && !self.context_closed {
            self.context_closed = true;
            return if io.consume_closed(CONTEXT).is_ok() {
                StepOutcome::Progress
            } else {
                failure(2)
            };
        }
        if let Some(experience) = bytes.input(EXPERIENCE) {
            if !self.producer.has_context() {
                return if self.context_closed {
                    failure(1)
                } else {
                    StepOutcome::Await
                };
            }
            let Some(memory) = bytes.input(MEMORY) else {
                return if io.input_closed(MEMORY) {
                    failure(3)
                } else {
                    StepOutcome::Await
                };
            };
            let (Some(intent), Some(policy), Some(clock)) =
                (bytes.input(INTENT), bytes.input(POLICY), bytes.input(CLOCK))
            else {
                return StepOutcome::Await;
            };
            if !io.output_ready(WINDOW) || !io.output_ready(CAPTURE) {
                return StepOutcome::Await;
            }
            if self
                .producer
                .stage(experience, memory, intent, policy, clock)
                .is_err()
            {
                return failure(4);
            }
            let (window, capture) = (
                self.producer.window().len() as u32,
                self.producer.capture().len() as u32,
            );
            if !io.output_ready_for(WINDOW, window) || !io.output_ready_for(CAPTURE, capture) {
                self.producer.cancel();
                return StepOutcome::Await;
            }
            for input in [EXPERIENCE, MEMORY, INTENT, POLICY, CLOCK] {
                if io.consume(input).is_err() {
                    return failure(2);
                }
            }
            if io.send_prepared(WINDOW, window).is_err()
                || io.send_prepared(CAPTURE, capture).is_err()
            {
                return failure(2);
            }
            self.staged = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(EXPERIENCE) && !self.experience_closed {
            self.experience_closed = true;
            return if io.consume_closed(EXPERIENCE).is_ok() {
                StepOutcome::Progress
            } else {
                failure(2)
            };
        }
        if !self.experience_closed {
            return StepOutcome::Await;
        }
        let Some(memory) = bytes.input(MEMORY) else {
            return if io.input_closed(MEMORY) {
                failure(3)
            } else {
                StepOutcome::Await
            };
        };
        if self.producer.check_memory(memory).is_err() {
            return failure(4);
        }
        if io.consume(MEMORY).is_err() {
            return failure(2);
        }
        // Retire the authored Current inputs only after all begun episodes have
        // completed. No new work is admitted during closure.
        for input in [INTENT, POLICY, CLOCK] {
            if bytes.input(input).is_some() && io.consume(input).is_err() {
                return failure(2);
            }
        }
        self.complete = true;
        StepOutcome::Complete
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if !self.staged {
            return None;
        }
        match port {
            WINDOW => Some(self.producer.window()),
            CAPTURE => Some(self.producer.capture()),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        if core::mem::take(&mut self.staged) {
            self.producer.commit();
        }
    }
    fn cancel(&mut self) {
        self.producer.cancel();
        self.staged = false;
        self.complete = true;
    }
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    super::episode_loop_back::exact_pure_gear(
        gear,
        &conduit_std_offers::episode_retrieval_producer_offer(),
    )?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (MAX + conduit_ai::TEMPORAL_EVIDENCE_FLOW_MAX_BYTES) as u32,
        maximum_value_bytes: MAX as u32,
        host_requests: 0,
        sign_items: 16,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::EpisodeRetrievalProducer(
        EpisodeRetrievalProducerBack {
            producer: PreparedEpisodeRetrievalProducer::prepare(),
            staged: false,
            context_closed: false,
            experience_closed: false,
            complete: false,
        },
    ))
}
