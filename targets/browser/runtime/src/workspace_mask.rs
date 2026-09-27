//! Browser realization of the tutorial Face through one ordinary Mask Form.

use conduit_body::BodyId;
use conduit_core::{
    bind_active_play, kind_id, port_id, ActivePlayIdentity, ArtifactId, Back, BackOfferBuilder,
    BootId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement,
    HostCallContractId, HostCallRequirement, HostId, HostProfileId, ImplementationId, Kind,
    KindIdentity, OfferGeneration, PortDescriptor, PortDirection, PortTemporal, SignId,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_kernel::scheduler::{
    FixedScheduler, SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, HostCallDisposition, HostCallId,
    HostCallOutcome, HostedSignLog, HostedValueStore, PortId as KernelPortId, RequestId,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredFrontPort, FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_presentation::{
    install_mask_form_value_aliases, AdmittedMaskFormRoutes, BodyMaskWardrobe,
    ManifestationLifecycle, MaskForm, MaskShow, MaskWardrobe, MaskWardrobeAction,
    MaskWardrobeControl, MaskWardrobeControlEvidence, MaskWardrobeLifetime, PlannedMaskForm,
    Presentation, SealedMaskFormRoute, PRESENTATION_INTERACTION_VALUE_KIND,
    PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MASK_SOURCE: &str = "form browser-graphical (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n mask: presentation/browser-dom-mask\n presentation >> mask.presentation\n mask.interaction >> interaction\n mask.show >> show\n}\n";
const MASK_OPERATION: &str = "browser.host/dom-mask@1";
const MASK_BYTES: u32 = 512 * 1024;
const MASK_PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
type MaskScheduler =
    FixedScheduler<MaskBack, HostedValueStore, HostedSignLog, 1, 3, MASK_PORTS, 12, 48, 3, 1, 1>;

struct MaskBack {
    pending: bool,
    complete: bool,
}

impl StepBack<MASK_PORTS> for MaskBack {
    fn step(
        &mut self,
        io: &mut StepIo<MASK_PORTS>,
        _inputs: &StepInputBytes<'_, MASK_PORTS>,
    ) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if self.pending {
            let Some((RequestId(0), outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
                return mask_back_failure(1);
            }
            let Some(output) = outcome.output else {
                return mask_back_failure(2);
            };
            if !io.output_ready(KernelPortId(1)) {
                return StepOutcome::Await;
            }
            if io.consume_host_completion().is_err()
                || io.send(KernelPortId(1), output.value).is_err()
            {
                return mask_back_failure(3);
            }
            self.pending = false;
            self.complete = true;
            return StepOutcome::Complete;
        }
        if let Some(value) = io.input(KernelPortId(0)) {
            let input = match BoundedValueRef::new(value, MASK_BYTES) {
                Ok(input) => input,
                Err(_) => return mask_back_failure(4),
            };
            if io.consume(KernelPortId(0)).is_err()
                || io
                    .request_host_call(RequestId(0), HostCallId(0), input)
                    .is_err()
            {
                return mask_back_failure(5);
            }
            self.pending = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }
}

fn mask_back_failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserMaskEffect {
    pub schema: &'static str,
    pub mask_form: conduit_core::FormIdentity,
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
    pub planned_mask: PlannedMaskForm,
    pub mask_play: ActivePlayIdentity,
    pub presentation: Presentation,
    pub mask_show: MaskShow,
}

pub struct BrowserMaskRuntime {
    routes: AdmittedMaskFormRoutes,
    wardrobe_action: MaskWardrobeControlEvidence,
    planned: PlannedMaskForm,
    play: ActivePlayIdentity,
    presentation: Presentation,
    show: MaskShow,
    scheduler: MaskScheduler,
    show_boundary: LoweredFrontPort,
    interaction_boundary: LoweredFrontPort,
    pending_node: conduit_kernel::NodeId,
    pending_request: RequestId,
}

impl BrowserMaskRuntime {
    pub fn prepare(
        body_id: BodyId,
        host_id: HostId,
        boot_id: BootId,
        presentation: Presentation,
    ) -> Result<(Self, BrowserMaskEffect), String> {
        let (mask, planned, routes) = planned_mask(host_id, boot_id)?;
        let wardrobe = MaskWardrobe::new(MaskWardrobeLifetime::Body, vec![], vec![])
            .map_err(|error| format!("{error:?}"))?;
        let scoped =
            BodyMaskWardrobe::new(body_id, None, wardrobe).map_err(|error| format!("{error:?}"))?;
        let scoped_body_id = scoped.body_id.clone();
        let mut control = MaskWardrobeControl::new(
            &scoped_body_id,
            scoped,
            planned.plan.plan_id.clone(),
            &routes,
            None,
        )
        .map_err(|error| format!("{error:?}"))?;
        let wardrobe_action = control
            .apply(
                0,
                MaskWardrobeAction::Wear(mask.form_identity.clone()),
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
        let presentation_boundary =
            exact_boundary(&lowered.front_ports, "presentation", PortDirection::Input)?;
        let show_boundary =
            exact_boundary(&lowered.front_ports, "show", PortDirection::Output)?.clone();
        let interaction_boundary =
            exact_boundary(&lowered.front_ports, "interaction", PortDirection::Output)?.clone();
        let mut scheduler = mask_scheduler(fragment, &lowered)?;
        let presentation_bytes =
            serde_json::to_vec(&presentation).map_err(|error| error.to_string())?;
        if presentation_bytes.len() > presentation_boundary.byte_capacity as usize {
            return Err("browser Mask Presentation exceeds its sealed Fore bound".into());
        }
        match scheduler
            .admit_remote_input(
                presentation_boundary.endpoint,
                presentation_boundary.cord,
                0,
                &presentation_bytes,
            )
            .map_err(|error| format!("inject browser Mask Presentation: {error:?}"))?
        {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence: 0 } => {}
            outcome => {
                return Err(format!(
                    "browser Mask Presentation was not admitted exactly: {outcome:?}"
                ))
            }
        }
        scheduler
            .close_remote_input(presentation_boundary.endpoint, presentation_boundary.cord)
            .map_err(|error| format!("close browser Mask Presentation: {error:?}"))?;
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
        let effect = BrowserMaskEffect {
            schema: "conduit.browser/mask-effect@1",
            mask_form: mask.form_identity,
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
                play,
                presentation,
                show,
                scheduler,
                show_boundary,
                interaction_boundary,
                pending_node: pending.node,
                pending_request: pending.request,
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
        drive_mask_to_terminal(
            &mut self.scheduler,
            [&self.show_boundary, &self.interaction_boundary],
        )?;
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
            mask_play: self.play.clone(),
            presentation: self.presentation.clone(),
            mask_show: self.show.clone(),
        }
    }
}

fn port(
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn exact_boundary<'a>(
    ports: &'a [LoweredFrontPort],
    name: &str,
    direction: PortDirection,
) -> Result<&'a LoweredFrontPort, String> {
    let mut matches = ports
        .iter()
        .filter(|port| port.front_port_id.as_str() == name && port.direction == direction);
    let port = matches
        .next()
        .ok_or_else(|| format!("browser Mask lacks sealed Fore port '{name}'"))?;
    if matches.next().is_some() {
        return Err(format!("browser Mask Fore port '{name}' is ambiguous"));
    }
    Ok(port)
}

fn mask_scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
) -> Result<MaskScheduler, String> {
    if fragment.placements.len() != 1 || lowered.cords.len() != 3 {
        return Err("browser Mask Plan has an unexpected finite shape".into());
    }
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "browser Mask nodes")?;
    let cords = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "browser Mask cords")?;
    let mut routes = FixedRoutes::<48, 3>::new(MASK_PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|error| format!("install browser Mask route: {error:?}"))?;
    }
    routes
        .seal()
        .map_err(|error| format!("seal browser Mask routes: {error:?}"))?;
    let mut bindings = FixedHostCallBindings::<1>::new(1);
    for operation in &lowered.host_calls {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("install browser Mask host call: {error:?}"))?;
    }
    bindings
        .seal()
        .map_err(|error| format!("seal browser Mask host calls: {error:?}"))?;
    let values = HostedValueStore::new(12, MASK_BYTES, MASK_BYTES * 4)
        .map_err(|error| format!("prepare browser Mask values: {error:?}"))?;
    const MASK_SIGN_ITEMS: u16 = 256;
    let sign_bytes = u32::from(MASK_SIGN_ITEMS)
        .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
        .ok_or("browser Mask Sign budget overflow")?;
    const MASK_REMOTE_SIGN_ITEMS: u16 = 64;
    let remote_sign_bytes = conduit_kernel::remote_sign_storage_bytes(MASK_REMOTE_SIGN_ITEMS)
        .ok_or("browser Mask remote Sign budget overflow")?;
    let signs = HostedSignLog::new_with_remote_storage(
        MASK_SIGN_ITEMS,
        sign_bytes,
        MASK_REMOTE_SIGN_ITEMS,
        remote_sign_bytes,
    )
    .map_err(|error| format!("prepare browser Mask signs: {error:?}"))?;
    MaskScheduler::new_with_host_calls(
        nodes,
        cords,
        routes,
        bindings,
        [MaskBack {
            pending: false,
            complete: false,
        }],
        values,
        signs,
    )
    .map_err(|error| format!("prepare browser Mask scheduler: {error:?}"))
}

