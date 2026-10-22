//! Browser realization of the tutorial Face through one ordinary Mask Plot.

use conduit_body::{BodyFaceSelector, BodyId, BodyMaskChainPlan, BodyMaskTopology, BodyPlan, Wake};
use conduit_core::{bind_active_play, ActivePlayIdentity, BootId, HostId, PortDirection, SignId};
use conduit_face::{
    AdmittedMaskPlotRoutes, BodyMaskWardrobe, Face, FaceAction, FaceInteraction,
    FaceInteractionArgument, ManifestationLifecycle, MaskInteractionCorrelation, MaskPlot,
    MaskShow, MaskWardrobe, MaskWardrobeAction, MaskWardrobeControl, MaskWardrobeControlEvidence,
    MaskWardrobeLifetime, PlannedMaskPlot, SealedMaskPlotRoute,
};
use conduit_kernel::scheduler::{
    FixedScheduler, SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, HostCallDisposition, HostCallId,
    HostCallOutcome, HostedSignLog, HostedValueStore, PortId as KernelPortId, RequestId, SignQuery,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredForePort, FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use serde::{Deserialize, Serialize};

use crate::installed_browser::dom_mask::MASK_BYTES;
#[path = "workspace_mask_execution.rs"]
pub(crate) mod execution;
#[path = "workspace_mask_interaction.rs"]
mod interaction;
#[path = "workspace_mask_journey.rs"]
mod journey;
#[path = "workspace_mask_plan.rs"]
pub(crate) mod plan;
#[path = "workspace_mask_preparation.rs"]
mod preparation;
pub use interaction::{BrowserMaskInteraction, BrowserMaskInteractionReceipt};
pub use journey::BrowserMaskJourneyOutcome;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserMaskEffect {
    pub schema: String,
    pub mask_plot: conduit_core::PlotIdentity,
    pub mask_plan_id: conduit_core::PlanId,
    pub mask_play: ActivePlayIdentity,
    pub show_id: String,
    pub manifestation_id: String,
    pub placement_id: conduit_core::PlacementId,
    pub face_id: String,
    pub face_revision: u64,
    pub text: Vec<conduit_face::FaceText>,
    pub actions: Vec<conduit_face::FaceAction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserMaskAcknowledgement {
    pub show_id: String,
    pub manifestation_id: String,
    pub mask_plan_id: conduit_core::PlanId,
    pub active_play_id: conduit_core::ActivePlayId,
    pub placement_id: conduit_core::PlacementId,
    pub face_id: String,
    pub face_revision: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskObservation {
    pub schema: &'static str,
    pub wardrobe_action: MaskWardrobeControlEvidence,
    pub planned_mask: PlannedMaskPlot,
    pub alternate: PlannedMaskPlot,
    pub body_plan: BodyPlan,
    pub mask_play: ActivePlayIdentity,
    pub face: Face,
    pub mask_show: MaskShow,
    pub execution: BrowserMaskExecutionReceipt,
    pub interaction: Option<BrowserMaskInteractionReceipt>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskExecutionReceipt {
    pub schema: &'static str,
    pub fore: Vec<BrowserMaskForeReceipt>,
    pub remote_signs: Vec<BrowserMaskRemoteSignReceipt>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskForeReceipt {
    pub front_port_id: String,
    pub direction: PortDirection,
    pub track: conduit_core::ConnectionTrack,
    pub value_kind: String,
    pub endpoint: u16,
    pub cord: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskRemoteSignReceipt {
    pub event_sequence: u32,
    pub kind: String,
    pub endpoint: u16,
    pub cord: u16,
    pub sequence: u64,
}

pub struct BrowserMaskRuntime {
    routes: AdmittedMaskPlotRoutes,
    wardrobe_action: MaskWardrobeControlEvidence,
    planned: PlannedMaskPlot,
    alternate: PlannedMaskPlot,
    body_plan: BodyPlan,
    basis_plan: BodyPlan,
    wake: Wake,
    play: ActivePlayIdentity,
    face: Face,
    show: MaskShow,
    scheduler: execution::MaskScheduler,
    show_boundary: LoweredForePort,
    interaction_boundary: LoweredForePort,
    pending_node: conduit_kernel::NodeId,
    pending_request: RequestId,
    acknowledgement_started: bool,
    fore_receipt: Vec<BrowserMaskForeReceipt>,
    pending_interaction_node: Option<conduit_kernel::NodeId>,
    pending_interaction_request: Option<RequestId>,
    interaction_receipt: Option<BrowserMaskInteractionReceipt>,
}

impl BrowserMaskRuntime {
    pub fn prepare(
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        face: Face,
        wake: Wake,
        base_plan: BodyPlan,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        Self::prepare_sequenced(body_id, host_id, boot_id, face, wake, base_plan, 1)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_sequenced(
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        face: Face,
        wake: Wake,
        base_plan: BodyPlan,
        play_sequence: u64,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        Self::prepare_route(
            body_id,
            host_id,
            boot_id,
            face,
            wake,
            base_plan,
            false,
            play_sequence,
        )
    }

    pub fn acknowledge(&mut self, ack: &BrowserMaskAcknowledgement) -> Result<(), String> {
        let exact = ack.show_id == self.show.show_id.as_str()
            && ack.manifestation_id == self.show.show.manifestation_id.as_str()
            && ack.mask_plan_id == self.planned.plan.plan_id
            && ack.active_play_id == self.play.active_play_id
            && ack.placement_id == self.show.show.placement_id
            && ack.face_id == self.face.identity.as_str()
            && ack.face_revision == self.face.revision;
        if !exact {
            return Err("browser Mask acknowledgement is stale or mismatched".into());
        }
        if self.acknowledgement_started {
            return Err("browser Mask acknowledgement has already started".into());
        }
        let show_bytes = serde_json::to_vec(&self.show.show).map_err(|error| error.to_string())?;
        // An uncertain/partial completion must not allocate another Show frame
        // on retry. Only mismatched evidence above leaves this entrance unused.
        self.acknowledgement_started = true;
        let output = self
            .scheduler
            .store_host_value(&show_bytes)
            .map_err(|error| format!("store browser Mask Show: {error:?}"))?;
        self.scheduler
            .complete_host_call(
                self.pending_node,
                self.pending_request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(output, MASK_BYTES)
                            .map_err(|_| "browser Mask Show bound")?,
                    ),
                    failure: None,
                },
            )
            .map_err(|error| format!("acknowledge browser DOM effect: {error:?}"))?;
        let offer = loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.show_boundary.endpoint, self.show_boundary.cord)
                .map_err(|error| format!("offer browser Mask Show: {error:?}"))?
            {
                break offer;
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("finish browser Mask Play: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => return Err(format!("browser Mask ended without its Show: {other:?}")),
            }
        };
        let observed = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read browser Mask Show: {error:?}"))?;
        if observed != show_bytes.as_slice() {
            return Err("browser Mask emitted a different Show".into());
        }
        self.scheduler
            .remote_egress_accept(
                self.show_boundary.endpoint,
                self.show_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("accept browser Mask Show: {error:?}"))?;
        self.scheduler
            .remote_egress_delivered(
                self.show_boundary.endpoint,
                self.show_boundary.cord,
                offer.sequence,
            )
            .map_err(|error| format!("deliver browser Mask Show: {error:?}"))?;
        let pending = self
            .scheduler
            .next_host_request()
            .ok_or("browser Mask did not suspend for DOM interaction")?;
        if pending.request != RequestId(1) {
            return Err(format!(
                "browser Mask exposed unexpected interaction request {:?}",
                pending.request
            ));
        }
        self.pending_interaction_node = Some(pending.node);
        self.pending_interaction_request = Some(pending.request);
        self.show = self
            .show
            .transition(
                ManifestationLifecycle::Available,
                SignId::from(format!(
                    "sign/browser-mask-available/{}",
                    self.face.revision
                )),
            )
            .map_err(|error| format!("{error:?}"))?;
        self.show
            .validate(&self.face)
            .map_err(|error| format!("{error:?}"))
    }

    /// Actual value backing reserved during preparation, including slot metadata.
    /// Sign/kernel tables and other Host allocations are separate reservations.
    pub fn reserved_value_storage_bytes(&self) -> usize {
        self.scheduler.values().reserved_storage_bytes()
    }

    pub(crate) fn face_view(&self, admitted: bool) -> crate::owner_face_mask::FaceView {
        crate::owner_face_mask::view::local_view_admitted(
            &self.body_plan.body_id,
            &self.face,
            &self.show,
            &self.play,
            admitted,
        )
    }

    pub fn observation(&self) -> BrowserMaskObservation {
        let _ = self.routes.plan_id();
        BrowserMaskObservation {
            schema: "conduit.browser/mask-observation@1",
            wardrobe_action: self.wardrobe_action.clone(),
            planned_mask: self.planned.clone(),
            alternate: self.alternate.clone(),
            body_plan: self.body_plan.clone(),
            mask_play: self.play.clone(),
            face: self.face.clone(),
            mask_show: self.show.clone(),
            execution: BrowserMaskExecutionReceipt {
                schema: "conduit.browser/mask-kernel-execution@1",
                fore: self.fore_receipt.clone(),
                remote_signs: self
                    .scheduler
                    .signs()
                    .events()
                    .filter_map(|event| {
                        self.scheduler
                            .signs()
                            .remote_identity(event.sequence)
                            .map(|remote| BrowserMaskRemoteSignReceipt {
                                event_sequence: event.sequence,
                                kind: format!("{:?}", event.kind),
                                endpoint: remote.endpoint.0,
                                cord: remote.cord.0,
                                sequence: remote.sequence,
                            })
                    })
                    .collect(),
            },
            interaction: self.interaction_receipt.clone(),
        }
    }

    pub fn actualize_journey(
        &self,
        observations: &[BrowserMaskObservation],
    ) -> Result<Vec<BrowserMaskJourneyOutcome>, String> {
        journey::actualize(observations)
    }

    pub fn alternate(&self, body_id: BodyId) -> Result<(Self, BrowserMaskEffect), String> {
        Self::prepare_route(
            body_id,
            self.play.host_id.clone(),
            self.play.boot_id.clone(),
            self.face.clone(),
            self.wake.clone(),
            self.basis_plan.clone(),
            true,
            1,
        )
    }

    pub fn replacement(&self, body_id: BodyId) -> Result<(Self, BrowserMaskEffect), String> {
        Self::prepare_route(
            body_id,
            self.play.host_id.clone(),
            BootId::from(format!("{}/mask-replacement", self.play.boot_id.as_str())),
            self.face.clone(),
            self.wake.clone(),
            self.body_plan.clone(),
            true,
            1,
        )
    }

    pub fn restored(&self, body_id: BodyId) -> Result<(Self, BrowserMaskEffect), String> {
        Self::prepare_route(
            body_id,
            self.play.host_id.clone(),
            self.play.boot_id.clone(),
            self.face.clone(),
            self.wake.clone(),
            self.basis_plan.clone(),
            false,
            1,
        )
    }
}

#[cfg(test)]
#[path = "workspace_mask_tests.rs"]
mod tests;
