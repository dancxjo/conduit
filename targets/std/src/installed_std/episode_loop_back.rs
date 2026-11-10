//! Ordinary pure std Backs for one Body's bounded episode loop.
//!
//! The composer begins an episode only when it holds both an observation and
//! the memory cell's Current reflecting every earlier episode, so at most one
//! episode is in flight and later observations stay queued at their Fore.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_chat::{
    EpisodeComposer, EpisodeRefusal, EpisodeRetainer, MAXIMUM_EPISODE_PROMPT_BYTES,
    MAXIMUM_EPISODE_RECORD_BYTES,
};
use conduit_core::{CapabilityOffer, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_semantic_catalog::{
    BoundedExperienceEvidenceProjection, EXPERIENCE_MODEL_REQUEST_MAX_BYTES,
};

pub(super) static COMPOSE_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EPISODE_COMPOSE_IMPLEMENTATION,
    budget: compose_budget,
    prepare: prepare_compose,
};
pub(super) static EXPERIENCE_COMPOSE_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EPISODE_EXPERIENCE_COMPOSE_IMPLEMENTATION,
    budget: experience_compose_budget,
    prepare: prepare_experience_compose,
};
pub(super) static RETAIN_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EPISODE_RETAIN_IMPLEMENTATION,
    budget: retain_budget,
    prepare: prepare_retain,
};

const OBSERVATION: PortId = PortId(0);
const MEMORY: PortId = PortId(1);
const CONTEXT: PortId = PortId(2);
const REQUEST: PortId = PortId(0);
const PENDING_OUT: PortId = PortId(1);
const PENDING_IN: PortId = PortId(0);
const RESPONSE: PortId = PortId(1);
const EPISODE: PortId = PortId(0);
/// The memory cell closed before publishing the Current an episode needed.
pub(super) const MEMORY_CLOSED: u16 = 20;
/// The model closed its result Flow while an episode still awaited a response.
pub(super) const RESPONSE_CLOSED_WHILE_PENDING: u16 = 21;
/// A model result arrived with no pending episode to pair it with.
pub(super) const RESPONSE_WITHOUT_EPISODE: u16 = 22;

pub(super) struct EpisodeComposeBack {
    composer: EpisodeComposer,
    experience: Option<ExperiencePreparation>,
    staged: bool,
    observations_closed: bool,
    context_closed: bool,
    complete: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for EpisodeComposeBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let Some(context) = bytes.input(CONTEXT) {
            if let Err(refusal) = self.composer.admit_context(context) {
                return refused(refusal);
            }
            return consumed(io.consume(CONTEXT).is_ok());
        }
        if io.input_closed(CONTEXT) && !self.context_closed {
            self.context_closed = true;
            return consumed(io.consume_closed(CONTEXT).is_ok());
        }
        if let Some(observation) = bytes.input(OBSERVATION) {
            if !self.composer.has_context() {
                return if self.context_closed {
                    refused(EpisodeRefusal::ContextMissing)
                } else {
                    StepOutcome::Await
                };
            }
            let Some(memory) = bytes.input(MEMORY) else {
                return if io.input_closed(MEMORY) {
                    fail(FailureCode::InvalidLifecycle, MEMORY_CLOSED)
                } else {
                    // Backpressure: the previous episode is not yet retained.
                    StepOutcome::Await
                };
            };
            if !io.output_ready(REQUEST) || !io.output_ready(PENDING_OUT) {
                return StepOutcome::Await;
            }
            let composed = match &mut self.experience {
                Some(preparation) => preparation.compose(&mut self.composer, observation, memory),
                None => self.composer.compose(observation, memory).map_err(refused),
            };
            if let Err(outcome) = composed {
                return outcome;
            }
            let request = self.composer.request().len() as u32;
            let pending = self.composer.pending().len() as u32;
            if !io.output_ready_for(REQUEST, request) || !io.output_ready_for(PENDING_OUT, pending)
            {
                self.composer.cancel();
                return StepOutcome::Await;
            }
            if io.consume(OBSERVATION).is_err()
                || io.consume(MEMORY).is_err()
                || io.send_prepared(REQUEST, request).is_err()
                || io.send_prepared(PENDING_OUT, pending).is_err()
            {
                return fail(FailureCode::WorkBudgetExhausted, 0);
            }
            self.staged = true;
            return StepOutcome::Progress;
        }
        if io.input_closed(OBSERVATION) && !self.observations_closed {
            self.observations_closed = true;
            return consumed(io.consume_closed(OBSERVATION).is_ok());
        }
        if !self.observations_closed {
            return StepOutcome::Await;
        }
        // Finish only after the memory cell reflects every begun episode.
        if let Some(memory) = bytes.input(MEMORY) {
            if let Err(refusal) = self.composer.check_memory(memory) {
                return refused(refusal);
            }
            if io.consume(MEMORY).is_err() {
                return fail(FailureCode::InvalidLifecycle, 0);
            }
            self.complete = true;
            return StepOutcome::Complete;
        }
        if io.input_closed(MEMORY) {
            return fail(FailureCode::InvalidLifecycle, MEMORY_CLOSED);
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        match (self.staged, port) {
            (true, REQUEST) => Some(self.composer.request()),
            (true, PENDING_OUT) => Some(self.composer.pending()),
            _ => None,
        }
    }

