//! Explicit canonical Face-value broker; the legacy 3 KiB broker stays distinct.
use super::*;
use conduit_kernel::scheduler::RemoteIngressOutcome;

impl FaceScheduler {
    pub fn new_face(admission: Admission) -> Result<Self, SchedulerError> {
        admission.validate_face()?;
        Self::prepare(admission, true)
    }

    /// Only Root's current admitted Fore/Mask route can call this boundary.
    /// Unknown routes, stale sequences and oversized values refuse before entry.
    pub fn admit_remote_input(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        match self
            .ledger
            .preflight_ingress(endpoint, cord, sequence, bytes)
        {
            Ok(()) => {}
            Err(SchedulerError::OutputBlocked) => {
                return Ok(RemoteIngressOutcome::Full { sequence });
            }
            Err(error) => return Err(error),
        }
        if self.staged.is_some() {
            return Err(SchedulerError::InvalidHostCallAccess);
        }
        let result = self.ingress_exchange(endpoint, cord, sequence, bytes);
        match result {
            Ok(outcome) => Ok(outcome),
            // Once guest entry occurs, an uncertain response never authorizes
            // implicit replay or another grant. Quarantine follows the existing
            // broker rule, preserving normal Full as a distinct live outcome.
            Err(error) => self.fail(error),
        }
    }

    fn ingress_exchange(
        &mut self,
        endpoint: RemoteEndpointId,
        cord: CordId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        let window = self
            .domain
            .as_mut()
            .ok_or(SchedulerError::Cancelled)?
            .transfer_window()
            .map_err(|_| SchedulerError::Cancelled)?;
        window.fill(0);
        window
            .get_mut(..bytes.len())
            .ok_or(SchedulerError::QueueByteCapacityExceeded)?
            .copy_from_slice(bytes);
        let response = self.request(Request::AdmitRemote {
            endpoint,
            cord,
            sequence,
            bytes: bytes.len() as u32,
        })?;
        match response {
            Response::RemoteAdmission(RemoteIngressOutcome::Accepted { sequence: observed })
                if observed == sequence =>
            {
                self.ledger
                    .accepted_ingress(endpoint, cord, sequence, bytes)?;
                Ok(RemoteIngressOutcome::Accepted { sequence })
            }
            Response::RemoteAdmission(RemoteIngressOutcome::Full { sequence: observed })
                if observed == sequence =>
            {
                Ok(RemoteIngressOutcome::Full { sequence })
            }
            _ => Err(SchedulerError::InvalidHostCallAccess),
        }
    }
}
