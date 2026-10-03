//! Read-only owner Face through the ordinary checked browser Mask Plot.
//!
//! Membership and Face authority stay on the Linux owner. This browser Host
//! plans and plays only its local DOM Mask; no Body Plan or interaction return
//! route is inferred from a Face snapshot.

use crate::workspace_mask::{execution, plan};
use conduit_body::BodyId;
use conduit_core::{bind_active_play, BootId, HostId, PortDirection, SignId};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_presentation::{
    ManifestationLifecycle, MaskShow, OwnerFaceSnapshotResponse, Presentation,
    OWNER_FACE_RESPONSE_SCHEMA,
};
use serde::{Deserialize, Serialize};

#[path = "owner_face_mask/abi.rs"]
mod abi;
#[path = "owner_face_mask/view.rs"]
mod view;

const MASK_BYTES: u32 = 512 * 1024;

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
    name: String,
    intent: String,
    target: String,
    availability: String,
    explanation: Option<String>,
    reason_code: Option<String>,
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
    pending_node: conduit_kernel::NodeId,
    pending_request: conduit_kernel::RequestId,
}

impl OwnerBrowserMask {
    fn prepare(
        basis: HostBasis,
        presentation: Presentation,
        play_sequence: u64,
    ) -> Result<Self, String> {
        presentation
            .validate()
            .map_err(|error| format!("owner Face invalid: {error:?}"))?;
        if presentation.basis.body_id.as_ref() != Some(&basis.body_id) {
            return Err("owner Face belongs to another Body".into());
        }
        let planned = plan::planned_mask(
            basis.host_id,
            basis.boot_id,
            plan::MASK_SOURCE,
            "browser-graphical",
        )?;
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
            pending_node: pending.node,
            pending_request: pending.request,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::Body;
    use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};
    use conduit_presentation::{Face, FaceContext, FaceFocus};

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
        let mut mask = OwnerBrowserMask::prepare(basis, face, 1).unwrap();
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
}
