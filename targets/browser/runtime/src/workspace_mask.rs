//! Browser realization of the tutorial Face through one ordinary Mask Plot.

use conduit_body::{BodyFaceSelector, BodyId, BodyMaskChainPlan, BodyMaskTopology, BodyPlan, Wake};
use conduit_core::{
    bind_active_play, kind_id, port_id, ActivePlayIdentity, ArtifactId, Back, BackOfferBuilder,
    BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement,
    HostCallContractId, HostCallRequirement, HostId, HostProfileId, ImplementationId, Kind,
    KindIdentity, OfferGeneration, PortDescriptor, PortDirection, PortTemporal, SignId,
    PROTOCOL_VERSION,
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
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    install_mask_plot_value_aliases, AdmittedMaskPlotRoutes, BodyMaskWardrobe, FaceInteraction,
    FaceInteractionArgument, ManifestationLifecycle, MaskInteractionCorrelation, MaskPlot,
    MaskShow, MaskWardrobe, MaskWardrobeAction, MaskWardrobeControl, MaskWardrobeControlEvidence,
    MaskWardrobeLifetime, PlannedMaskPlot, Presentation, PresentationAction, SealedMaskPlotRoute,
    FACE_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MASK_OPERATION: &str = "browser.host/dom-mask@1";
const MASK_BYTES: u32 = 512 * 1024;
#[path = "workspace_mask_execution.rs"]
pub(crate) mod execution;
#[path = "workspace_mask_interaction.rs"]
mod interaction;
#[path = "workspace_mask_journey.rs"]
mod journey;
#[path = "workspace_mask_plan.rs"]
pub(crate) mod plan;
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
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub text: Vec<conduit_presentation::PresentationText>,
    pub actions: Vec<conduit_presentation::PresentationAction>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserMaskAcknowledgement {
    pub show_id: String,
    pub manifestation_id: String,
    pub mask_plan_id: conduit_core::PlanId,
    pub active_play_id: conduit_core::ActivePlayId,
    pub placement_id: conduit_core::PlacementId,
    pub presentation_id: String,
    pub presentation_revision: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskObservation {
    pub schema: &'static str,
    pub wardrobe_action: MaskWardrobeControlEvidence,
    pub planned_mask: PlannedMaskPlot,
    pub alternate: PlannedMaskPlot,
    pub body_plan: BodyPlan,
    pub mask_play: ActivePlayIdentity,
    pub presentation: Presentation,
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
    presentation: Presentation,
    show: MaskShow,
    scheduler: execution::MaskScheduler,
    show_boundary: LoweredForePort,
    interaction_boundary: LoweredForePort,
    pending_node: conduit_kernel::NodeId,
    pending_request: RequestId,
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
        presentation: Presentation,
        wake: Wake,
        base_plan: BodyPlan,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        Self::prepare_route(
            body_id,
            host_id,
            boot_id,
            presentation,
            wake,
            base_plan,
            false,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare_route(
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        presentation: Presentation,
        wake: Wake,
        base_plan: BodyPlan,
        select_alternate: bool,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        let planned = plan::planned_mask(
            host_id.clone(),
            boot_id.clone(),
            plan::MASK_SOURCE,
            "browser-graphical",
        )?;
        let alternate = plan::planned_mask(
            host_id,
            boot_id,
            plan::ALTERNATE_MASK_SOURCE,
            "browser-graphical-alternate",
        )?;
        let source_placement_id = base_plan
            .plots
            .first()
            .and_then(|f| f.plan.fragments.first())
            .and_then(|f| f.placements.first())
            .ok_or("Body Plan lacks Presentation source placement")?
            .placement_id
            .clone();
        let chains = [&planned, &alternate]
            .into_iter()
            .map(|p| BodyMaskChainPlan {
                plan: p.plan.clone(),
                stage_placement_ids: p
                    .plan
                    .fragments
                    .iter()
                    .flat_map(|f| &f.placements)
                    .map(|v| v.placement_id.clone())
                    .collect(),
            })
            .collect();
        let body_plan = BodyPlan::seal_with_masks(
            &wake,
            base_plan.plots.clone(),
            vec![BodyMaskTopology {
                face: BodyFaceSelector {
                    plot: base_plan.plots.first().map(|f| f.plot.clone()),
                    source_placement_id,
                },
                chains,
            }],
        )
        .map_err(|e| format!("seal browser Mask Body Plan: {e:?}"))?;
        let (planned, alternate) = if select_alternate {
            (alternate, planned)
        } else {
            (planned, alternate)
        };
        let routes = plan::admitted_routes(&body_plan, &planned, &alternate, true, true)?;
        let mask = planned.mask.clone();
        let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![], vec![])
            .map_err(|error| format!("{error:?}"))?;
        let scoped =
            BodyMaskWardrobe::new(body_id, None, wardrobe).map_err(|error| format!("{error:?}"))?;
        let scoped_body_id = scoped.body_id.clone();
        let mut control =
            MaskWardrobeControl::new(&scoped_body_id, scoped, &body_plan, &routes, None)
                .map_err(|error| format!("{error:?}"))?;
        let wardrobe_action = control
            .apply(
                0,
                MaskWardrobeAction::Wear(mask.plot_identity.clone()),
                &routes,
            )
            .map_err(|error| format!("{error:?}"))?;
        let terminal = planned.show_placement();
        let play = bind_active_play(
            &planned.plan.plan_id,
            &terminal.host_id,
            &terminal.boot_id,
            1,
        );
        let face_subject = presentation
            .subjects
            .first()
            .ok_or_else(|| "browser Mask refuses a Presentation without a subject".to_string())?
            .identity
            .clone();
        let show = MaskShow::prepared(
            &planned,
            &presentation,
            play.clone(),
            face_subject,
            "browser/document".into(),
            SignId::from(format!(
                "sign/browser-mask-prepared/{}",
                presentation.revision
            )),
        )
        .map_err(|error| format!("{error:?}"))?;
        let fragment = planned
            .plan
            .fragments
            .first()
            .ok_or("browser Mask Plan has no fragment")?;
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| format!("lower browser Mask: {error:?}"))?;
        let face_boundary =
            execution::exact_boundary(&lowered.fore_ports, "face", PortDirection::Input)?;
        let show_boundary =
            execution::exact_boundary(&lowered.fore_ports, "show", PortDirection::Output)?.clone();
        let interaction_boundary =
            execution::exact_boundary(&lowered.fore_ports, "interaction", PortDirection::Output)?
                .clone();
        let mut scheduler = execution::mask_scheduler(fragment, &lowered)?;
        let presentation_bytes =
            serde_json::to_vec(&presentation).map_err(|error| error.to_string())?;
        if presentation_bytes.len() > face_boundary.byte_capacity as usize {
            return Err("browser Mask Face exceeds its sealed Fore bound".into());
        }
        match scheduler
            .admit_remote_input(
                face_boundary.endpoint,
                face_boundary.cord,
                0,
                &presentation_bytes,
            )
            .map_err(|error| format!("inject browser Mask Face: {error:?}"))?
        {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 } => {}
            outcome => {
                return Err(format!(
                    "browser Mask Face was not admitted exactly: {outcome:?}"
                ))
            }
        }
        scheduler
            .close_remote_input(face_boundary.endpoint, face_boundary.cord)
            .map_err(|error| format!("close browser Mask Face: {error:?}"))?;
        let pending = loop {
            if let Some(request) = scheduler.next_host_request() {
                break request;
            }
            match scheduler
                .step()
                .map_err(|error| format!("start browser Mask Play: {error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                other => {
                    return Err(format!(
                        "browser Mask did not suspend on its DOM effect: {other:?}"
                    ))
                }
            }
        };
        let presented = scheduler
            .host_value(pending.input.value)
            .map_err(|error| format!("read browser Mask effect input: {error:?}"))?;
        if presented != presentation_bytes.as_slice() {
            return Err("browser Mask Back did not receive the exact Presentation".into());
        }
        let fore_receipt = lowered
            .fore_ports
            .iter()
            .map(|port| BrowserMaskForeReceipt {
                front_port_id: port.front_port_id.as_str().into(),
                direction: port.direction,
                track: port.track,
                value_kind: port.value_kind.as_str().into(),
                endpoint: port.endpoint.0,
                cord: port.cord.0,
            })
            .collect();
        let effect = BrowserMaskEffect {
            schema: "conduit.browser/mask-effect@1".into(),
            mask_plot: mask.plot_identity,
            mask_plan_id: planned.plan.plan_id.clone(),
            mask_play: play.clone(),
            show_id: show.show_id.as_str().into(),
            manifestation_id: show.show.manifestation_id.as_str().into(),
            placement_id: show.show.placement_id.clone(),
            presentation_id: presentation.identity.as_str().into(),
            presentation_revision: presentation.revision,
            text: presentation.text.clone(),
            actions: presentation.actions.clone(),
        };
        Ok((
            Self {
                routes,
                wardrobe_action,
                planned,
                alternate,
                body_plan,
                basis_plan: base_plan,
                wake,
                play,
                presentation,
                show,
                scheduler,
                show_boundary,
                interaction_boundary,
                pending_node: pending.node,
                pending_request: pending.request,
                fore_receipt,
                pending_interaction_node: None,
                pending_interaction_request: None,
                interaction_receipt: None,
            },
            effect,
        ))
    }

    pub fn acknowledge(&mut self, ack: &BrowserMaskAcknowledgement) -> Result<(), String> {
        let exact = ack.show_id == self.show.show_id.as_str()
            && ack.manifestation_id == self.show.show.manifestation_id.as_str()
            && ack.mask_plan_id == self.planned.plan.plan_id
            && ack.active_play_id == self.play.active_play_id
            && ack.placement_id == self.show.show.placement_id
            && ack.presentation_id == self.presentation.identity.as_str()
            && ack.presentation_revision == self.presentation.revision;
        if !exact {
            return Err("browser Mask acknowledgement is stale or mismatched".into());
        }
        let show_bytes = serde_json::to_vec(&self.show.show).map_err(|error| error.to_string())?;
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
                    self.presentation.revision
                )),
            )
            .map_err(|error| format!("{error:?}"))?;
        self.show
            .validate(&self.presentation)
            .map_err(|error| format!("{error:?}"))
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
            presentation: self.presentation.clone(),
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
            self.presentation.clone(),
            self.wake.clone(),
            self.basis_plan.clone(),
            true,
        )
    }

    pub fn replacement(&self, body_id: BodyId) -> Result<(Self, BrowserMaskEffect), String> {
        Self::prepare_route(
            body_id,
            self.play.host_id.clone(),
            BootId::from(format!("{}/mask-replacement", self.play.boot_id.as_str())),
            self.presentation.clone(),
            self.wake.clone(),
            self.body_plan.clone(),
            true,
        )
    }

    pub fn restored(&self, body_id: BodyId) -> Result<(Self, BrowserMaskEffect), String> {
        Self::prepare_route(
            body_id,
            self.play.host_id.clone(),
            self.play.boot_id.clone(),
            self.presentation.clone(),
            self.wake.clone(),
            self.basis_plan.clone(),
            false,
        )
    }
}

#[cfg(test)]
#[path = "workspace_mask_tests.rs"]
mod tests;