    fn step_committed(&mut self) {
        if core::mem::take(&mut self.staged) {
            self.composer.commit();
        }
    }

    fn cancel(&mut self) {
        self.staged = false;
        self.composer.cancel();
        self.complete = true;
    }
}

struct ExperiencePreparation {
    projection: BoundedExperienceEvidenceProjection,
    prompt: Box<[u8; EXPERIENCE_MODEL_REQUEST_MAX_BYTES]>,
}

impl ExperiencePreparation {
    fn compose(
        &mut self,
        composer: &mut EpisodeComposer,
        bytes: &[u8],
        memory: &[u8],
    ) -> Result<(), StepOutcome> {
        let malformed = || refused(EpisodeRefusal::ObservationMalformed);
        if bytes.len() > conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES {
            return Err(malformed());
        }
        let input = conduit_chat::EpisodeExperience::decode(bytes).map_err(|_| malformed())?;
        let evidence = input.evidence().clone().encode().map_err(|_| malformed())?;
        let context = self
            .projection
            .execute(&evidence)
            .map_err(experience_refused)?;
        let length =
            super::experience_model_request::render_context(context, self.prompt.as_mut(), None)
                .map_err(experience_refused)?;
        composer
            .compose_episode(
                input.observation().clone(),
                memory,
                Some(&self.prompt[..length]),
            )
            .map_err(refused)
    }
}

fn experience_refused(refusal: conduit_human::ExperienceEvidenceRefusal) -> StepOutcome {
    fail(
        FailureCode::InvalidInput,
        100 + super::experience_evidence_back::refusal_detail(refusal),
    )
}

pub(super) struct EpisodeRetainBack {
    retainer: EpisodeRetainer,
    staged: bool,
    complete: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for EpisodeRetainBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        match (bytes.input(PENDING_IN), bytes.input(RESPONSE)) {
            (Some(pending), Some(response)) => {
                if !io.output_ready(EPISODE) {
                    return StepOutcome::Await;
                }
                let length = match self.retainer.retain(pending, response) {
                    Ok(record) => record.len() as u32,
                    Err(refusal) => return refused(refusal),
                };
                if !io.output_ready_for(EPISODE, length) {
                    return StepOutcome::Await;
                }
                if io.consume(PENDING_IN).is_err()
                    || io.consume(RESPONSE).is_err()
                    || io.send_prepared(EPISODE, length).is_err()
                {
                    return fail(FailureCode::WorkBudgetExhausted, 0);
                }
                self.staged = true;
                StepOutcome::Progress
            }
            (Some(_), None) if io.input_closed(RESPONSE) => {
                fail(FailureCode::InvalidLifecycle, RESPONSE_CLOSED_WHILE_PENDING)
            }
            (None, Some(_)) if io.input_closed(PENDING_IN) => {
                fail(FailureCode::InvalidLifecycle, RESPONSE_WITHOUT_EPISODE)
            }
            (None, None) if io.input_closed(PENDING_IN) && io.input_closed(RESPONSE) => {
                if io.consume_closed(PENDING_IN).is_err() || io.consume_closed(RESPONSE).is_err() {
                    return fail(FailureCode::InvalidLifecycle, 0);
                }
                self.complete = true;
                StepOutcome::Complete
            }
            _ => StepOutcome::Await,
        }
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == EPISODE).then(|| self.retainer.record())
    }

    fn step_committed(&mut self) {
        self.staged = false;
    }

    fn cancel(&mut self) {
        self.staged = false;
        self.complete = true;
    }
}

