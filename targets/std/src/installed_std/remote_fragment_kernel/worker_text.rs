//! Inspect a real pending effect without acknowledging or completing it.
use super::InstalledRemoteFragment;
use crate::worker_text_result::WorkerTextResult;
use conduit_core::{HostAdvertisement, PRESENT_HOST_CALL_CONTRACT};
use conduit_kernel::scheduler::HostCallRequest;
impl InstalledRemoteFragment {
    /// Prepare a result payload for owner authentication from the actual pending
    /// face/text input. This performs no IO and leaves kernel custody intact.
    /// The Line driver completes the original call only after actual delivery;
    /// loss/cancellation follows the existing remote fragment cancellation path.
    pub fn pending_worker_text_result(
        &self,
        host: &HostAdvertisement,
        request: HostCallRequest,
    ) -> Result<WorkerTextResult, String> {
        if !self.scheduler.host_call_is_pending(request) {
            return Err("worker text request is no longer pending".into());
        }
        let operation = self
            .lowered
            .host_calls
            .iter()
            .find(|op| op.node == request.node && op.call == request.call)
            .ok_or("worker text request has no lowered operation")?;
        if operation.contract_id.as_str() != PRESENT_HOST_CALL_CONTRACT
            || operation.target_kind.as_ref().map(|k| k.as_str())
                != Some(conduit_std_offers::TEXT_FACE_TARGET)
        {
            return Err("worker text request is not the admitted text presenter".into());
        }
        let placement = self
            .lowered
            .identity
            .placements
            .iter()
            .find(|(node, _)| *node == request.node)
            .map(|(_, placement)| placement)
            .ok_or("worker text node has no placement")?;
        let placement = self
            .placements
            .iter()
            .find(|p| &p.placement_id == placement)
            .ok_or("worker text placement was not retained")?;
        let work = self.host_request_view(request)?;
        WorkerTextResult::from_pending(
            host,
            &self.worker_text_basis,
            placement,
            request,
            work.input,
        )
    }
    /// Finish the selected stdout presenter only after the caller has observed
    /// authenticated result delivery. A write failure is not a completion and
    /// must not be retried as if it had no effect.
    pub fn complete_worker_text_to(
        &mut self,
        host: &HostAdvertisement,
        request: HostCallRequest,
        output: &mut impl std::io::Write,
    ) -> Result<(), String> {
        let result = self.pending_worker_text_result(host, request)?;
        let text = &result.fragment().text[0].text;
        let label = &result.fragment().subjects[0].name;
        super::super::simple_face_host::present_named(
            Some(&conduit_core::kind_id(conduit_std_offers::TEXT_FACE_TARGET)),
            text.as_bytes(),
            output,
            Some(label),
        )?;
        self.complete_host_call(
            request,
            conduit_kernel::HostCallOutcome {
                disposition: conduit_kernel::HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
    }
}
