//! Owner Face and typed action through the ordinary checked browser Mask Plot.
//!
//! Membership and Face authority stay on the Linux owner. This browser Host
//! plays the owner-selected DOM Mask Plan. The owner validates semantic
//! interactions and the acknowledged Show on the authenticated carrier.

use crate::workspace_mask::execution;
use conduit_body::BodyId;
use conduit_core::{bind_active_play, BootId, HostId, PortDirection, SignId, ValueConstraint};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_presentation::{
    FaceInteraction, FaceInteractionArgument, ManifestationLifecycle, MaskShow,
    OwnerFaceSnapshotResponse, Presentation, RemoteOwnerMaskRouteSeal, OWNER_FACE_RESPONSE_SCHEMA,
};
use serde::{Deserialize, Serialize};

#[path = "owner_face_mask/abi.rs"]
mod abi;
#[path = "owner_face_mask/interaction.rs"]
mod interaction;
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
    arguments: Vec<ProposedArgument>,
    sequence: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposedArgument {
    name: String,
    value: String,
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
    semantic_role: Option<String>,
    choice_multiplicity: Option<String>,
    values: Vec<ValueView>,
    disclosure: String,
    text: Vec<String>,
    properties: Vec<String>,
    flags: Vec<FlagView>,
}

#[derive(Serialize)]
struct ValueView {
    name: String,
    kind: String,
    text: String,
}

#[derive(Serialize)]
struct FlagView {
    name: String,
    value: bool,
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
    disclosure: String,
    availability: String,
    explanation: Option<String>,
    reason_code: Option<String>,
    arguments: Vec<ArgumentView>,
}

#[derive(Serialize)]
struct ArgumentView {
    name: String,
    value_name: String,
    value_kind: String,
    maximum_bytes: u32,
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
    route: Option<RouteView>,
    show_id: String,
    show_state: &'static str,
    interactions_admitted: bool,
    subjects: Vec<SubjectView>,
    relationships: Vec<RelationshipView>,
    actions: Vec<ActionView>,
}

#[derive(Serialize)]
struct RouteView {
    plan_id: String,
    owner_host_id: String,
    owner_boot_id: String,
    mask_host_id: String,
    mask_boot_id: String,
}

struct OwnerBrowserMask {
    body_id: BodyId,
    presentation: Presentation,
    planned: conduit_presentation::PlannedMaskPlot,
    play: conduit_core::ActivePlayIdentity,
    route: Option<RouteView>,
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
        route: RemoteOwnerMaskRouteSeal,
        play_sequence: u64,
        interactions_admitted: bool,
    ) -> Result<Self, String> {
        presentation
            .validate()
            .map_err(|error| format!("owner Face invalid: {error:?}"))?;
        if presentation.basis.body_id.as_ref() != Some(&basis.body_id) {
            return Err("owner Face belongs to another Body".into());
        }
        if route.body_id != basis.body_id
            || route.face_id != presentation.identity
            || route.face_revision != presentation.revision
        {
            return Err("owner route differs from the current Face".into());
        }
        let host = crate::installed_browser::membership_advertisement(
            basis.host_id.clone(),
            basis.boot_id.clone(),
        );
        route
            .validate_mask_host_offer(&host)
            .map_err(|error| format!("owner Mask route differs from this browser: {error:?}"))?;
        let inspection = RouteView {
            plan_id: route.route_plan_id.as_str().into(),
            owner_host_id: route.owner_host.host_id.as_str().into(),
            owner_boot_id: route.owner_host.boot_id.as_str().into(),
            mask_host_id: route.mask_host.host_id.as_str().into(),
            mask_boot_id: route.mask_host.boot_id.as_str().into(),
        };
        let selected_lines = (
            route.face_line.clone(),
            route.return_line.clone(),
            route.interaction_line.clone(),
        );
        Self::prepare_planned(
            basis,
            presentation,
            route.planned_mask,
            Some(inspection),
            Some(selected_lines),
            play_sequence,
            interactions_admitted,
        )
    }

    /// Component-only proof of the local Mask kernel and ABI, without an
    /// owner-issued route or a live browser carrier.
    #[cfg(test)]
    fn prepare_component_fixture(
        basis: HostBasis,
        presentation: Presentation,
        play_sequence: u64,
        interactions_admitted: bool,
    ) -> Result<Self, String> {
        let host = crate::installed_browser::membership_advertisement(
            basis.host_id.clone(),
            basis.boot_id.clone(),
        );
        let planned = crate::workspace_mask::plan::planned_mask(
            &host,
            crate::workspace_mask::plan::MASK_SOURCE,
            "browser-graphical",
        )?;
        Self::prepare_planned(
            basis,
            presentation,
            planned,
            None,
            None,
            play_sequence,
            interactions_admitted,
        )
    }

    fn prepare_planned(
        basis: HostBasis,
        presentation: Presentation,
        planned: conduit_presentation::PlannedMaskPlot,
        route: Option<RouteView>,
        selected_lines: Option<(
            conduit_core::AdmittedLine,
            conduit_core::AdmittedLine,
            Option<conduit_core::AdmittedLine>,
        )>,
        play_sequence: u64,
        interactions_admitted: bool,
    ) -> Result<Self, String> {
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
        if let Some((face_line, return_line, interaction_line)) = selected_lines {
            let selected = face_boundary.selected_line.is_some()
                || show_boundary.selected_line.is_some()
                || interaction_boundary.selected_line.is_some();
            if selected
                && (face_boundary.selected_line.as_ref() != Some(&face_line)
                    || show_boundary.selected_line.as_ref() != Some(&return_line)
                    || interaction_boundary.selected_line != interaction_line)
            {
                return Err("browser Mask lowered a different selected carrier Line".into());
            }
        }
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
            route,
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
}

#[cfg(test)]
#[path = "owner_face_mask/tests.rs"]
mod tests;
