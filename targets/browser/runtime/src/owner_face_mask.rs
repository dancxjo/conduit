//! Owner Face and typed action through the ordinary checked browser Mask Plot.
//!
//! Membership and Face authority stay on the Linux owner. This browser Host
//! plans and plays only its local DOM Mask. The owner separately admits and
//! validates a semantic interaction return on the authenticated carrier.

use crate::workspace_mask::{execution, plan};
use conduit_body::BodyId;
use conduit_core::{bind_active_play, BootId, HostId, PortDirection, SignId, ValueConstraint};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_presentation::{
    FaceInteraction, FaceInteractionArgument, ManifestationLifecycle, MaskShow,
    OwnerFaceSnapshotResponse, Presentation, OWNER_FACE_RESPONSE_SCHEMA,
};
use serde::{Deserialize, Serialize};

#[path = "owner_face_mask/abi.rs"]
mod abi;
#[path = "owner_face_mask/view.rs"]
mod view;

use crate::installed_browser::dom_mask::MASK_BYTES;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostBasis {
    body_id: BodyId,
    host_id: HostId,
    boot_id: BootId,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnerFrame {
    kind: String,
    protocol: u16,
    response: OwnerFaceSnapshotResponse,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Acknowledgement {
    show_id: String,
    face_id: String,
    face_revision: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedInteraction {
    show_id: String,
    face_id: String,
    face_revision: String,
    action_id: String,
    target: String,
    interval_ms: String,
    sequence: u64,
}

#[derive(Serialize)]
struct InteractionEmission<'a> {
    show: &'a MaskShow,
    interaction: FaceInteraction,
}

#[derive(Serialize)]
struct SubjectView {
    identity: String,
    name: String,
    role: String,
    text: Vec<String>,
    properties: Vec<String>,
}

#[derive(Serialize)]
struct RelationshipView {
    source: String,
    target: String,
    kind: String,
}

#[derive(Serialize)]
struct ActionView {
    identity: String,
    name: String,
    intent: String,
    target: String,
    availability: String,
    explanation: Option<String>,
    reason_code: Option<String>,
    arguments: Vec<ArgumentView>,
    current_value: Option<String>,
}

#[derive(Serialize)]
struct ArgumentView {
    name: String,
    value_name: String,
    choices: Vec<String>,
}

#[derive(Serialize)]
struct FaceView {
    schema: &'static str,
    body_id: String,
    face_id: String,
    face_revision: String,
    mask_plot_id: String,
    mask_plan_id: String,
    mask_play_id: String,
    show_id: String,
    show_state: &'static str,
    interactions_admitted: bool,
    subjects: Vec<SubjectView>,
    relationships: Vec<RelationshipView>,
    actions: Vec<ActionView>,
}

struct OwnerBrowserMask {
    body_id: BodyId,
    presentation: Presentation,
    planned: conduit_presentation::PlannedMaskPlot,
    play: conduit_core::ActivePlayIdentity,
    show: MaskShow,
    scheduler: execution::MaskScheduler,
    show_boundary: conduit_plan_lowering::lowering::LoweredForePort,
    interaction_boundary: conduit_plan_lowering::lowering::LoweredForePort,
    pending_node: conduit_kernel::NodeId,
    pending_request: conduit_kernel::RequestId,
    pending_interaction: Option<(conduit_kernel::NodeId, conduit_kernel::RequestId)>,
    interactions_admitted: bool,
}

impl OwnerBrowserMask {
    fn prepare(
        basis: HostBasis,
        presentation: Presentation,
        play_sequence: u64,
        interactions_admitted: bool,
    ) -> Result<Self, String> {
        presentation
            .validate()
            .map_err(|error| format!("owner Face invalid: {error:?}"))?;
        if presentation.basis.body_id.as_ref() != Some(&basis.body_id) {
            return Err("owner Face belongs to another Body".into());
        }
        let host = crate::installed_browser::membership_advertisement(basis.host_id, basis.boot_id);
        let planned = plan::planned_mask(&host, plan::MASK_SOURCE, "browser-graphical")?;
        let terminal = planned.show_placement();
        let play = bind_active_play(
            &planned.plan.plan_id,
            &terminal.host_id,
            &terminal.boot_id,
            play_sequence,
        );
        let face_subject = presentation
            .subjects
            .first()
            .ok_or("owner Face has no semantic subject")?
            .identity
            .clone();
        let show = MaskShow::prepared(
            &planned,
            &presentation,
            play.clone(),
            face_subject,
            "browser/document".into(),
            SignId::from(format!(
                "sign/browser-owner-face-prepared/{}",
                presentation.revision
            )),
        )
        .map_err(|error| format!("prepare owner Face Show: {error:?}"))?;
        let fragment = planned
            .plan
            .fragments
            .first()
            .ok_or("browser Mask Plan has no fragment")?;
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| format!("lower owner Face Mask: {error:?}"))?;
        let face_boundary =
            execution::exact_boundary(&lowered.fore_ports, "face", PortDirection::Input)?;
        let show_boundary =
            execution::exact_boundary(&lowered.fore_ports, "show", PortDirection::Output)?.clone();
        let interaction_boundary =
            execution::exact_boundary(&lowered.fore_ports, "interaction", PortDirection::Output)?
                .clone();
        let mut scheduler = execution::mask_scheduler(fragment, &lowered)?;
        let bytes = serde_json::to_vec(&presentation).map_err(|error| error.to_string())?;
        if bytes.len() > face_boundary.byte_capacity as usize {
            return Err("owner Face exceeds the browser Mask Fore bound".into());
        }
        match scheduler
            .admit_remote_input(face_boundary.endpoint, face_boundary.cord, 0, &bytes)
            .map_err(|error| format!("admit owner Face to Mask: {error:?}"))?
        {
            RemoteIngressOutcome::Accepted { sequence: 0 } => {}
            _ => return Err("owner Face Mask did not admit the exact Face".into()),
        }
        scheduler
            .close_remote_input(face_boundary.endpoint, face_boundary.cord)
            .map_err(|error| format!("close owner Face input: {error:?}"))?;
        let pending = loop {
            if let Some(request) = scheduler.next_host_request() {
                break request;
            }
            match scheduler
                .step()
                .map_err(|error| format!("start owner Face Mask: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => {
                    return Err(format!(
                        "owner Face Mask did not request DOM presentation: {other:?}"
                    ))
                }
            }
        };
        let presented = scheduler
            .host_value(pending.input.value)
            .map_err(|error| format!("read owner Face Mask input: {error:?}"))?;
        if presented != bytes.as_slice() {
            return Err("browser Mask received a different owner Face".into());
        }
        Ok(Self {
            body_id: basis.body_id,
            presentation,
            planned,
            play,
            show,
            scheduler,
            show_boundary,
            interaction_boundary,
            pending_node: pending.node,
            pending_request: pending.request,
            pending_interaction: None,
            interactions_admitted,
        })
    }

    fn acknowledge(&mut self, ack: Acknowledgement) -> Result<(), String> {
        if ack.show_id != self.show.show_id.as_str()
            || ack.face_id != self.presentation.identity.as_str()
            || ack.face_revision != self.presentation.revision.to_string()
            || self.show.show.lifecycle != ManifestationLifecycle::Prepared
        {
            return Err("owner Face Show acknowledgement is stale or mismatched".into());
        }
        let show_bytes = serde_json::to_vec(&self.show.show).map_err(|error| error.to_string())?;
        let output = self
            .scheduler
            .store_host_value(&show_bytes)
            .map_err(|error| format!("store owner Face Show: {error:?}"))?;
        self.scheduler
            .complete_host_call(
                self.pending_node,
                self.pending_request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(output, MASK_BYTES)
                            .map_err(|_| "owner Face Show bound")?,
                    ),
                    failure: None,
                },
            )
            .map_err(|error| format!("complete browser DOM Mask Host Call: {error:?}"))?;
        let offer = loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.show_boundary.endpoint, self.show_boundary.cord)
                .map_err(|error| format!("offer owner Face Show: {error:?}"))?
            {
                break offer;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("advance owner Face Mask: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => return Err(format!("owner Face Mask ended without Show: {other:?}")),
            }
        };
        let observed = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read owner Face Show: {error:?}"))?;
        if observed != show_bytes.as_slice() {
            return Err("browser Mask emitted a different Show".into());
        }
        self.scheduler
            .remote_egress_accept(
                self.show_boundary.endpoint,
                self.show_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("accept owner Face Show: {error:?}"))?;
        self.scheduler
            .remote_egress_delivered(
                self.show_boundary.endpoint,
                self.show_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("deliver owner Face Show: {error:?}"))?;
        let pending = self
            .scheduler
            .next_host_request()
            .ok_or("browser Mask did not suspend for its read-only interaction boundary")?;
        if pending.request != conduit_kernel::RequestId(1) {
            return Err("browser Mask exposed an unexpected request after Show".into());
        }
        self.pending_interaction = Some((pending.node, pending.request));
        self.show = self
            .show
            .transition(
                ManifestationLifecycle::Available,
                SignId::from(format!(
                    "sign/browser-owner-face-shown/{}",
                    self.presentation.revision
                )),
            )
            .map_err(|error| format!("activate owner Face Show: {error:?}"))?;
        self.show
            .validate(&self.presentation)
            .map_err(|error| format!("validate owner Face Show: {error:?}"))
    }

    fn interact(
        &mut self,
        proposed: ProposedInteraction,
    ) -> Result<InteractionEmission<'_>, String> {
        if !self.interactions_admitted
            || self.show.show.lifecycle != ManifestationLifecycle::Available
        {
            return Err("owner interaction return is not admitted for this Show".into());
        }
        if proposed.show_id != self.show.show_id.as_str()
            || proposed.face_id != self.presentation.identity.as_str()
            || proposed.face_revision != self.presentation.revision.to_string()
        {
            return Err("owner browser interaction has a stale Face or Show".into());
        }
        let action = self
            .presentation
            .resolve_action(self.presentation.revision, &proposed.action_id)
            .map_err(|error| format!("owner browser action refused: {error:?}"))?;
        if action.target != proposed.target || action.arguments.len() != 1 {
            return Err(
                "owner browser interaction has a different target or argument contract".into(),
            );
        }
        let declaration = &action.arguments[0];
        let interaction = FaceInteraction::new(
            &self.presentation,
            &self.show,
            &proposed.action_id,
            &proposed.target,
            vec![FaceInteractionArgument {
                name: declaration.name.clone(),
                value_kind: declaration.contract.value_kind.as_str().into(),
                value: proposed.interval_ms.into_bytes(),
            }],
            proposed.sequence,
        )
        .map_err(|error| format!("owner browser interaction refused: {error:?}"))?;
        let bytes = interaction.encode();
        if bytes.len() > self.interaction_boundary.byte_capacity as usize {
            return Err("owner browser interaction exceeds its sealed Fore bound".into());
        }
        let value = self
            .scheduler
            .store_host_value(&bytes)
            .map_err(|error| format!("store owner browser interaction: {error:?}"))?;
        let (node, request) = self
            .pending_interaction
            .take()
            .ok_or("owner browser Mask interaction Fore is terminal")?;
        self.scheduler
            .complete_host_call(
                node,
                request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, MASK_BYTES).map_err(|_| "interaction bound")?,
                    ),
                    failure: None,
                },
            )
            .map_err(|error| format!("complete owner browser interaction: {error:?}"))?;
        let offer = loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(
                    self.interaction_boundary.endpoint,
                    self.interaction_boundary.cord,
                )
                .map_err(|error| format!("offer owner browser interaction: {error:?}"))?
            {
                break offer;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("advance owner browser Mask: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => {
                    return Err(format!(
                        "owner browser Mask ended without interaction: {other:?}"
                    ))
                }
            }
        };
        if self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read interaction: {error:?}"))?
            != bytes.as_slice()
        {
            return Err("owner browser Mask emitted a different interaction".into());
        }
        self.scheduler
            .remote_egress_accept(
                self.interaction_boundary.endpoint,
                self.interaction_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("accept interaction: {error:?}"))?;
        self.scheduler
            .remote_egress_delivered(
                self.interaction_boundary.endpoint,
                self.interaction_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("deliver interaction: {error:?}"))?;
        Ok(InteractionEmission {
            show: &self.show,
            interaction,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::Body;
    use conduit_core::{
        kind_id, CheckedPlotId, CheckedValueContract, SignId, SourceDocumentId, ValueConstraint,
    };
    use conduit_presentation::{
        Face, FaceActionArgument, FaceContext, FaceFocus, PresentationAction,
        PresentationActionAvailability, PresentationDisclosureLevel, UTF8_TEXT_VALUE_KIND,
    };

    #[test]
    fn exact_owner_face_runs_the_browser_mask_and_acknowledges_its_show() {
        let body = Body::born(
            SourceDocumentId::from("source/owner-face-mask"),
            CheckedPlotId::from("checked/owner-face-mask"),
            1,
            SignId::from("sign/born-owner-face-mask"),
        )
        .unwrap();
        let revision = 9_007_199_254_740_993;
        let face = Face::project(
            &body,
            None,
            revision,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
        )
        .unwrap()
        .presentation;
        let basis = HostBasis {
            body_id: body.body_id.clone(),
            host_id: HostId::from("host/browser-owner-face"),
            boot_id: BootId::from("boot/browser-owner-face"),
        };
        let mut mask = OwnerBrowserMask::prepare(basis, face, 1, false).unwrap();
        let prepared = mask.view();
        assert_eq!(prepared.face_revision, revision.to_string());
        assert_eq!(prepared.show_state, "prepared");
        assert!(!prepared.interactions_admitted);
        assert!(mask
            .acknowledge(Acknowledgement {
                show_id: prepared.show_id.clone(),
                face_id: prepared.face_id.clone(),
                face_revision: prepared.face_revision.clone(),
            })
            .is_ok());
        assert_eq!(mask.view().show_state, "available");
        assert!(mask
            .acknowledge(Acknowledgement {
                show_id: prepared.show_id,
                face_id: prepared.face_id,
                face_revision: prepared.face_revision,
            })
            .is_err());
    }

    #[test]
    fn current_browser_mask_emits_one_exact_typed_clock_interaction() {
        let body = Body::born(
            SourceDocumentId::from("source/owner-browser-action"),
            CheckedPlotId::from("checked/owner-browser-action"),
            1,
            SignId::from("sign/born-owner-browser-action"),
        )
        .unwrap();
        let revision = 9_007_199_254_740_993;
        let face = Face::project(
            &body,
            None,
            revision,
            FaceContext::Overview,
            FaceFocus::Body,
            vec![],
        )
        .unwrap()
        .presentation;
        let target = face.subjects[0].identity.clone();
        let action = PresentationAction {
            identity: "body/action/change-clock-interval/1".into(),
            intent: "conduit.intent/change-clock-interval@1".into(),
            target: target.clone(),
            name: "Change clock interval".into(),
            arguments: vec![FaceActionArgument {
                name: "clock/interval-ms".into(),
                value_name: "Clock interval".into(),
                contract: CheckedValueContract::new(
                    kind_id(UTF8_TEXT_VALUE_KIND),
                    4,
                    vec![ValueConstraint::CanonicalMembership {
                        members: vec![b"250".to_vec(), b"500".to_vec()],
                        negated: false,
                    }],
                )
                .unwrap(),
            }],
            disclosure: PresentationDisclosureLevel::CurrentAction,
            availability: PresentationActionAvailability::Available,
        };
        let face = Presentation::new_with_semantics_and_temporal(
            face.revision,
            face.basis,
            face.subjects,
            face.relationships,
            face.properties,
            face.text,
            vec![action.clone()],
            face.disclosures,
            face.temporal_references,
            face.temporal_facts,
        )
        .unwrap();
        let basis = HostBasis {
            body_id: body.body_id,
            host_id: HostId::from("host/browser-owner-action"),
            boot_id: BootId::from("boot/browser-owner-action"),
        };
        let mut mask = OwnerBrowserMask::prepare(basis, face, 1, true).unwrap();
        let prepared = mask.view();
        assert_eq!(prepared.actions[0].arguments[0].choices, ["250", "500"]);
        mask.acknowledge(Acknowledgement {
            show_id: prepared.show_id.clone(),
            face_id: prepared.face_id.clone(),
            face_revision: prepared.face_revision.clone(),
        })
        .unwrap();
        let proposed = |revision: &str, interval: &str| ProposedInteraction {
            show_id: prepared.show_id.clone(),
            face_id: prepared.face_id.clone(),
            face_revision: revision.into(),
            action_id: action.identity.clone(),
            target: target.clone(),
            interval_ms: interval.into(),
            sequence: 1,
        };
        assert!(mask.interact(proposed("9007199254740992", "500")).is_err());
        assert!(mask
            .interact(proposed(&prepared.face_revision, "3000"))
            .is_err());
        let emitted = mask
            .interact(proposed(&prepared.face_revision, "500"))
            .unwrap();
        assert_eq!(emitted.interaction.face_revision, revision);
        assert_eq!(emitted.interaction.arguments[0].value, b"500");
        assert_eq!(emitted.show.show_id.as_str(), prepared.show_id);
        assert!(mask
            .interact(proposed(&prepared.face_revision, "250"))
            .is_err());
    }
}