fn drive_mask_to_terminal(
    scheduler: &mut MaskScheduler,
    boundaries: [&LoweredFrontPort; 2],
) -> Result<(), String> {
    for _ in 0..32 {
        if boundaries.iter().all(|port| {
            scheduler
                .remote_egress_terminal(port.endpoint, port.cord)
                .unwrap_or(false)
        }) {
            return Ok(());
        }
        for port in boundaries {
            if scheduler
                .remote_egress_offer(port.endpoint, port.cord)
                .map_err(|error| format!("inspect browser Mask Fore output: {error:?}"))?
                .is_some()
                && port.front_port_id.as_str() != "show"
            {
                return Err("browser Mask emitted an unexpected interaction".into());
            }
        }
        match scheduler
            .step()
            .map_err(|error| format!("drain browser Mask Play: {error:?}"))?
        {
            SchedulerStatus::Progress { .. } | SchedulerStatus::Drained | SchedulerStatus::Idle => {
            }
            SchedulerStatus::Cancelled => return Err("browser Mask Play was cancelled".into()),
        }
    }
    Err("browser Mask Fore outputs did not become terminal".into())
}

fn planned_mask(
    host_id: HostId,
    boot_id: BootId,
) -> Result<(MaskForm, PlannedMaskForm, AdmittedMaskFormRoutes), String> {
    let definition = Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id("presentation/browser-dom-mask"),
        kind_contract_revision: KindIdentity::from("conduit.browser/presentation-dom-mask@1"),
        inputs: vec![port(
            "presentation",
            PRESENTATION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )],
        outputs: vec![
            port(
                "interaction",
                PRESENTATION_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "show",
                SHOW_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            ),
        ],
        configuration: vec![],
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: 512 * 1024,
        },
    };
    let mut startup = StartupCatalog::new();
    install_mask_form_value_aliases(&mut startup).map_err(|error| format!("{error:?}"))?;
    startup
        .insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .map_err(|error| format!("{error:?}"))?;
    let mut profiles = ProfileCatalog::new();
    profiles
        .insert_kind(definition.clone())
        .map_err(|error| format!("{error:?}"))?;
    let syntax = parse_syntax_document(MASK_SOURCE);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("{:?}", syntax.diagnostics));
    }
    let checked = check_syntax_document(&syntax, &startup).map_err(|error| format!("{error:?}"))?;
    let authoring = expand_canonical_form_for_authoring(&checked, "browser-graphical", &profiles)
        .map_err(|error| format!("{error:?}"))?;
    let mask = MaskForm::admit(&authoring).map_err(|error| format!("{error:?}"))?;
    let offer = BackOfferBuilder::new(
        definition,
        Back {
            capability_id: CapabilityId::from("capability/browser-dom-mask"),
            execution_profile_id: ExecutionProfileId::from("browser/mask@1"),
            implementation_id: ImplementationId::from("implementation/browser-dom-mask"),
            artifact_id: ArtifactId::from("artifact/browser-runtime"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(MASK_OPERATION),
                target_kind: Some(kind_id("presentation/browser-dom-mask")),
                maximum_in_flight: 1,
                maximum_input_bytes: MASK_BYTES,
                maximum_output_bytes: MASK_BYTES,
            }],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("browser/mask@1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![offer],
        planner_capabilities: vec![],
    };
    let placements = conduit_planner::default_expanded_placements(
        &authoring.expanded,
        core::slice::from_ref(&host),
    )
    .map_err(|error| format!("{error:?}"))?;
    let mut boundary_limits = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, authoring.input_bindings.as_slice()),
        (PortDirection::Output, authoring.output_bindings.as_slice()),
    ] {
        for binding in bindings {
            boundary_limits.insert(
                conduit_planner::FrontBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 4,
                    byte_capacity: 512 * 1024,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authoring,
        &[host],
        &placements,
        &[],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 4,
            connection_byte_capacity: 512 * 1024,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .map_err(|error| format!("{error:?}"))?;
    let planned = PlannedMaskForm::admit(&mask, &plan).map_err(|error| format!("{error:?}"))?;
    let route = SealedMaskFormRoute {
        route_id: "route/browser-graphical".into(),
        mask_form: mask.form_identity.clone(),
        plan_id: plan.plan_id.clone(),
        placement_ids: plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .map(|placement| placement.placement_id.clone())
            .collect(),
        currently_available: true,
    };
    let routes = AdmittedMaskFormRoutes::new(&plan, core::slice::from_ref(&mask), vec![route])
        .map_err(|error| format!("{error:?}"))?;
    Ok((mask, planned, routes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{CheckedFormId, ExpandedFormId, PlanId, SourceDocumentId};
    use conduit_presentation::{
        PresentationBasis, PresentationRole, PresentationSubject, PresentationText,
    };

    fn body_id() -> BodyId {
        conduit_body::Body::born(
            SourceDocumentId::from("source/browser-mask-test"),
            CheckedFormId::from("checked/browser-mask-test"),
            1,
            SignId::from("sign/body-born"),
        )
        .unwrap()
        .body_id
    }

    fn presentation() -> Presentation {
        Presentation::new(
            7,
            PresentationBasis {
                body_id: Some(body_id()),
                wake_id: None,
                source_document_id: Some(SourceDocumentId::from("source/application")),
                checked_form_id: Some(CheckedFormId::from("checked/application")),
                expanded_form_id: Some(ExpandedFormId::from("expanded/application")),
                plan_id: Some(PlanId::from("plan/application")),
                active_play_id: None,
                sign_ids: vec![SignId::from("sign/presentation")],
            },
            vec![PresentationSubject {
                identity: "body/browser-mask-test".into(),
                role: PresentationRole::Body,
                label: "Test Body".into(),
                accessibility_name: "Test Body".into(),
            }],
            vec![],
            vec![],
            vec![PresentationText {
                subject: "body/browser-mask-test".into(),
                text: "One canonical journey".into(),
            }],
        )
        .unwrap()
    }

    fn acknowledgement(effect: &BrowserMaskEffect) -> BrowserMaskAcknowledgement {
        BrowserMaskAcknowledgement {
            show_id: effect.show_id.clone(),
            manifestation_id: effect.manifestation_id.clone(),
            mask_plan_id: effect.mask_plan_id.clone(),
            active_play_id: effect.mask_play.active_play_id.clone(),
            placement_id: effect.placement_id.clone(),
            presentation_id: effect.presentation_id.clone(),
            presentation_revision: effect.presentation_revision,
        }
    }

    #[test]
    fn show_becomes_available_only_after_exact_browser_acknowledgement() {
        let (mut runtime, effect) = BrowserMaskRuntime::prepare(
            body_id(),
            HostId::from("host/browser"),
            BootId::from("boot/browser"),
            presentation(),
        )
        .unwrap();
        assert_eq!(
            runtime.show.show.lifecycle,
            ManifestationLifecycle::Prepared
        );
        assert_ne!(
            runtime.presentation.basis.plan_id.as_ref(),
            Some(&runtime.planned.plan.plan_id)
        );

        let mut stale = acknowledgement(&effect);
        stale.presentation_revision += 1;
        assert!(runtime.acknowledge(&stale).is_err());
        assert_eq!(
            runtime.show.show.lifecycle,
            ManifestationLifecycle::Prepared
        );

        runtime.acknowledge(&acknowledgement(&effect)).unwrap();
        assert_eq!(
            runtime.show.show.lifecycle,
            ManifestationLifecycle::Available
        );
        assert_eq!(
            runtime.wardrobe_action.resulting_wardrobe.worn,
            vec![runtime.planned.mask.form_identity.clone()]
        );
    }
}
