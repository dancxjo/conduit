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
    fn owned_io(&self) -> Result<(), SchedulerError> {
        if self.remote_sign_reservation == 0
            || !matches!(self.phase, Phase::Prepared | Phase::Active)
        {
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
        if sequence == self.owned_value_index + 1
            && self.kernel.cords[index].next_remote_sequence == sequence
        {
            return Ok(RemoteIngressOutcome::Full { sequence });
        }
        if sequence != self.owned_value_index {
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
        if self.kernel.cords[index].next_remote_sequence != self.owned_value_index + 1 {
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
        let result = self.kernel.remote_egress_offer(endpoint, cord);
        if result
            .as_ref()
            .is_ok_and(|offer| offer.is_some_and(|offer| offer.sequence != self.owned_value_index))
        {
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
        if sequence != self.owned_value_index {
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
