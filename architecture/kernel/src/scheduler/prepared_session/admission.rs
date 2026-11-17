//! Exact owned-boundary and preallocated optional-output admission.
//! Existing default entrances use this same validation without optional storage.
use super::*;
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
        let remote = cords.iter().any(|cord| {
            !matches!(cord.source, CordEndpoint::Local { .. })
                || !matches!(cord.sink, CordEndpoint::Local { .. })
        });
        if remote && boundaries.is_empty() {
            return Err((SessionRefusal::UnsupportedRemote, kernel));
        }
        if !owned_boundaries::exact(cords, boundaries)
            || !owned_boundaries::optional_outputs_exact(cords, boundaries, optional_outputs)
        {
            return Err((SessionRefusal::InvalidOwnedBoundary, kernel));
        }
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
            owned_value_index: 0,
            optional_outputs,
        })
    }
    /// Additional managed heap storage required only for optional outputs.
    /// Ordinary lockstep sessions retain no per-Cord tracking allocation.
    pub fn optional_output_storage_requirement() -> usize {
        OptionalOutputStorage::<C>::required_bytes()
    }
    pub fn optional_output_storage_bytes(&self) -> usize {
        if self.optional_outputs.is_some() {
            Self::optional_output_storage_requirement()
        } else {
            0
        }
    }
    pub(super) fn owned_sequence(&self, index: usize) -> u64 {
        self.optional_outputs
            .as_ref()
            .map_or(self.owned_value_index, |state| state.starts[index])
    }
    pub(super) fn owned_output_is_optional(&self, index: usize) -> bool {
        self.optional_outputs
            .as_ref()
            .is_some_and(|state| state.optional[index])
    }
}
