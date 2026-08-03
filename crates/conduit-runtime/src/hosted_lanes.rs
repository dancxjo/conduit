//! Fixed, bounded hosted realization of physical execution lanes.
//!
//! Workers compute proposals only. They never receive authoritative cord,
//! lifecycle, authority, or evidence state; the Conduit scheduler commits the
//! returned proposals in deterministic ticket order.

use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread::{self, JoinHandle};

use conduit_core::{ExecutionArrangement, ExecutionGuarantee, Id, IsolationProfile};

use crate::ResolvedExecutionArrangement;

pub const FIXED_HOSTED_LANE_PROVIDER_ID: &str = "provider/fixed-hosted-lanes";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedLaneReservation {
    pub generation: u64,
    pub lanes: u16,
    pub command_slots_per_lane: u16,
    pub completion_slots: u16,
    pub proposal_slots: u16,
    pub maximum_proposal_bytes: u64,
    pub evidence_slots: u32,
}

impl HostedLaneReservation {
    pub const fn validate(self) -> Result<(), HostedLaneError> {
        if self.generation == 0
            || self.lanes == 0
            || self.command_slots_per_lane != 1
            || self.completion_slots < self.lanes
            || self.proposal_slots < self.lanes
            || self.maximum_proposal_bytes == 0
            || self.evidence_slots < self.lanes as u32 * 2
        {
            return Err(HostedLaneError::InvalidReservation);
        }
        Ok(())
    }
}

/// One bounded region computation. Its output is staged, not authoritative.
pub trait HostedLaneJob: Send + 'static {
    type Proposal: Send + 'static;

    fn compute(self) -> Self::Proposal;

    /// Complete accounted size of one returned proposal, including any
    /// proposal-owned heap storage.
    fn proposal_bytes(proposal: &Self::Proposal) -> u64;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedLaneObservation {
    pub generation: u64,
    pub batch: u64,
    pub lane: u16,
    pub ticket: u64,
    /// Causal provider sequence at worker entry.
    pub entered_sequence: u64,
    /// Sequence after every lane in the batch had entered.
    pub release_sequence: u64,
    /// Causal provider sequence after computation returned or faulted.
    pub finished_sequence: u64,
    pub faulted: bool,
}

#[derive(Debug)]
pub struct HostedProposal<P> {
    pub ticket: u64,
    pub lane: u16,
    pub value: P,
}

#[derive(Debug)]
pub struct HostedProposalBatch<'a, P> {
    proposals: &'a mut Vec<HostedProposal<P>>,
    physical_completion_order: &'a [HostedLaneObservation],
}

impl<P> HostedProposalBatch<'_, P> {
    /// Proposals are ordered only by deterministic caller-assigned ticket.
    #[must_use]
    pub fn proposals(&self) -> &[HostedProposal<P>] {
        self.proposals
    }

    /// Physical observations retain completion order and never choose commit.
    #[must_use]
    pub const fn physical_completion_order(&self) -> &[HostedLaneObservation] {
        self.physical_completion_order
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedCommitBatch<'a> {
    pub committed_tickets: &'a [u64],
    pub physical_completion_order: &'a [HostedLaneObservation],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedLaneError {
    InvalidReservation,
    ArrangementMismatch,
    WrongBatchSize,
    DuplicateTicket,
    BatchSequenceExhausted,
    ProviderLost,
    ProviderStartupFailed,
    WorkerFault { lane: u16, ticket: u64 },
    CommitDomainMismatch,
    CoordinatorTerminal,
    StaleProposal,
    CommitRejected,
    ProposalCapacityExceeded,
}

impl HostedLaneError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::InvalidReservation => "CND-LAN-001",
            Self::ArrangementMismatch => "CND-LAN-007",
            Self::WrongBatchSize => "CND-LAN-002",
            Self::DuplicateTicket => "CND-LAN-003",
            Self::BatchSequenceExhausted => "CND-LAN-004",
            Self::ProviderLost => "CND-LAN-005",
            Self::ProviderStartupFailed => "CND-LAN-008",
            Self::WorkerFault { .. } => "CND-LAN-006",
            Self::CommitDomainMismatch => "CND-LAN-009",
            Self::CoordinatorTerminal => "CND-LAN-010",
            Self::StaleProposal => "CND-LAN-011",
            Self::CommitRejected => "CND-LAN-012",
            Self::ProposalCapacityExceeded => "CND-LAN-013",
        }
    }
}

