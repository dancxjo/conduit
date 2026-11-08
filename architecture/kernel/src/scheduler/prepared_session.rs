//! Retain one prepared kernel across explicitly admitted finite invocations.
//!
//! This is a lifecycle owner around the existing scheduler, not another kernel.
//! It owns every mutable entry so a failed invocation cannot later masquerade as
//! a successful drain. Host preparation still owns exact Source/Plan/resource
//! eligibility, fresh active-Play identity and external output acceptance.
use super::{CordState, FixedScheduler, ReadySet, SchedulerError, SchedulerStatus, StepBack};
use crate::{
    CordEndpoint, HostCallOutcome, NodeId, RequestId, SignQuery, SignSink, ValueRef, ValueStorage,
};
use alloc::boxed::Box;
mod owned_boundaries;
pub use owned_boundaries::OwnedValueBoundary;

/// Opt-in preparation contract for a Back with retained immutable bindings.
///
/// Preflight must be read-only and validate its next inputs against the exact
/// retained store. Commit must be infallible, bounded and preserve code,
/// resources, request generations and immutable bindings. Cancellation is not a
/// rearm operation. Stateful effects must refuse unless their owning semantic
/// contract explicitly admits this boundary.
pub trait ReusableStepBack<const PORTS: usize>: StepBack<PORTS> {
    type Next;
    fn preflight_next(&self, next: &Self::Next, values: &dyn ValueStorage) -> bool;
    fn begin_next(&mut self, next: Self::Next);
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRefusal {
    NotFresh,
    UnsupportedRemote,
    InvalidOwnedBoundary,
    InvocationExhausted,
    NotNormallyDrained,
    OutstandingOwnership,
    EvidenceReservation,
    BackNotReady { node: NodeId },
}
/// Exact previous invocation evidence is transferred, never implicitly cleared.
pub struct DrainedInvocation<E> {
    pub signs: E,
    pub decisions: u32,
    pub logical_invocations: u64,
}
pub struct NextInvocationRefused<E, T> {
    pub reason: SessionRefusal,
    pub signs: E,
    pub next: T,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Phase {
    Prepared,
    Active,
    Drained,
    Faulted,
}

struct OptionalOwnedOutputs<const C: usize> {
    starts: [u64; C],
    optional: [bool; C],
}

mod optional_storage;
pub use optional_storage::OptionalOutputStorage;

pub struct PreparedKernelSession<
    D,
    S,
    E,
    const N: usize,
    const C: usize,
    const P: usize,
    const Q: usize,
    const R: usize,
    const T: usize,
    const H: usize = 0,
    const A: usize = 0,
> where
    D: ReusableStepBack<P>,
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    kernel: Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
    phase: Phase,
    remote_sign_reservation: u16,
    has_owned_boundaries: bool,
    owned_value_index: u64,
    optional_outputs: Option<Box<OptionalOwnedOutputs<C>>>,
}
impl<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const A: usize,
    > PreparedKernelSession<D, S, E, N, C, P, Q, R, T, H, A>
where
    D: ReusableStepBack<P>,
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    /// Takes an untouched prepared scheduler. This first profile has local Cords;
    /// transport epochs and remote acknowledgement are not inferred or reset.
    #[allow(clippy::type_complexity)]
    pub fn new(
        kernel: Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
    ) -> Result<
        Self,
        (
            SessionRefusal,
            Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        ),
    > {
        Self::new_with_owned_value_boundaries(kernel, &[], 0)
    }
    /// Opt-in for exclusively owned process-local Value adapters. The Host must
    /// establish exact Plan/Boot/Type and output acceptance outside this owner.
    /// Every non-local endpoint must be listed exactly once. This does not admit
    /// a network Line, restart transport epochs, or acknowledge output delivery.
    /// Remote payload sequences survive rearm; evidence storage is pre-admitted.
    #[allow(clippy::type_complexity)]
    pub fn new_with_owned_value_boundaries(
        kernel: Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        boundaries: &[OwnedValueBoundary],
        remote_sign_reservation: u16,
    ) -> Result<
        Self,
        (
            SessionRefusal,
            Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        ),
    > {
        Self::new_with_optional_owned_outputs(kernel, boundaries, &[], remote_sign_reservation)
    }
    /// Explicit zero-or-one output Values on selected process-local boundaries.
    /// Normal empty closure does not consume or fabricate a payload sequence.
    /// Inputs and other outputs retain the existing exactly-one-Value contract.
    #[allow(clippy::type_complexity)]
    pub fn new_with_optional_owned_outputs(
        kernel: Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        boundaries: &[OwnedValueBoundary],
        optional_outputs: &[OwnedValueBoundary],
        remote_sign_reservation: u16,
    ) -> Result<
        Self,
        (
            SessionRefusal,
            Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        ),
    > {
        let storage = (!optional_outputs.is_empty()).then(OptionalOutputStorage::prepare);
        Self::new_with_optional_storage(
            kernel,
            boundaries,
            optional_outputs,
            remote_sign_reservation,
            storage,
        )
    }

