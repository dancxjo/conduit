//! Owner Face and typed action through the ordinary checked browser Mask Plot.
//!
//! Membership and Face authority stay on the Linux owner. This browser Host
//! plays the owner-selected DOM Mask Plan. The owner validates semantic
//! interactions and the acknowledged Show on the authenticated carrier.

use crate::workspace_mask::execution;
use conduit_body::BodyId;
use conduit_core::{bind_active_play, BootId, HostId, PortDirection, SignId, ValueConstraint};
use conduit_face::{
    Face, FaceInteraction, FaceInteractionArgument, ManifestationLifecycle, MaskShow,
    OwnerFaceSnapshotResponse, RemoteOwnerMaskRouteSeal, OWNER_FACE_RESPONSE_SCHEMA,
};
use conduit_kernel::scheduler::{RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use serde::{Deserialize, Serialize};

#[path = "owner_face_mask/abi.rs"]
mod abi;
#[path = "owner_face_mask/component.rs"]
mod component;
#[path = "owner_face_mask/interaction.rs"]
mod interaction;
#[path = "owner_face_mask/preparation.rs"]
mod preparation;
#[path = "owner_face_mask/view.rs"]
pub(crate) mod view;

use crate::installed_browser::dom_mask::MASK_BYTES;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HostBasis {
    body_id: BodyId,
    host_id: HostId,
    boot_id: BootId,
    #[serde(default)]
    visual_constraints: Option<component::VisualConstraints>,
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
    unmodeled_property_count: usize,
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
pub(crate) struct FaceView {
    schema: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    component: Option<component::ComponentView>,
    body_id: String,
    face_id: String,
    face_revision: String,
    mask_plot_id: String,
    mask_expanded_plot_id: String,
    mask_plan_id: String,
    mask_play_id: String,
    route: Option<RouteView>,
    show_id: String,
    show_state: &'static str,
    show_failure: Option<conduit_face::ManifestationFailure>,
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
    component: Option<component::ComponentView>,
    body_id: BodyId,
    face: Face,
    planned: conduit_face::PlannedMaskPlot,
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
    fn acknowledge(&mut self, ack: Acknowledgement) -> Result<(), String> {
        if ack.show_id != self.show.show_id.as_str()
            || ack.face_id != self.face.identity.as_str()
            || ack.face_revision != self.face.revision.to_string()
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
                    self.face.revision
                )),
            )
            .map_err(|error| format!("activate owner Face Show: {error:?}"))?;
        self.show
            .validate(&self.face)
            .map_err(|error| format!("validate owner Face Show: {error:?}"))
    }
}

#[cfg(test)]
#[path = "owner_face_mask/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "owner_face_mask/open_domain_tests.rs"]
mod open_domain_tests;

#[cfg(test)]
#[path = "owner_face_mask/patchbay_tests.rs"]
mod patchbay_tests;

#[cfg(test)]
#[path = "owner_face_mask/lifecycle_corpus_tests.rs"]
mod lifecycle_corpus_tests;

#[cfg(test)]
#[path = "owner_face_mask/committed_todo_tests.rs"]
mod committed_todo_tests;

#[cfg(test)]
#[path = "owner_face_mask/prebirth_tests.rs"]
mod prebirth_tests;

#[cfg(test)]
#[path = "owner_face_mask/parsec_agency_tests.rs"]
mod parsec_agency_tests;