impl fmt::Display for HostedLaneError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidReservation => "hosted lane reservation is invalid",
            Self::ArrangementMismatch => {
                "exact execution arrangement does not select this hosted provider"
            }
            Self::WrongBatchSize => "hosted proposal batch does not fill the admitted lanes",
            Self::DuplicateTicket => "hosted proposal batch contains duplicate tickets",
            Self::BatchSequenceExhausted => "hosted lane batch sequence is exhausted",
            Self::ProviderLost => "hosted lane provider was lost",
            Self::ProviderStartupFailed => "hosted lane provider could not start its fixed workers",
            Self::WorkerFault { .. } => "hosted lane computation faulted",
            Self::CommitDomainMismatch => {
                "hosted lane coordinator has no matching deterministic commit domain"
            }
            Self::CoordinatorTerminal => "hosted lane coordinator is already terminal",
            Self::StaleProposal => "hosted lane proposal does not match its deterministic ticket",
            Self::CommitRejected => "authoritative deterministic commit rejected a proposal",
            Self::ProposalCapacityExceeded => {
                "hosted lane proposals exceeded their pre-admitted byte reservation"
            }
        })
    }
}

/// Exact fixed-lane compute/commit coordinator. Workers produce only bounded
/// proposals; the caller-owned commit closure remains the sole authoritative
/// mutation boundary and is invoked in plan-derived ticket order.
pub struct FixedHostedExecutionCoordinator<J: HostedLaneJob> {
    provider: FixedHostedLaneProvider<J>,
    plan_epoch: u64,
    commit_domain: String,
    next_ticket: u64,
    maximum_batch_proposals: u16,
    committed_tickets: Vec<u64>,
    terminal: bool,
    disposed_slots: u64,
}

impl<J: HostedLaneJob> FixedHostedExecutionCoordinator<J> {
    pub fn admit(
        arrangement: &ResolvedExecutionArrangement,
        placement_id: &str,
        commit_domain: &str,
        first_ticket: u64,
    ) -> Result<Self, HostedLaneError> {
        if arrangement.identity != arrangement.computed_identity() || arrangement.plan_epoch == 0 {
            return Err(HostedLaneError::ArrangementMismatch);
        }
        let domain = arrangement
            .commit_domains
            .iter()
            .find(|domain| domain.id == commit_domain)
            .ok_or(HostedLaneError::CommitDomainMismatch)?;
        if domain.ordering != conduit_core::CommitOrdering::DeterministicFrontier {
            return Err(HostedLaneError::CommitDomainMismatch);
        }
        let provider = arrangement
            .with_contract(|contract| {
                FixedHostedLaneProvider::admit(contract, Id(placement_id), Id(commit_domain))
            })
            .map_err(|_| HostedLaneError::ArrangementMismatch)??;
        let lanes = provider.reservation().lanes;
        if domain.proposal_slots < lanes || domain.commit_slots < lanes {
            return Err(HostedLaneError::CommitDomainMismatch);
        }
        let maximum_batch_proposals = domain.proposal_slots.min(domain.commit_slots);
        let mut committed_tickets = Vec::new();
        committed_tickets
            .try_reserve_exact(usize::from(maximum_batch_proposals))
            .map_err(|_| HostedLaneError::InvalidReservation)?;
        Ok(Self {
            provider,
            plan_epoch: arrangement.plan_epoch,
            commit_domain: commit_domain.to_owned(),
            next_ticket: first_ticket,
            maximum_batch_proposals,
            committed_tickets,
            terminal: false,
            disposed_slots: 0,
        })
    }

    #[must_use]
    pub const fn plan_epoch(&self) -> u64 {
        self.plan_epoch
    }

    #[must_use]
    pub fn commit_domain(&self) -> &str {
        &self.commit_domain
    }

