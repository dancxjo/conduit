//! Cooperative owner transport. Local runtime admission remains independent.
use super::{PatchbayHtmlServer, ServerError};
use conduit_body::{BodyExecutionPhase, BodyPlayIdentity};
use conduit_face::{FaceInteraction, MaskShow};
use serde::{Deserialize, Serialize};
use std::net::TcpStream;

#[derive(Clone)]
pub(super) struct PendingFaceLull {
    pub play: BodyPlayIdentity,
    pub wake_id: conduit_body::WakeId,
}

#[derive(Deserialize)]
struct Observation {
    schema: String,
    body_plan_id: conduit_core::PlanId,
    active_play_id: conduit_core::ActivePlayId,
    face: conduit_face::Face,
    mask_show: Option<MaskShow>,
    interaction: Option<InteractionReceipt>,
}
#[derive(Deserialize)]
struct InteractionReceipt {
    correlation: conduit_face::MaskInteractionCorrelation,
}
#[derive(Serialize)]
struct Publication<'a> {
    schema: &'static str,
    body_plan_id: &'a conduit_core::PlanId,
    active_play_id: &'a conduit_core::ActivePlayId,
    revision: u64,
    body_evidence: &'a conduit_body::BodyBiographyEvidence,
    reviewed_plots: &'a [crate::BrowserReviewedPlot],
    face: &'a conduit_face::Face,
}
fn refuse(reason: &str) -> ServerError {
    ServerError::Interaction(reason.into())
}
impl PatchbayHtmlServer {
    fn current_body_face_claim(&self) -> Result<conduit_body::BodyExecutionClaim, ServerError> {
        if !self.selected_browser_mask || self.pending_face_lull.is_some() {
            return Err(refuse("BodyFacePublicationUnavailable"));
        }
        let planning = self
            .body_planning
            .as_ref()
            .ok_or_else(|| refuse("BodyProposalAbsent"))?;
        let snapshot = planning.snapshot();
        let claim = snapshot
            .execution_claims
            .iter()
            .rev()
            .find(|claim| {
                claim.phase == BodyExecutionPhase::Started
                    && claim.play.plan_id == planning.current_plan().plan_id
            })
            .ok_or_else(|| refuse("BodyFaceCurrentStartedClaimAbsent"))?;
        Ok(claim.clone())
    }
    pub(super) fn body_face_publication(&self) -> Result<Vec<u8>, ServerError> {
        let claim = self.current_body_face_claim()?;
        let workbench = self
            .snapshot
            .body_workbench
            .as_ref()
            .ok_or_else(|| refuse("BodyWorkbenchAbsent"))?;
        let evidence = self
            .body_workload
            .as_ref()
            .ok_or_else(|| refuse("BodyWorkloadAbsent"))?
            .evidence();
        serde_json::to_vec(&Publication {
            schema: "conduit.browser/body-face-publication@1",
            body_plan_id: &claim.play.plan_id,
            active_play_id: &claim.play.active_play_id,
            revision: workbench.evidence_revision,
            body_evidence: evidence,
            reviewed_plots: &workbench.reviewed_plots,
            face: &self.snapshot.face,
        })
        .map_err(|_| ServerError::InvalidRequest)
    }
    fn checked_body_face_observation(
        &self,
        bytes: &[u8],
    ) -> Result<(Observation, conduit_body::BodyExecutionClaim), ServerError> {
        if bytes.is_empty() || bytes.len() > 1024 * 1024 {
            return Err(ServerError::InvalidRequest);
        }
        let observation: Observation =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        let claim = self.current_body_face_claim()?;
        if observation.schema != "conduit.browser/body-face-observation@1"
            || observation.body_plan_id != claim.play.plan_id
            || observation.active_play_id != claim.play.active_play_id
            || observation.face != self.snapshot.face
        {
            return Err(refuse("BodyFaceStaleOwnerBasis"));
        }
        Ok((observation, claim))
    }
    pub(super) fn acknowledge_body_face(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ServerError> {
        let (observation, claim) = self.checked_body_face_observation(bytes)?;
        if self.body_face_show.is_some() || observation.interaction.is_some() {
            return Err(refuse("BodyFaceDuplicateAcknowledgement"));
        }
        let show = observation
            .mask_show
            .ok_or_else(|| refuse("BodyFaceShowAbsent"))?;
        show.validate(&self.snapshot.face)
            .map_err(|_| refuse("BodyFaceInvalidShow"))?;
        if show.show.host_id != claim.host_id || show.show.boot_id != claim.boot_id {
            return Err(refuse("BodyFaceForeignShow"));
        }
        let mut control = self
            .mask_control
            .clone()
            .ok_or_else(|| refuse("BodyFaceMaskAbsent"))?;
        control
            .acknowledge_selected_graphical(&show)
            .map_err(|_| refuse("BodyFaceMaskNotAcknowledged"))?;
        let topology = control
            .project_current(self.body_planning.as_ref().unwrap(), &claim.play)
            .map_err(|_| refuse("BodyFaceTopologyInvalid"))?;
        let mut snapshot = self.snapshot.clone();
        snapshot.mask_topology = Some(
            patchbay_application::project_mask_topology(control.face(), &topology)
                .map_err(|_| refuse("BodyFaceTopologyInvalid"))?,
        );
        let encoded = snapshot.encode()?;
        self.mask_control = Some(control);
        self.body_face_show = Some(show);
        self.snapshot = snapshot;
        self.encoded_snapshot = encoded;
        Ok(self.encoded_snapshot.clone())
    }
    pub(super) fn interact_body_face(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ServerError> {
        let (observation, claim) = self.checked_body_face_observation(bytes)?;
        let show = self
            .body_face_show
            .as_ref()
            .ok_or_else(|| refuse("BodyFaceShowNotAcknowledged"))?;
        if observation.mask_show.as_ref() != Some(show) {
            return Err(refuse("BodyFaceForeignShow"));
        }
        let correlation = observation
            .interaction
            .ok_or_else(|| refuse("BodyFaceInteractionAbsent"))?
            .correlation;
        let interaction: FaceInteraction = correlation.interaction.clone();
        if show
            .correlate_interaction(interaction.clone())
            .map_err(|_| refuse("BodyFaceInteractionInvalid"))?
            != correlation
        {
            return Err(refuse("BodyFaceCorrelationInvalid"));
        }
        interaction
            .validate_against(&self.snapshot.face, show)
            .map_err(|_| refuse("BodyFaceInteractionInvalid"))?;
        let action = self
            .snapshot
            .face
            .resolve_action(self.snapshot.face.revision, &interaction.action_id)
            .map_err(|_| refuse("BodyFaceActionInvalid"))?;
        if action.intent != "conduit.intent/lull@1" || interaction.sequence != 1 {
            return Err(refuse("BodyFaceActionUnsupported"));
        }
        let pending = PendingFaceLull {
            play: claim.play,
            wake_id: self.body_planning.as_ref().unwrap().wake().wake_id.clone(),
        };
        let response = serde_json::to_vec(&serde_json::json!({"schema":"conduit.browser/body-face-action@1","disposition":"awaiting-terminal","play":pending.play}))
            .map_err(|_| ServerError::InvalidRequest)?;
        let mut snapshot = self.snapshot.clone();
        snapshot.mask_topology = None;
        snapshot.interaction.revision = snapshot
            .interaction
            .revision
            .checked_add(1)
            .ok_or_else(|| refuse("BodyFaceInteractionRevisionExhausted"))?;
        snapshot.interaction.last_disposition =
            Some("Pending(BodyFaceLullAwaitingTerminal)".into());
        let encoded = snapshot.encode()?;
        let mut control = self
            .mask_control
            .clone()
            .ok_or_else(|| refuse("BodyFaceMaskAbsent"))?;
        control.retire_selected_graphical_show();
        self.mask_control = Some(control);
        self.snapshot = snapshot;
        self.encoded_snapshot = encoded;
        self.pending_face_lull = Some(pending);
        self.body_face_show = None;
        Ok(response)
    }
    fn body_face_state(&self) -> Result<Vec<u8>, ServerError> {
        let pending = self.pending_face_lull.as_ref().map(|pending| serde_json::json!({
            "play": pending.play, "wake_id": pending.wake_id, "disposition": "awaiting-terminal",
        }));
        serde_json::to_vec(
            &serde_json::json!({"schema":"conduit.browser/body-face-state@1",
                "pending_lull": pending, "show_acknowledged":self.body_face_show.is_some(),
                "body_planning":self.snapshot.body_planning,
            }),
        )
        .map_err(|_| ServerError::InvalidRequest)
    }
    pub(super) fn deliver_body_face(
        &mut self,
        stream: &mut TcpStream,
        bytes: &[u8],
        operation: &str,
    ) -> Result<(), ServerError> {
        let result = match operation {
            "publication" => self.body_face_publication(),
            "state" => self.body_face_state(),
            "acknowledge" => self.acknowledge_body_face(bytes),
            "interact" => self.interact_body_face(bytes),
            _ => Err(ServerError::InvalidRequest),
        };
        match result {
            Ok(bytes) => {
                super::write_response(stream, "200 OK", "application/json; charset=utf-8", &bytes)
            }
            Err(ServerError::Interaction(reason)) => super::write_response(
                stream,
                "409 Conflict",
                "text/plain; charset=utf-8",
                reason.as_bytes(),
            ),
            Err(ServerError::InvalidRequest) => super::write_response(
                stream,
                "400 Bad Request",
                "text/plain; charset=utf-8",
                b"invalid Body Face request",
            ),
            Err(error) => Err(error),
        }
    }
}
