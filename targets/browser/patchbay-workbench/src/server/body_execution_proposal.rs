//! Read-only transfer of an exact unstarted Body proposal to a host consumer.
//! Transfer does not acquire resources, admit execution, or create a play.
use super::{PatchbayHtmlServer, ServerError};
use conduit_body::{BodyPlan, Wake, WakeLifecycle};
use serde::Serialize;
use std::io::{self, Write};
use std::net::TcpStream;

// The ordinary two-resident keyboard workload carries 1,701,516 bytes of exact plan metadata.
// Browser Body start reserves a separate 64 KiB for host framing and observations.
const MAX_PROPOSAL_BYTES: usize = 2 * 1024 * 1024;

#[derive(Serialize)]
struct ExecutionProposal<'a> {
    schema: &'static str,
    wake: &'a Wake,
    plan: &'a BodyPlan,
    body_evidence: &'a conduit_body::BodyBiographyEvidence,
    source: String,
    owner_face_revision: String,
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
    pub(super) fn body_planning_source(&self) -> Result<String, ServerError> {
        let plots = self
            .body_planning_plots
            .iter()
            .map(|candidate| {
                Ok(serde_json::json!({
                    "slug": candidate.checked_plot_id.as_str(),
                    "entry": candidate.checked_entry_name().map_err(ServerError::Interaction)?,
                    "title": candidate.label,
                    "source": candidate.source,
                }))
            })
            .collect::<Result<Vec<_>, ServerError>>()?;
        Ok(
            serde_json::json!({"schema":"conduit.creche/reviewed-plot-bundle@2","plots":plots})
                .to_string(),
        )
    }

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
        let domain_fragments = plan.plots.iter().flat_map(|plot| &plot.plan.fragments);
        let selected_mask_fragments = plan
            .mask_topologies
            .iter()
            .flat_map(|topology| &topology.chains)
            .flat_map(|chain| &chain.plan.fragments)
            .filter(|_| self.selected_browser_mask);
        for fragment in domain_fragments.chain(selected_mask_fragments) {
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
        // A selected remote Mask starts from this retained owner revision.  Its
        // first post-Started projection must take history::retain's one append.
        let owner_face_revision = self
            .snapshot
            .body_workbench
            .as_ref()
            .ok_or_else(|| refuse("BodyWorkbenchAbsent"))?
            .evidence_revision;
        if self.selected_browser_mask && !evidence.wakes.iter().any(|retained| retained == wake) {
            return Err(refuse("BodyProposalOwnerBasisNotRetained"));
        }
        owner_face_revision
            .checked_add(1)
            .ok_or_else(|| refuse("BodyEvidenceRevisionExhausted"))?;
        let proposal = ExecutionProposal {
            schema: "conduit.body/execution-proposal@1",
            wake,
            plan,
            body_evidence: evidence,
            source: self.body_planning_source()?,
            owner_face_revision: owner_face_revision.to_string(),
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
pub(super) mod tests;