    #[must_use]
    pub const fn disposed_slots(&self) -> u64 {
        self.disposed_slots
    }

    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        self.terminal
    }

    pub fn compute_and_commit<I>(
        &mut self,
        jobs: I,
        mut commit: impl FnMut(u64, J::Proposal) -> Result<(), ()>,
    ) -> Result<HostedCommitBatch<'_>, HostedLaneError>
    where
        I: IntoIterator<Item = J>,
        I::IntoIter: ExactSizeIterator,
    {
        if self.terminal {
            return Err(HostedLaneError::CoordinatorTerminal);
        }
        let jobs = jobs.into_iter();
        let lane_count = self.provider.reservation().lanes;
        if jobs.len() != usize::from(lane_count) || lane_count > self.maximum_batch_proposals {
            return Err(HostedLaneError::WrongBatchSize);
        }
        let next = self
            .next_ticket
            .checked_add(u64::from(lane_count))
            .ok_or(HostedLaneError::BatchSequenceExhausted)?;
        let first_ticket = self.next_ticket;
        let jobs = jobs.enumerate().map(move |(index, job)| {
            (
                first_ticket + u64::try_from(index).expect("admitted lane index fits u64"),
                job,
            )
        });
        {
            let batch = match self.provider.compute_proposals(jobs) {
                Ok(batch) => batch,
                Err(error) => {
                    self.terminal = true;
                    self.disposed_slots = self.disposed_slots.saturating_add(u64::from(lane_count));
                    return Err(error);
                }
            };
            let mut expected = self.next_ticket;
            let proposal_count = batch.proposals.len();
            self.committed_tickets.clear();
            for (index, proposal) in batch.proposals.drain(..).enumerate() {
                if proposal.ticket != expected {
                    self.terminal = true;
                    self.disposed_slots = self
                        .disposed_slots
                        .saturating_add(u64::try_from(proposal_count - index).unwrap_or(u64::MAX));
                    return Err(HostedLaneError::StaleProposal);
                }
                if commit(proposal.ticket, proposal.value).is_err() {
                    self.terminal = true;
                    self.disposed_slots = self
                        .disposed_slots
                        .saturating_add(u64::try_from(proposal_count - index).unwrap_or(u64::MAX));
                    return Err(HostedLaneError::CommitRejected);
                }
                self.committed_tickets.push(proposal.ticket);
                expected = expected
                    .checked_add(1)
                    .ok_or(HostedLaneError::BatchSequenceExhausted)?;
            }
        }
        self.next_ticket = next;
        let physical_completion_order = self.provider.physical_completion_order();
        Ok(HostedCommitBatch {
            committed_tickets: &self.committed_tickets,
            physical_completion_order,
        })
    }

    /// Fence this coordinator between batches. A new epoch requires a newly
    /// admitted arrangement and worker population.
    pub fn cancel(&mut self) -> Result<(), HostedLaneError> {
        if self.terminal {
            return Err(HostedLaneError::CoordinatorTerminal);
        }
        self.terminal = true;
        Ok(())
    }
}

struct GateState {
    batch: u64,
    expected: u16,
    entered: u16,
    released: bool,
    release_sequence: u64,
}

struct BatchGate {
    state: Mutex<GateState>,
    changed: Condvar,
}

impl BatchGate {
    fn new() -> Self {
        Self {
            state: Mutex::new(GateState {
                batch: 0,
                expected: 0,
                entered: 0,
                released: false,
                release_sequence: 0,
            }),
            changed: Condvar::new(),
        }
    }