    /// Bind only validated exact optional endpoints into already allocated stores.
    /// Refusal drops the unused tracking store; the untouched kernel is returned.
    #[allow(clippy::type_complexity)]
    pub fn new_with_preallocated_optional_outputs(
        kernel: Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        boundaries: &[OwnedValueBoundary],
        optional_outputs: &[OwnedValueBoundary],
        remote_sign_reservation: u16,
        storage: OptionalOutputStorage<C>,
    ) -> Result<
        Self,
        (
            SessionRefusal,
            Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        ),
    > {
        Self::new_with_optional_storage(
            kernel,
            boundaries,
            optional_outputs,
            remote_sign_reservation,
            Some(storage),
        )
    }

    #[allow(clippy::type_complexity)]
    fn new_with_optional_storage(
        kernel: Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        boundaries: &[OwnedValueBoundary],
        optional_outputs: &[OwnedValueBoundary],
        remote_sign_reservation: u16,
        storage: Option<OptionalOutputStorage<C>>,
    ) -> Result<
        Self,
        (
            SessionRefusal,
            Box<FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>>,
        ),
    > {
        if optional_outputs.is_empty() != storage.is_none() {
            return Err((SessionRefusal::InvalidOwnedBoundary, kernel));
        }
        if kernel.decisions != 0
            || kernel.logical_invocations != 0
            || kernel.cancelled
            || !kernel.signs.is_empty()
            || !kernel.debug_control.is_clear()
            || kernel.pending_host_count != 0
            || kernel.queue_slots.iter().any(Option::is_some)
        {
            return Err((SessionRefusal::NotFresh, kernel));
        }
        let cords = &kernel.cord_specs[..kernel.active_cords];
        let external = owned_boundaries::has_external(cords);
        if external && boundaries.is_empty() {
            return Err((SessionRefusal::UnsupportedRemote, kernel));
        }
        if !owned_boundaries::exact(cords, boundaries)
            || !owned_boundaries::optional_outputs_exact(cords, boundaries, optional_outputs)
        {
            return Err((SessionRefusal::InvalidOwnedBoundary, kernel));
        }
        let remote = owned_boundaries::requires_line_signs(cords);
        let remote_sign_reservation = if external && !remote {
            0
        } else {
            remote_sign_reservation
        };
        if (remote && remote_sign_reservation == 0)
            || (remote_sign_reservation != 0
                && kernel
                    .signs
                    .ensure_remote_capacity(remote_sign_reservation)
                    .is_err())
        {
            return Err((SessionRefusal::EvidenceReservation, kernel));
        }
        for boundary in boundaries {
            let index = usize::from(boundary.cord.0);
            if kernel.cords[index].next_remote_sequence == u64::MAX {
                return Err((SessionRefusal::InvocationExhausted, kernel));
            }
        }
        let optional_outputs = storage.map(|storage| {
            let mut tracking = storage.inner;
            for boundary in boundaries {
                let index = usize::from(boundary.cord.0);
                tracking.starts[index] = kernel.cords[index].next_remote_sequence;
            }
            for boundary in optional_outputs {
                tracking.optional[usize::from(boundary.cord.0)] = true;
            }
            tracking
        });
        Ok(Self {
            kernel,
            phase: Phase::Prepared,
            remote_sign_reservation,
            has_owned_boundaries: !boundaries.is_empty(),
            owned_value_index: 0,
            optional_outputs,
        })
    }
    /// Additional managed heap storage required only for optional outputs.
    /// Ordinary lockstep sessions retain no per-Cord tracking allocation.
    pub fn optional_output_storage_requirement() -> usize {
        OptionalOutputStorage::<C>::required_bytes()
    }
    fn owned_sequence(&self, index: usize) -> u64 {
        self.optional_outputs
            .as_ref()
            .map_or(self.owned_value_index, |state| state.starts[index])
    }
    fn owned_output_is_optional(&self, index: usize) -> bool {
        self.optional_outputs
            .as_ref()
            .is_some_and(|state| state.optional[index])
    }
    pub fn kernel(&self) -> &FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A> {
        &self.kernel
    }
    /// Borrow the exact normally drained evidence before committing a rearm.
    pub fn drained_evidence(&self) -> Result<DrainedInvocation<&E>, SessionRefusal> {
        if self.phase != Phase::Drained {
            return Err(SessionRefusal::NotNormallyDrained);
        }
        Ok(DrainedInvocation {
            signs: &self.kernel.signs,
            decisions: self.kernel.decisions,
            logical_invocations: self.kernel.logical_invocations,
        })
    }
    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        let result = self.kernel.step();
        self.observe_step(result)
    }
    pub fn step_with_checkpoint(
        &mut self,
        proceed: impl FnMut(NodeId) -> bool,
    ) -> Result<SchedulerStatus, SchedulerError> {
        let result = self.kernel.step_with_checkpoint(proceed);
        self.observe_step(result)
    }
    fn observe_step(
        &mut self,
        result: Result<SchedulerStatus, SchedulerError>,
    ) -> Result<SchedulerStatus, SchedulerError> {
        if self.phase != Phase::Faulted {
            self.phase = match &result {
                Ok(SchedulerStatus::Drained) => Phase::Drained,
                Ok(SchedulerStatus::Cancelled) | Err(_) => Phase::Faulted,
                _ => Phase::Active,
            };
        }
        result
    }
    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        self.phase = Phase::Faulted;
        self.kernel.cancel()
    }
    pub fn next_host_request(&mut self) -> Option<crate::scheduler::HostCallRequest> {
        self.kernel.next_host_request()
    }
    pub fn complete_host_call(
        &mut self,
        node: NodeId,
        request: RequestId,
        outcome: HostCallOutcome,
    ) -> Result<(), SchedulerError> {
        let result = self.kernel.complete_host_call(node, request, outcome);
        if result.is_err() {
            self.phase = Phase::Faulted;
        }
        result
    }
    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<ValueRef, SchedulerError> {
        let result = self.kernel.store_host_value(bytes);
        if result.is_err() && self.phase == Phase::Active {
            self.phase = Phase::Faulted;
        }
        result
    }
    /// An explicit resident pin; it does not create or invalidate a ValueRef.
    pub fn retain_between_invocations(&mut self, value: ValueRef) -> Result<(), SchedulerError> {
        if !matches!(self.phase, Phase::Prepared | Phase::Drained) {
            return Err(SchedulerError::ValueOwnershipViolation);
        }
        self.kernel.values.retain(value).map_err(Into::into)
    }
    pub fn release_between_invocations(&mut self, value: ValueRef) -> Result<(), SchedulerError> {
        if !matches!(self.phase, Phase::Prepared | Phase::Drained) {
            return Err(SchedulerError::ValueOwnershipViolation);
        }
        self.kernel.values.release(value).map_err(Into::into)
    }
    /// All refusal checks precede mutation. Refusal returns the untouched next
    /// inputs and evidence reservation. Old mandatory evidence remains owned by
    /// this session until success transfers it to the caller.
    #[allow(clippy::result_large_err)]
    pub fn begin_next(
        &mut self,
        next: [D::Next; N],
        signs: E,
    ) -> Result<DrainedInvocation<E>, NextInvocationRefused<E, [D::Next; N]>> {
        if let Err(reason) = self.preflight(&next, &signs) {
            return Err(NextInvocationRefused {
                reason,
                signs,
                next,
            });
        }
        if self.has_owned_boundaries {
            self.owned_value_index += 1;
        }
        for (driver, input) in self.kernel.drivers[..self.kernel.active_nodes]
            .iter_mut()
            .zip(next)
        {
            driver.begin_next(input);
        }
        let old = DrainedInvocation {
            signs: core::mem::replace(&mut self.kernel.signs, signs),
            decisions: self.kernel.decisions,
            logical_invocations: self.kernel.logical_invocations,
        };
        // Immutable specs, routes, terminal contracts, recovery bindings, values,
        // prepared resources and consumed Host Request generations survive.
        self.kernel.completed.fill(false);
        self.kernel.ready = ReadySet::new(self.kernel.active_nodes);
        self.kernel.terminal_inputs_consumed.fill([false; P]);
        self.kernel.terminal_phases.fill(None);
        self.kernel.terminal_cancellation_pending.fill(false);
        self.kernel.unresolved_abnormal.fill(None);
        self.kernel.projected_recovery_source.fill(None);
        for (index, cord) in self.kernel.cords[..self.kernel.active_cords]
            .iter_mut()
            .enumerate()
        {
            let sequence = cord.next_remote_sequence;
            if let Some(tracking) = &mut self.optional_outputs {
                tracking.starts[index] = sequence;
            }
            *cord = CordState::EMPTY;
            cord.next_remote_sequence = sequence;
        }
        self.kernel.armed_quiescence_count = 0;
        self.kernel.cursor = 0;
        self.kernel.host_request_cursor = 0;
        self.kernel.host_slot_search_start = 0;
        self.kernel.cancellation_search_start = 0;
        self.kernel.decisions = 0;
        self.kernel.logical_invocations = 0;
        #[cfg(test)]
        self.kernel
            .scan_counts
            .set(super::scan_characterization::ScanCounts::default());
        self.phase = Phase::Prepared;
        Ok(old)
    }
    fn preflight(&self, next: &[D::Next; N], signs: &E) -> Result<(), SessionRefusal> {
        let kernel = &self.kernel;
        if self.has_owned_boundaries
            && (self.owned_value_index >= u64::MAX - 1
                || kernel.cords[..kernel.active_cords]
                    .iter()
                    .enumerate()
                    .any(|(index, cord)| {
                        (!matches!(kernel.cord_specs[index].source, CordEndpoint::Local { .. })
                            || !matches!(kernel.cord_specs[index].sink, CordEndpoint::Local { .. }))
                            && cord.next_remote_sequence == u64::MAX
                    }))
        {
            return Err(SessionRefusal::InvocationExhausted);
        }
        if self.phase != Phase::Drained
            || kernel.cancelled
            || !kernel.completed[..kernel.active_nodes]
                .iter()
                .all(|value| *value)
            || !kernel.debug_control.is_clear()
            || kernel
                .signs
                .contains_kind(crate::KernelEventKind::SemanticAbnormal)
            || kernel.signs.retention_gap().is_some()
        {
            return Err(SessionRefusal::NotNormallyDrained);
        }
        if kernel.pending_host_count != 0
            || kernel.pending_host_calls.iter().any(Option::is_some)
            || kernel.pending_host_slots.iter().any(Option::is_some)
            || kernel.undispatched_host_count != 0
            || kernel.undispatched_cancellation_count != 0
            || kernel.queue_slots.iter().any(Option::is_some)
            || kernel.cords[..kernel.active_cords]
                .iter()
                .enumerate()
                .any(|(index, cord)| {
                    cord.len != 0
                        || cord.queued_bytes != 0
                        || cord.offered_remote_sequence.is_some()
                        || cord.remote_accepted
                        || cord.producer_abnormal
                        || (self.has_owned_boundaries
                            && (!cord.producer_closed
                                || ((!matches!(
                                    kernel.cord_specs[index].source,
                                    CordEndpoint::Local { .. }
                                ) || !matches!(
                                    kernel.cord_specs[index].sink,
                                    CordEndpoint::Local { .. }
                                )) && !owned_boundaries::sequence_completed(
                                    self.owned_sequence(index),
                                    cord.next_remote_sequence,
                                    self.owned_output_is_optional(index),
                                ))))
                })
        {
            return Err(SessionRefusal::OutstandingOwnership);
        }
        if !signs.is_empty()
            || signs.item_capacity() != kernel.signs.item_capacity()
            || signs.byte_capacity() != kernel.signs.byte_capacity()
            || signs.retention_gap().is_some()
            || (self.remote_sign_reservation != 0
                && signs
                    .ensure_remote_capacity(self.remote_sign_reservation)
                    .is_err())
        {
            return Err(SessionRefusal::EvidenceReservation);
        }
        for (node, driver) in kernel.drivers[..kernel.active_nodes].iter().enumerate() {
            if !driver.preflight_next(&next[node], &kernel.values) {
                return Err(SessionRefusal::BackNotReady {
                    node: NodeId(node as u16),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