pub(super) fn consumed(ok: bool) -> StepOutcome {
    if ok {
        StepOutcome::Progress
    } else {
        fail(FailureCode::InvalidLifecycle, 0)
    }
}

pub(super) const fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

pub(super) fn refused(refusal: EpisodeRefusal) -> StepOutcome {
    fail(FailureCode::InvalidInput, refusal_detail(refusal))
}

// Stable diagnostic identities on the ordinary kernel failure Sign.
pub(super) fn refusal_detail(refusal: EpisodeRefusal) -> u16 {
    use EpisodeRefusal::*;
    match refusal {
        ContextMissing => 1,
        ContextMalformed => 2,
        ContextChangedIdentity => 3,
        ObservationMalformed => 4,
        MemoryMalformed => 5,
        MemoryOutOfOrder => 6,
        PendingMalformed => 7,
        ResponseMalformed => 8,
        ResponseNotProduced => 9,
        ResponseBoundExceeded => 10,
        PromptBoundExceeded => 11,
        Encoding => 12,
    }
}

/// Every episode Back is realized only from its exact pure installed offer.
pub(super) fn exact_pure_gear(
    gear: &PlannedGear,
    expected: &CapabilityOffer,
) -> Result<(), String> {
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.artifact_id != expected.implementation.artifact_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.limits != expected.limits
        || gear.semantic_contract != expected.semantic_contract
        || !gear.configuration.is_empty()
        || !gear.host_calls.is_empty()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
        || gear.base.is_some()
        || !gear.realization_properties.is_empty()
        || !gear.realization_characteristics.is_empty()
        || !gear.pool_references.is_empty()
        || !gear.terminal_transductions.is_empty()
    {
        return Err(format!(
            "{} differs from its exact installed offer",
            expected.kind_id.as_str()
        ));
    }
    Ok(())
}

fn compose_budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    exact_pure_gear(gear, &conduit_std_offers::episode_compose_offer())?;
    Ok(compose_storage(0, MAXIMUM_EPISODE_RECORD_BYTES))
}

fn experience_compose_budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    exact_pure_gear(
        gear,
        &conduit_std_offers::episode_experience_compose_offer(),
    )?;
    let maximum = conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES;
    Ok(compose_storage(maximum, maximum))
}

fn compose_storage(input_bytes: usize, maximum_value_bytes: usize) -> BackBudget {
    BackBudget {
        value_items: 2,
        value_bytes: (input_bytes + MAXIMUM_EPISODE_PROMPT_BYTES + MAXIMUM_EPISODE_RECORD_BYTES)
            as u32,
        maximum_value_bytes: maximum_value_bytes as u32,
        host_requests: 0,
        sign_items: 16,
    }
}

fn retain_budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    exact_pure_gear(gear, &conduit_std_offers::episode_retain_offer())?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: MAXIMUM_EPISODE_RECORD_BYTES as u32,
        maximum_value_bytes: MAXIMUM_EPISODE_RECORD_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}

fn prepare_compose(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    compose_budget(gear)?;
    Ok(InstalledBack::EpisodeCompose(new_composer(None)))
}

fn prepare_experience_compose(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    experience_compose_budget(gear)?;
    Ok(InstalledBack::EpisodeCompose(new_composer(Some(
        ExperiencePreparation {
            projection: BoundedExperienceEvidenceProjection::prepare(),
            prompt: Box::new([0; EXPERIENCE_MODEL_REQUEST_MAX_BYTES]),
        },
    ))))
}

fn new_composer(experience: Option<ExperiencePreparation>) -> EpisodeComposeBack {
    EpisodeComposeBack {
        composer: EpisodeComposer::new(),
        experience,
        staged: false,
        observations_closed: false,
        context_closed: false,
        complete: false,
    }
}

fn prepare_retain(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    retain_budget(gear)?;
    Ok(InstalledBack::EpisodeRetain(EpisodeRetainBack {
        retainer: EpisodeRetainer::new(),
        staged: false,
        complete: false,
    }))
}