    fn begin(&self, batch: u64, expected: u16) -> Result<(), HostedLaneError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| HostedLaneError::ProviderLost)?;
        state.batch = batch;
        state.expected = expected;
        state.entered = 0;
        state.released = false;
        state.release_sequence = 0;
        Ok(())
    }

    fn enter_and_wait(
        &self,
        batch: u64,
        sequence: &AtomicU64,
    ) -> Result<(u64, u64), HostedLaneError> {
        let entered_sequence = next_sequence(sequence)?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| HostedLaneError::ProviderLost)?;
        if state.batch != batch || state.released || state.entered >= state.expected {
            return Err(HostedLaneError::ProviderLost);
        }
        state.entered += 1;
        self.changed.notify_all();
        while !state.released {
            state = self
                .changed
                .wait(state)
                .map_err(|_| HostedLaneError::ProviderLost)?;
            if state.batch != batch {
                return Err(HostedLaneError::ProviderLost);
            }
        }
        Ok((entered_sequence, state.release_sequence))
    }

    fn release_when_all_entered(
        &self,
        batch: u64,
        sequence: &AtomicU64,
    ) -> Result<(), HostedLaneError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| HostedLaneError::ProviderLost)?;
        while state.batch == batch && state.entered < state.expected {
            state = self
                .changed
                .wait(state)
                .map_err(|_| HostedLaneError::ProviderLost)?;
        }
        if state.batch != batch || state.released {
            return Err(HostedLaneError::ProviderLost);
        }
        state.release_sequence = next_sequence(sequence)?;
        state.released = true;
        self.changed.notify_all();
        Ok(())
    }

    fn abort(&self, batch: u64) {
        if let Ok(mut state) = self.state.lock()
            && state.batch == batch
        {
            state.batch = 0;
            state.released = true;
            self.changed.notify_all();
        }
    }
}

fn next_sequence(sequence: &AtomicU64) -> Result<u64, HostedLaneError> {
    sequence
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
            value.checked_add(1)
        })
        .map(|prior| prior + 1)
        .map_err(|_| HostedLaneError::BatchSequenceExhausted)
}

struct LaneCommand<J> {
    batch: u64,
    ticket: u64,
    job: J,
}

struct LaneCompletion<P> {
    observation: HostedLaneObservation,
    proposal: Option<P>,
}

/// A fixed population of independently progressing hosted workers.
pub struct FixedHostedLaneProvider<J: HostedLaneJob> {
    reservation: HostedLaneReservation,
    commands: Vec<mpsc::SyncSender<LaneCommand<J>>>,
    completions: mpsc::Receiver<LaneCompletion<J::Proposal>>,
    workers: Vec<JoinHandle<()>>,
    gate: Arc<BatchGate>,
    sequence: Arc<AtomicU64>,
    next_batch: u64,
    pending_jobs: Vec<Option<(u64, J)>>,
    proposals: Vec<HostedProposal<J::Proposal>>,
    physical_completion_order: Vec<HostedLaneObservation>,
}

impl<J: HostedLaneJob> FixedHostedLaneProvider<J> {
    /// Admit this provider directly from one already validated portable
    /// arrangement. The provider interprets only its own placement and lane
    /// facts; region selection and deterministic commit stay in the runtime.
    pub fn admit(
        arrangement: ExecutionArrangement<'_>,
        placement_id: Id<'_>,
        commit_domain_id: Id<'_>,
    ) -> Result<Self, HostedLaneError> {
        let placement = arrangement
            .placements
            .iter()
            .find(|placement| placement.id == placement_id)
            .ok_or(HostedLaneError::ArrangementMismatch)?;
        if placement.provider.id.as_str() != FIXED_HOSTED_LANE_PROVIDER_ID
            || placement.isolation != IsolationProfile::StepNative
        {
            return Err(HostedLaneError::ArrangementMismatch);
        }
        let domain = arrangement
            .commit_domains
            .iter()
            .find(|domain| domain.id == commit_domain_id)
            .ok_or(HostedLaneError::CommitDomainMismatch)?;
        let lanes = arrangement
            .lanes
            .iter()
            .filter(|lane| {
                lane.placement == placement.id
                    && arrangement.regions.iter().any(|region| {
                        region.placement == placement.id
                            && region.lane == lane.id
                            && region.commit_domain == domain.id
                    })
            })
            .collect::<Vec<_>>();
        let lane_count =
            u16::try_from(lanes.len()).map_err(|_| HostedLaneError::InvalidReservation)?;
        if lane_count == 0
            || lanes.iter().any(|lane| {
                lane.placement_generation != placement.generation
                    || lane.independent_progress != ExecutionGuarantee::Guaranteed
                    || lane.simultaneous_execution != ExecutionGuarantee::Guaranteed
                    || !arrangement.regions.iter().any(|region| {
                        region.placement == placement.id
                            && region.placement_generation == placement.generation
                            && region.lane == lane.id
                            && region.lane_generation == lane.generation
                            && region.commit_domain == domain.id
                            && region.independent
                    })
            })
        {
            return Err(HostedLaneError::ArrangementMismatch);
        }
        let proposal_slots = lanes
            .iter()
            .try_fold(0_u16, |total, lane| total.checked_add(lane.proposal_slots));
        let completion_slots = lanes
            .iter()
            .try_fold(0_u16, |total, lane| total.checked_add(lane.commit_slots));
        let evidence_slots = lanes
            .iter()
            .try_fold(0_u32, |total, lane| total.checked_add(lane.evidence_slots));
        Self::start(HostedLaneReservation {
            generation: placement.generation,
            lanes: lane_count,
            command_slots_per_lane: 1,
            completion_slots: completion_slots.ok_or(HostedLaneError::InvalidReservation)?,
            proposal_slots: proposal_slots.ok_or(HostedLaneError::InvalidReservation)?,
            maximum_proposal_bytes: domain.maximum_proposal_bytes,
            evidence_slots: evidence_slots.ok_or(HostedLaneError::InvalidReservation)?,
        })
    }

