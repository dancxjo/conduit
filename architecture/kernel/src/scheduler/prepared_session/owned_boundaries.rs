//! Private Value adapter ownership, distinct from remote transport admission.
use super::*;
use crate::scheduler::{CordSpec, RemoteIngressOutcome, RemoteValueOffer};
use crate::{CordId, RemoteEndpointId};

/// Exact non-local Cord endpoint owned exclusively by a local Value adapter.
/// This declaration carries no network, resource or authority permission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnedValueBoundary {
    pub cord: CordId,
    pub endpoint: RemoteEndpointId,
}

pub(super) fn has_external(cords: &[CordSpec]) -> bool {
    cords
        .iter()
        .any(|cord| cord.source_local().is_none() || cord.sink_local().is_none())
}
/// Only full assigned preparation can classify local Fore. Its owned Value
/// sequencing remains independent from actual Line lifecycle Sign custody.
pub(super) fn requires_line_signs(cords: &[CordSpec]) -> bool {
    cords.iter().any(|cord| {
        !cord.is_local_fore() && (cord.source_local().is_none() || cord.sink_local().is_none())
    })
}

pub(super) fn exact(cords: &[CordSpec], boundaries: &[OwnedValueBoundary]) -> bool {
    if boundaries.len() > cords.len() {
        return false;
    }
    for (i, boundary) in boundaries.iter().enumerate() {
        if boundaries[..i]
            .iter()
            .any(|previous| previous.cord == boundary.cord)
        {
            return false;
        }
        let Some(cord) = cords.get(usize::from(boundary.cord.0)) else {
            return false;
        };
        if !matches!((cord.source, cord.sink),
            (CordEndpoint::Remote(endpoint), CordEndpoint::Local { .. }) |
            (CordEndpoint::Local { .. }, CordEndpoint::Remote(endpoint)) if endpoint == boundary.endpoint)
        {
            return false;
        }
    }
    cords.iter().all(|cord| match (cord.source, cord.sink) {
        (CordEndpoint::Local { .. }, CordEndpoint::Local { .. }) => true,
        (CordEndpoint::Remote(endpoint), CordEndpoint::Local { .. })
        | (CordEndpoint::Local { .. }, CordEndpoint::Remote(endpoint)) => boundaries
            .iter()
            .any(|b| b.cord == cord.cord && b.endpoint == endpoint),
        _ => false,
    })
}

pub(super) fn optional_outputs_exact(
    cords: &[CordSpec],
    boundaries: &[OwnedValueBoundary],
    optional: &[OwnedValueBoundary],
) -> bool {
    optional.len() <= boundaries.len() && optional.iter().enumerate().all(|(index, output)| {
        boundaries.contains(output)
            && !optional[..index].iter().any(|prior| prior.cord == output.cord)
            && cords.get(usize::from(output.cord.0)).is_some_and(|cord|
                matches!((cord.source, cord.sink),
                    (CordEndpoint::Local { .. }, CordEndpoint::Remote(endpoint)) if endpoint == output.endpoint))
    })
}
pub(super) fn sequence_completed(start: u64, actual: u64, optional: bool) -> bool {
    (optional && actual == start) || start.checked_add(1) == Some(actual)
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
        const TERMINAL_ROWS: usize,
        const PHASE_BYTES: usize,
    > PreparedKernelSession<D, S, E, N, C, P, Q, R, T, H, A, TERMINAL_ROWS, PHASE_BYTES>
where
    D: ReusableStepBack<P>,
    S: ValueStorage,
    E: SignSink + SignQuery,
{
    pub fn optional_output_storage_bytes(&self) -> usize {
        if self.optional_outputs.is_some() {
            Self::optional_output_storage_requirement()
        } else {
            0
        }
    }
    fn owned_io(&self) -> Result<(), SchedulerError> {
        if !self.has_owned_boundaries || !matches!(self.phase, Phase::Prepared | Phase::Active) {
            return Err(SchedulerError::InvalidRemoteCordAccess);
        }
        Ok(())
    }
    fn observe_io<V>(&mut self, result: Result<V, SchedulerError>) -> Result<V, SchedulerError> {
        if result.is_err() {
            self.phase = Phase::Faulted;
        }
        result
    }
    pub fn admit_owned_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        self.owned_io()?;
        let index = usize::from(cord.0);
        let spec = self
            .kernel
            .cord_specs
            .get(index)
            .filter(|_| index < self.kernel.active_cords)
            .filter(|spec| spec.source == CordEndpoint::Remote(endpoint))
            .ok_or(SchedulerError::InvalidRemoteCordAccess)?;
        if bytes.len() > spec.maximum_value_bytes as usize {
            return self.observe_io(Err(SchedulerError::QueueByteCapacityExceeded));
        }
        // One Value per declared input in each invocation. A subsequent Value
        // is pressured until an explicit acknowledged drain and rearm; it is
        // never smuggled into an already consumed Value input queue.
        if self.owned_sequence(index).checked_add(1) == Some(sequence)
            && self.kernel.cords[index].next_remote_sequence == sequence
        {
            return Ok(RemoteIngressOutcome::Full { sequence });
        }
        if sequence != self.owned_sequence(index) {
            return self.observe_io(Err(SchedulerError::RemoteSequenceRejected));
        }
        let result = self
            .kernel
            .admit_remote_input(endpoint, cord, sequence, bytes);
        if matches!(result, Ok(RemoteIngressOutcome::Accepted { .. })) {
            self.phase = Phase::Active;
        }
        self.observe_io(result)
    }
    pub fn close_owned_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<(), SchedulerError> {
        self.owned_io()?;
        let index = usize::from(cord.0);
        if index >= self.kernel.active_cords
            || self.kernel.cord_specs[index].source != CordEndpoint::Remote(endpoint)
        {
            return self.observe_io(Err(SchedulerError::InvalidRemoteCordAccess));
        }
        if self.owned_sequence(index).checked_add(1)
            != Some(self.kernel.cords[index].next_remote_sequence)
        {
            return self.observe_io(Err(SchedulerError::ValueOwnershipViolation));
        }
        let result = self.kernel.close_remote_input(endpoint, cord);
        self.observe_io(result)
    }
    pub fn owned_output(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
    ) -> Result<Option<RemoteValueOffer>, SchedulerError> {
        self.owned_io()?;
        let index = usize::from(cord.0);
        if index >= self.kernel.active_cords {
            return self.observe_io(Err(SchedulerError::InvalidRemoteCordAccess));
        }
        let result = self.kernel.remote_egress_offer(endpoint, cord);
        if result.as_ref().is_ok_and(|offer| {
            offer.is_some_and(|offer| offer.sequence != self.owned_sequence(index))
        }) {
            return self.observe_io(Err(SchedulerError::RemoteSequenceRejected));
        }
        self.observe_io(result)
    }
    /// Accept and retire only the exact current output. A bad acknowledgement
    /// faults this invocation; it can never silently rearm or replay a Value.
    pub fn accept_owned_output(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
    ) -> Result<(), SchedulerError> {
        self.owned_io()?;
        let index = usize::from(cord.0);
        if index >= self.kernel.active_cords {
            return self.observe_io(Err(SchedulerError::InvalidRemoteCordAccess));
        }
        if sequence != self.owned_sequence(index) {
            return self.observe_io(Err(SchedulerError::RemoteSequenceRejected));
        }
        let result = self
            .kernel
            .remote_egress_accept(endpoint, cord, sequence)
            .and_then(|()| {
                self.kernel
                    .remote_egress_delivered(endpoint, cord, sequence)
            });
        self.observe_io(result)
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod optional_output_tests;
