//! Read-only transfer of an exact unstarted Body proposal to a Host consumer.
//! Transfer does not acquire resources, admit execution, or create a Play.
use super::{PatchbayHtmlServer, ServerError};
use conduit_body::{BodyPlan, Wake, WakeLifecycle};
use serde::Serialize;
use std::io::{self, Write};
use std::net::TcpStream;

// Leave room for Host observations in the browser's 256 KiB start input arena.
const MAX_PROPOSAL_BYTES: usize = 192 * 1024;

#[derive(Serialize)]
struct ExecutionProposal<'a> {
    schema: &'static str,
    wake: &'a Wake,
    plan: &'a BodyPlan,
}

struct BoundedOutput(Vec<u8>);

impl Write for BoundedOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_PROPOSAL_BYTES.saturating_sub(self.0.len()) {
            return Err(io::Error::other(
                "Body execution proposal exceeds its byte bound",
            ));
        }
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl PatchbayHtmlServer {
    pub(super) fn body_execution_proposal(&self) -> Result<Vec<u8>, ServerError> {
        let refuse = |reason: &str| ServerError::Interaction(reason.into());
        let planning = self
            .body_planning
            .as_ref()
            .ok_or_else(|| refuse("BodyProposalAbsent"))?;
        let wake = planning.wake();
        if wake.lifecycle != WakeLifecycle::AwaitingPlan || !wake.plans.is_empty() {
            return Err(refuse("BodyProposalAlreadyAdmitted"));
        }
        if planning.snapshot().unavailable_proposal_sign_id.is_some() {
            return Err(refuse("BodyProposalUnavailable"));
        }
        let evidence = self
            .body_workload
            .as_ref()
            .ok_or_else(|| refuse("BodyWorkloadAbsent"))?
            .evidence();
        if evidence.body.body_id != wake.body_id
            || evidence.body.workload_revision != wake.workload_revision
            || evidence.body.workset != wake.workset
        {
            return Err(refuse("BodyProposalStaleWorkload"));
        }
        let plan = planning.current_plan();
        plan.validate_for(wake)
            .map_err(|_| refuse("BodyProposalInvalidPlan"))?;
        for fragment in plan.forms.iter().flat_map(|form| &form.plan.fragments) {
            if !evidence.membership.parts.iter().any(|part| {
                part.state == conduit_body::MembershipState::Admitted
                    && part.current.as_ref().is_some_and(|current| {
                        current.host_id == fragment.host_id
                            && current.boot_id == fragment.boot_id
                            && current.offer_generation == fragment.offer_generation
                    })
            }) {
                return Err(refuse("BodyProposalStaleHost"));
            }
        }
        let proposal = ExecutionProposal {
            schema: "conduit.patchbay/body-execution-proposal@1",
            wake,
            plan,
        };
        let mut output = BoundedOutput(Vec::with_capacity(MAX_PROPOSAL_BYTES));
        serde_json::to_writer(&mut output, &proposal)
            .map_err(|error| ServerError::Interaction(error.to_string()))?;
        Ok(output.0)
    }

    pub(super) fn deliver_body_execution_proposal(
        &self,
        stream: &mut TcpStream,
    ) -> Result<(), ServerError> {
        match self.body_execution_proposal() {
            Ok(body) => {
                super::write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
            }
            Err(ServerError::Interaction(reason)) => super::write_response(
                stream,
                "409 Conflict",
                "text/plain; charset=utf-8",
                reason.as_bytes(),
            ),
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests;