    pub fn start(reservation: HostedLaneReservation) -> Result<Self, HostedLaneError> {
        reservation.validate()?;
        let lane_count = usize::from(reservation.lanes);
        let gate = Arc::new(BatchGate::new());
        let sequence = Arc::new(AtomicU64::new(0));
        let (completion_tx, completions) =
            mpsc::sync_channel(usize::from(reservation.completion_slots));
        let mut commands = Vec::new();
        let mut workers = Vec::new();
        let mut pending_jobs = Vec::new();
        let mut proposals = Vec::new();
        let mut physical_completion_order = Vec::new();
        commands
            .try_reserve_exact(lane_count)
            .map_err(|_| HostedLaneError::InvalidReservation)?;
        workers
            .try_reserve_exact(lane_count)
            .map_err(|_| HostedLaneError::InvalidReservation)?;
        pending_jobs
            .try_reserve_exact(lane_count)
            .map_err(|_| HostedLaneError::InvalidReservation)?;
        proposals
            .try_reserve_exact(lane_count)
            .map_err(|_| HostedLaneError::InvalidReservation)?;
        physical_completion_order
            .try_reserve_exact(lane_count)
            .map_err(|_| HostedLaneError::InvalidReservation)?;
        pending_jobs.resize_with(lane_count, || None);
        for lane in 0..reservation.lanes {
            let (command_tx, command_rx): (
                mpsc::SyncSender<LaneCommand<J>>,
                mpsc::Receiver<LaneCommand<J>>,
            ) = mpsc::sync_channel(usize::from(reservation.command_slots_per_lane));
            commands.push(command_tx);
            let completion_tx = completion_tx.clone();
            let worker_gate = Arc::clone(&gate);
            let worker_sequence = Arc::clone(&sequence);
            let generation = reservation.generation;
            let worker = thread::Builder::new()
                .name(format!("conduit-lane-{lane}"))
                .spawn(move || {
                    while let Ok(command) = command_rx.recv() {
                        let LaneCommand { batch, ticket, job } = command;
                        let entered = worker_gate.enter_and_wait(batch, &worker_sequence);
                        let Ok((entered_sequence, release_sequence)) = entered else {
                            break;
                        };
                        let outcome = catch_unwind(AssertUnwindSafe(|| job.compute()));
                        let Ok(finished_sequence) = next_sequence(&worker_sequence) else {
                            break;
                        };
                        let (proposal, faulted) = match outcome {
                            Ok(proposal) => (Some(proposal), false),
                            Err(_) => (None, true),
                        };
                        if completion_tx
                            .send(LaneCompletion {
                                observation: HostedLaneObservation {
                                    generation,
                                    batch,
                                    lane,
                                    ticket,
                                    entered_sequence,
                                    release_sequence,
                                    finished_sequence,
                                    faulted,
                                },
                                proposal,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
            match worker {
                Ok(worker) => workers.push(worker),
                Err(_) => {
                    commands.clear();
                    for worker in workers.drain(..) {
                        let _ = worker.join();
                    }
                    return Err(HostedLaneError::ProviderStartupFailed);
                }
            }
        }
        drop(completion_tx);
        Ok(Self {
            reservation,
            commands,
            completions,
            workers,
            gate,
            sequence,
            next_batch: 1,
            pending_jobs,
            proposals,
            physical_completion_order,
        })
    }

    #[must_use]
    pub const fn reservation(&self) -> HostedLaneReservation {
        self.reservation
    }

    /// Dispatch exactly one bounded region computation to every admitted lane.
    /// The causal gate proves every lane entered before any proposal could
    /// finish. Returned proposals are sorted by deterministic ticket; physical
    /// completion observations retain their actual provider order. Job,
    /// proposal, completion-order, and sort storage was reserved at Start and
    /// is reused by every batch.
    pub fn compute_proposals<I>(
        &mut self,
        jobs: I,
    ) -> Result<HostedProposalBatch<'_, J::Proposal>, HostedLaneError>
    where
        I: IntoIterator<Item = (u64, J)>,
        I::IntoIter: ExactSizeIterator,
    {
        let jobs = jobs.into_iter();
        if jobs.len() != usize::from(self.reservation.lanes) {
            return Err(HostedLaneError::WrongBatchSize);
        }
        self.proposals.clear();
        self.physical_completion_order.clear();
        for (index, job) in jobs.enumerate() {
            if self.pending_jobs[..index]
                .iter()
                .flatten()
                .any(|(prior, _)| prior == &job.0)
            {
                self.pending_jobs[..index].fill_with(|| None);
                return Err(HostedLaneError::DuplicateTicket);
            }
            self.pending_jobs[index] = Some(job);
        }
        let batch = self.next_batch;
        self.next_batch = batch
            .checked_add(1)
            .ok_or(HostedLaneError::BatchSequenceExhausted)?;
        self.gate.begin(batch, self.reservation.lanes)?;
        for lane in 0..self.pending_jobs.len() {
            let (ticket, job) = self.pending_jobs[lane]
                .take()
                .expect("validated batch fills every preallocated job slot");
            if self.commands[lane]
                .send(LaneCommand { batch, ticket, job })
                .is_err()
            {
                self.pending_jobs[lane + 1..].fill_with(|| None);
                self.gate.abort(batch);
                return Err(HostedLaneError::ProviderLost);
            }
        }
        if let Err(error) = self.gate.release_when_all_entered(batch, &self.sequence) {
            self.gate.abort(batch);
            return Err(error);
        }

        let lane_count = usize::from(self.reservation.lanes);
        let mut first_fault = None;
        let mut proposal_bytes = 0_u64;
        for _ in 0..lane_count {
            let completion = self
                .completions
                .recv()
                .map_err(|_| HostedLaneError::ProviderLost)?;
            let observation = completion.observation;
            self.physical_completion_order.push(observation);
            let Some(value) = completion.proposal else {
                first_fault.get_or_insert(HostedLaneError::WorkerFault {
                    lane: observation.lane,
                    ticket: observation.ticket,
                });
                continue;
            };
            let Some(next_proposal_bytes) = proposal_bytes.checked_add(J::proposal_bytes(&value))
            else {
                first_fault.get_or_insert(HostedLaneError::ProposalCapacityExceeded);
                continue;
            };
            proposal_bytes = next_proposal_bytes;
            if proposal_bytes > self.reservation.maximum_proposal_bytes {
                first_fault.get_or_insert(HostedLaneError::ProposalCapacityExceeded);
                continue;
            }
            self.proposals.push(HostedProposal {
                ticket: observation.ticket,
                lane: observation.lane,
                value,
            });
        }
        if let Some(fault) = first_fault {
            return Err(fault);
        }
        self.proposals.sort_by_key(|proposal| proposal.ticket);
        Ok(HostedProposalBatch {
            proposals: &mut self.proposals,
            physical_completion_order: &self.physical_completion_order,
        })
    }

    fn physical_completion_order(&self) -> &[HostedLaneObservation] {
        &self.physical_completion_order
    }
}

impl<J: HostedLaneJob> Drop for FixedHostedLaneProvider<J> {
    fn drop(&mut self) {
        self.commands.clear();
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}
