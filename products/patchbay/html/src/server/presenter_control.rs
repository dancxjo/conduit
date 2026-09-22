//! Typed Presenter-topology requests through the ordinary body planning owner.

use super::{PatchbayHtmlServer, ServerError};
use conduit_core::{PlanId, SignId};
use patchbay_model::{BodyPlanningTransition, PresenterTopologyMode};
use serde::Deserialize;
use std::net::TcpStream;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    presentation_id: String,
    presentation_revision: u64,
    basis_plan_id: PlanId,
    mode: PresenterTopologyModeWire,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum PresenterTopologyModeWire {
    Graphical,
    GraphicalAndSpeech,
    Speech,
}

impl PatchbayHtmlServer {
    pub(super) fn deliver_presenter_topology(
        &mut self,
        stream: &mut TcpStream,
        body: &[u8],
    ) -> Result<(), ServerError> {
        let response = self.apply_presenter_topology(body)?;
        super::write_response(
            stream,
            "200 OK",
            "application/json; charset=utf-8",
            &response,
        )
    }

    fn apply_presenter_topology(&mut self, body: &[u8]) -> Result<Vec<u8>, ServerError> {
        let request: Request =
            serde_json::from_slice(body).map_err(|_| ServerError::InvalidRequest)?;
        if request.presentation_id != self.snapshot.presentation.identity.as_str()
            || request.presentation_revision != self.snapshot.presentation.revision
        {
            return Err(ServerError::Interaction("StaleRequest".into()));
        }
        let sequence = self
            .snapshot
            .interaction
            .revision
            .checked_add(1)
            .ok_or_else(|| ServerError::Interaction("PresenterRevisionExhausted".into()))?;
        let mode = match request.mode {
            PresenterTopologyModeWire::Graphical => PresenterTopologyMode::Graphical,
            PresenterTopologyModeWire::GraphicalAndSpeech => {
                PresenterTopologyMode::GraphicalAndSpeech
            }
            PresenterTopologyModeWire::Speech => PresenterTopologyMode::Speech,
        };
        let mut control = self
            .presenter_control
            .clone()
            .ok_or_else(|| ServerError::Interaction("PresenterControlUnavailable".into()))?;
        let mut planning = self
            .body_planning
            .clone()
            .ok_or_else(|| ServerError::Interaction("BodyPlanningUnavailable".into()))?;
        control
            .request_mode(
                &request.basis_plan_id,
                mode,
                &mut planning,
                BodyPlanningTransition {
                    unsatisfied_sign_id: Some(SignId::from(format!(
                        "patchbay-html/presenter/{sequence}/unsatisfied"
                    ))),
                    plan_ready_sign_id: SignId::from(format!(
                        "patchbay-html/presenter/{sequence}/plan"
                    )),
                    play_sequence: sequence,
                    play_started_sign_id: SignId::from(format!(
                        "patchbay-html/presenter/{sequence}/play"
                    )),
                },
            )
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        let session = self
            .body_workload
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("BodyWorkloadAbsent".into()))?;
        let (session, mut snapshot) =
            super::body_execution::history::retain(&self.snapshot, session, &planning)?;
        snapshot.body_planning = Some(planning.snapshot());
        control
            .refresh_presentation(snapshot.presentation.clone())
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        let play = conduit_body::BodyPlayIdentity::bind(planning.current_plan(), sequence);
        let topology = control
            .project_current(&planning, &play)
            .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
        snapshot.presenter_topology = Some(
            patchbay_model::project_presenter_topology(control.presentation(), &topology)
                .map_err(|error| ServerError::Interaction(format!("{error:?}")))?,
        );
        snapshot.interaction.revision = sequence;
        snapshot.interaction.last_request_id = Some("presenter-topology".into());
        snapshot.interaction.last_disposition = Some("Succeeded(BodyReplanned)".into());
        self.encoded_snapshot = snapshot.encode()?;
        self.body_workload = Some(session);
        self.body_planning = Some(planning);
        self.presenter_control = Some(control);
        self.snapshot = snapshot;
        Ok(self.encoded_snapshot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::body_execution_proposal::tests::proposed_server;
    use conduit_core::bind_sign;
    use serde_json::{json, Value};

    fn execution(action: Value) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema": "conduit.patchbay/body-execution-request@1",
            "action": action,
        }))
        .unwrap()
    }

    #[test]
    fn browser_visible_control_replans_parallel_speech_and_restored_graphics() {
        let mut server = proposed_server();
        let plan = server.body_planning.as_ref().unwrap().current_plan();
        let fragment = &plan.forms[0].plan.fragments[0];
        server
            .apply_body_execution(&execution(json!({
                "kind": "Claim", "plan_id": plan.plan_id,
                "host_id": fragment.host_id, "boot_id": fragment.boot_id,
            })))
            .unwrap();
        let claim = server
            .body_planning
            .as_ref()
            .unwrap()
            .snapshot()
            .execution_claims[0]
            .clone();
        let sign = |sequence| {
            bind_sign(
                &claim.host_id,
                &claim.boot_id,
                Some(&claim.play.active_play_id),
                sequence,
            )
            .sign_id
        };
        let planning = server.body_planning.as_ref().unwrap();
        let wake = planning
            .wake()
            .body_plan_ready(planning.current_plan(), sign(0))
            .unwrap()
            .body_play_started(planning.current_plan(), &claim.play, sign(1))
            .unwrap();
        server
            .apply_body_execution(&execution(json!({
                "kind": "Started", "play": claim.play, "wake_at_start": wake,
            })))
            .unwrap();
        server
            .apply_body_execution(&execution(json!({
                "kind": "Terminal", "play": claim.play,
                "disposition": "completed", "terminal_sign_id": sign(2),
            })))
            .unwrap();
        let source_forms = server
            .body_planning
            .as_ref()
            .unwrap()
            .current_plan()
            .forms
            .clone();
        assert!(server.snapshot.presenter_topology.is_some());

        let invoke = |server: &mut PatchbayHtmlServer, mode: &str| {
            let request = json!({
                "presentation_id": server.snapshot.presentation.identity,
                "presentation_revision": server.snapshot.presentation.revision,
                "basis_plan_id": server.body_planning.as_ref().unwrap().current_plan().plan_id,
                "mode": mode,
            });
            server
                .apply_presenter_topology(&serde_json::to_vec(&request).unwrap())
                .unwrap();
            server.snapshot.presenter_topology.clone().unwrap()
        };
        let parallel = invoke(&mut server, "graphical-and-speech");
        assert_eq!(chains(&parallel), 2);
        let parallel_plan = server
            .body_planning
            .as_ref()
            .unwrap()
            .current_plan()
            .plan_id
            .clone();
        let speech = invoke(&mut server, "speech");
        assert_eq!(chains(&speech), 1);
        assert_ne!(
            server
                .body_planning
                .as_ref()
                .unwrap()
                .current_plan()
                .plan_id,
            parallel_plan
        );
        let restored = invoke(&mut server, "graphical-and-speech");
        assert_eq!(chains(&restored), 2);
        assert_eq!(
            server.body_planning.as_ref().unwrap().current_plan().forms,
            source_forms
        );
        for intent in ["add", "remove", "replace", "reorder", "toggle-parallel"] {
            assert!(restored
                .actions
                .iter()
                .any(|action| action.intent.contains(intent)));
        }
    }

    fn chains(presentation: &conduit_presentation::Presentation) -> usize {
        presentation
            .subjects
            .iter()
            .filter(|subject| subject.role == conduit_presentation::PresentationRole::Manifestation)
            .count()
    }
}
