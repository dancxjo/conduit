//! Trusted-loopback Body execution accounting, not remote authentication.
//! Claims live only in this server session; no restart fencing is asserted.
use super::{PatchbayHtmlServer, ServerError};
use conduit_body::{BodyPlayIdentity, Wake};
use conduit_core::{BootId, HostId, PlanId, SignId};
use serde::Deserialize;
use std::net::TcpStream;

pub(super) mod history;

const MAX_EXECUTION_REQUEST_BYTES: usize = 64 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExecutionRequest {
    schema: String,
    action: ExecutionAction,
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum ExecutionAction {
    Lull {
        wake_id: conduit_body::WakeId,
    },
    Claim {
        plan_id: PlanId,
        host_id: HostId,
        boot_id: BootId,
    },
    Started {
        play: BodyPlayIdentity,
        wake_at_start: Wake,
    },
    RefusedBeforeStart {
        play: BodyPlayIdentity,
        reason: String,
    },
    Terminal {
        play: BodyPlayIdentity,
        disposition: String,
        terminal_sign_id: SignId,
    },
}

impl PatchbayHtmlServer {
    pub(super) fn apply_body_execution(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ServerError> {
        if bytes.is_empty() || bytes.len() > MAX_EXECUTION_REQUEST_BYTES {
            return Err(ServerError::InvalidRequest);
        }
        let request: ExecutionRequest =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        if request.schema != "conduit.patchbay/body-execution-request@1" {
            return Err(ServerError::InvalidRequest);
        }
        if matches!(request.action, ExecutionAction::Claim { .. }) {
            // Check current workload and admitted Host/Boot/offer generation at
            // the mutation boundary, not merely when the proposal was fetched.
            self.body_execution_proposal()?;
        }
        let mut planning = self
            .body_planning
            .clone()
            .ok_or_else(|| ServerError::Interaction("BodyProposalAbsent".into()))?;
        let report = match request.action {
            ExecutionAction::Lull { wake_id } => {
                if planning.wake().wake_id != wake_id {
                    return Err(ServerError::Interaction("BodyLullStaleWake".into()));
                }
                let sequence = self
                    .snapshot
                    .interaction
                    .revision
                    .checked_add(1)
                    .ok_or_else(|| {
                        ServerError::Interaction("BodyExecutionRevisionExhausted".into())
                    })?;
                planning
                    .lull(
                        SignId::from(format!("patchbay-html/body-lull/{sequence}/wake")),
                        SignId::from(format!("patchbay-html/body-lull/{sequence}/retained")),
                    )
                    .map_err(|error| ServerError::Interaction(format!("BodyLull{error:?}")))?;
                Ok(())
            }
            ExecutionAction::Claim {
                plan_id,
                host_id,
                boot_id,
            } => planning
                .claim_execution(&plan_id, &host_id, &boot_id)
                .map(|_| ()),
            ExecutionAction::Started {
                play,
                wake_at_start,
            } => planning.report_execution_started(&play, &wake_at_start),
            ExecutionAction::RefusedBeforeStart { play, reason } => {
                planning.report_execution_refused(&play, &reason)
            }
            ExecutionAction::Terminal {
                play,
                disposition,
                terminal_sign_id,
            } => planning.report_execution_terminal(&play, &disposition, &terminal_sign_id),
        };
        report.map_err(|error| ServerError::Interaction(format!("BodyExecution{error:?}")))?;
        let session = self
            .body_workload
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("BodyWorkloadAbsent".into()))?;
        let (session, mut snapshot) = history::retain(&self.snapshot, session, &planning)?;
        snapshot.body_planning = Some(planning.snapshot());
        snapshot.interaction.revision = snapshot
            .interaction
            .revision
            .checked_add(1)
            .ok_or_else(|| ServerError::Interaction("BodyExecutionRevisionExhausted".into()))?;
        snapshot.interaction.last_request_id = Some("body-execution/report".into());
        snapshot.interaction.last_disposition =
            Some("Succeeded(BodyExecutionAccountingUpdated)".into());
        let encoded = snapshot.encode()?;
        let navigation = super::navigation_state(&snapshot)?;
        self.body_workload = Some(session);
        self.body_planning = Some(planning);
        self.navigation = navigation;
        self.snapshot = snapshot;
        self.encoded_snapshot = encoded;
        Ok(self.encoded_snapshot.clone())
    }

    pub(super) fn deliver_body_execution(
        &mut self,
        stream: &mut TcpStream,
        bytes: &[u8],
    ) -> Result<(), ServerError> {
        match self.apply_body_execution(bytes) {
            Ok(body) => {
                super::write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
            }
            Err(ServerError::InvalidRequest) => super::write_response(
                stream,
                "400 Bad Request",
                "text/plain; charset=utf-8",
                b"invalid Body execution request",
            ),
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
