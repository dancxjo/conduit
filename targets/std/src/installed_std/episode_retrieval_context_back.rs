//! Resolve one captured retrieval result, select context, and pair request/pending.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_chat::{
    EpisodeCurrentProjectionRefusal, PreparedEpisodeRetrievalContext,
    PreparedEpisodeRetrievalPacket, PreparedEpisodeRetrievalRequest,
};
use conduit_core::{ConfigurationValue, PlannedGear};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_plot::rust_binding::NativeRustBinding;
mod projector;
type Projector =
    Box<dyn FnMut(&[u8], &mut [u8]) -> Result<usize, EpisodeCurrentProjectionRefusal> + Send>;
const CAPTURE: PortId = PortId(0);
const OUTCOME: PortId = PortId(1);
const REQUEST: PortId = PortId(0);
const PENDING: PortId = PortId(1);
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EPISODE_RETRIEVAL_CONTEXT_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct EpisodeRetrievalContextBack {
    packet: Box<PreparedEpisodeRetrievalPacket>,
    context: Box<PreparedEpisodeRetrievalContext>,
    request: Box<PreparedEpisodeRetrievalRequest<Projector>>,
    staged: bool,
    closed: [bool; 2],
    complete: bool,
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
impl EpisodeRetrievalContextBack {
    fn stage(&mut self, capture: &[u8], outcome: &[u8]) -> Result<(), u16> {
        let (snapshot, _) = self.packet.decode(capture).map_err(|_| 10u16)?;
        self.context.select(&snapshot, outcome).map_err(|_| 20u16)?;
        self.request
            .compose(&snapshot, &self.context)
            .map_err(request_refusal)?;
        Ok(())
    }
}
fn request_refusal(reason: conduit_chat::EpisodeRetrievalRequestRefusal) -> u16 {
    use conduit_chat::EpisodeRetrievalRequestRefusal as R;
    match reason {
        R::InstructionBound => 31,
        R::SnapshotMismatch => 32,
        R::CurrentProjection(_) => 33,
        R::InvalidCurrentJson => 34,
        R::InvalidFragmentJson => 35,
        R::MalformedExperience => 36,
        R::ByteBound => 37,
        R::PendingBound => 38,
        R::MalformedPending => 39,
    }
}
impl<const PORTS: usize> StepBack<PORTS> for EpisodeRetrievalContextBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if let (Some(capture), Some(outcome)) = (bytes.input(CAPTURE), bytes.input(OUTCOME)) {
            if !io.output_ready(REQUEST) || !io.output_ready(PENDING) {
                return StepOutcome::Await;
            }
            if let Err(detail) = self.stage(capture, outcome) {
                return fail(detail);
            }
            let (request, pending) = (
                self.request.output().len() as u32,
                self.request.pending().len() as u32,
            );
            if !io.output_ready_for(REQUEST, request) || !io.output_ready_for(PENDING, pending) {
                return StepOutcome::Await;
            }
            if io.consume(CAPTURE).is_err()
                || io.consume(OUTCOME).is_err()
                || io.send_prepared(REQUEST, request).is_err()
                || io.send_prepared(PENDING, pending).is_err()
            {
                return fail(1);
            }
            self.staged = true;
            return StepOutcome::Progress;
        }
        for (index, port) in [CAPTURE, OUTCOME].into_iter().enumerate() {
            if bytes.input(port).is_none() && io.input_closed(port) && !self.closed[index] {
                if io.consume_closed(port).is_err() {
                    return fail(1);
                }
                self.closed[index] = true;
                return StepOutcome::Progress;
            }
        }
        if (self.closed[0] && bytes.input(OUTCOME).is_some())
            || (self.closed[1] && bytes.input(CAPTURE).is_some())
        {
            return fail(2);
        }
        if self.closed.iter().all(|closed| *closed) {
            self.complete = true;
            StepOutcome::Complete
        } else {
            StepOutcome::Await
        }
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if !self.staged {
            return None;
        }
        match port {
            REQUEST => Some(self.request.output()),
            PENDING => Some(self.request.pending()),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        self.staged = false;
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.complete = true;
    }
}
fn configuration(gear: &PlannedGear) -> Result<(conduit_ai::ContextSelectionPolicy, &str), String> {
    if gear.configuration.len() != 3 {
        return Err("retrieval context requires policy, instruction, and relevance_profile".into());
    }
    if !gear.configuration.iter().any(|entry| {
        entry.key == "relevance_profile"
            && entry.value
                == ConfigurationValue::Text(conduit_chat::EPISODE_SHARED_TERM_PROFILE.into())
    }) {
        return Err("unsupported episode relevance profile".into());
    }
    let policy = gear
        .configuration
        .iter()
        .find(|entry| entry.key == "policy")
        .ok_or("retrieval policy missing")?;
    let ConfigurationValue::Structured(policy) = &policy.value else {
        return Err("retrieval policy must be native".into());
    };
    if policy.canonical_value().len() > 4096 {
        return Err("retrieval policy exceeds capture bound".into());
    }
    let policy = conduit_ai::ContextSelectionPolicy::decode(policy.canonical_value())
        .map_err(|_| "retrieval policy differs from its native Type")?;
    if policy.token_accounting_profile() != conduit_chat::EPISODE_CONTEXT_BYTE_CHARGE_PROFILE {
        return Err("retrieval byte-charge profile unsupported".into());
    }
    let instruction = gear
        .configuration
        .iter()
        .find(|entry| entry.key == "instruction")
        .ok_or("retrieval instruction missing")?;
    let ConfigurationValue::Text(instruction) = &instruction.value else {
        return Err("retrieval instruction must be Text".into());
    };
    if instruction.is_empty() || instruction.len() > 1024 {
        return Err("retrieval instruction bound exceeded".into());
    }
    Ok((policy, instruction))
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    // Configuration is checked separately; every realization/port/effect fact
    // still matches the exact pure offer before preparation allocates anything.
    let mut identity = gear.clone();
    identity.configuration.clear();
    super::episode_loop_back::exact_pure_gear(
        &identity,
        &conduit_std_offers::episode_retrieval_context_offer(),
    )?;
    configuration(gear)?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: (conduit_chat::MAXIMUM_EPISODE_RETRIEVAL_REQUEST_BYTES
            + conduit_chat::MAXIMUM_EPISODE_RECORD_BYTES) as u32,
        maximum_value_bytes: conduit_chat::MAXIMUM_EPISODE_RECORD_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    let (policy, instruction) = configuration(gear)?;
    Ok(InstalledBack::EpisodeRetrievalContext(
        EpisodeRetrievalContextBack {
            packet: Box::new(PreparedEpisodeRetrievalPacket::prepare()),
            context: Box::new(
                PreparedEpisodeRetrievalContext::prepare_with_shared_terms(policy)
                    .map_err(|error| format!("prepare context selection: {error:?}"))?,
            ),
            request: Box::new(
                PreparedEpisodeRetrievalRequest::prepare_fact_view(
                    instruction,
                    projector::prepare(),
                )
                .map_err(|error| format!("prepare bounded retrieval request: {error:?}"))?,
            ),
            staged: false,
            closed: [false; 2],
            complete: false,
        },
    ))
}
