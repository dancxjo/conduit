//! Target-owned realization of the body's selected Presenter chains.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::{
    BodyPlan, BodyPlayIdentity, BodyPresentationSelector, BodyPresenterChainPlan,
    BodyPresenterTopology, ResidentForm,
};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    OfferGeneration, PROTOCOL_VERSION, PlacementId, Plan, SignId, kind_id, resource_offer,
    resource_requirement,
};
use conduit_form::{ProfileCatalog, parse};
use conduit_planner::{default_placements, plan};
use conduit_presentation::{
    MAX_RENDERER_VALUE_BYTES, Manifestation, ManifestationLifecycle, Presentation,
    PresentationBasis, PresentationRole, PresentationSubject, RendererRealizationOffer,
    renderer_kind_definition, renderer_offer,
};
use patchbay_application::{
    PatchbayPresenterMode, PatchbayPresenterStage, PatchbayPresenterTopology,
};

#[derive(Clone)]
pub(super) struct PresenterControl {
    host_id: HostId,
    boot_id: conduit_core::BootId,
    graphical: Option<PresenterStage>,
    speech: Option<PresenterStage>,
    sequence: u64,
    presentation: Option<Presentation>,
    manifestations: Vec<Manifestation>,
}

#[derive(Clone)]
struct PresenterStage {
    plan: Plan,
    placement_id: PlacementId,
    target: String,
}

impl PresenterControl {
    pub(super) fn graphical(host_id: HostId, boot_id: conduit_core::BootId) -> Result<Self, ()> {
        let graphical = prepare_stage(
            Adapter::Native,
            &host_id,
            &boot_id,
            1,
            "conduitos/native-patchbay",
        )?;
        Ok(Self {
            host_id,
            boot_id,
            graphical: Some(graphical),
            speech: None,
            sequence: 2,
            presentation: None,
            manifestations: Vec::new(),
        })
    }

    pub(super) fn request(&mut self, mode: PatchbayPresenterMode) -> Result<(), ()> {
        if matches!(
            mode,
            PatchbayPresenterMode::Graphical | PatchbayPresenterMode::GraphicalAndSpeech
        ) && self.graphical.is_none()
        {
            self.graphical = Some(prepare_stage(
                Adapter::Native,
                &self.host_id,
                &self.boot_id,
                self.sequence,
                "conduitos/native-patchbay-restored",
            )?);
            self.sequence = self.sequence.checked_add(1).ok_or(())?;
        }
        if matches!(
            mode,
            PatchbayPresenterMode::Speech | PatchbayPresenterMode::GraphicalAndSpeech
        ) && self.speech.is_none()
        {
            self.speech = Some(prepare_stage(
                Adapter::Speech,
                &self.host_id,
                &self.boot_id,
                self.sequence,
                "conduitos/test-speech",
            )?);
            self.sequence = self.sequence.checked_add(1).ok_or(())?;
        }
        if mode == PatchbayPresenterMode::Graphical {
            self.speech = None;
        } else if mode == PatchbayPresenterMode::Speech {
            self.graphical = None;
        }
        self.presentation = None;
        self.manifestations.clear();
        Ok(())
    }

    pub(super) fn topology(
        &self,
        selector: BodyPresentationSelector,
    ) -> Result<BodyPresenterTopology, ()> {
        let mut chains = Vec::new();
        for stage in self.graphical.iter().chain(self.speech.iter()) {
            chains.push(BodyPresenterChainPlan {
                plan: stage.plan.clone(),
                stage_placement_ids: vec![stage.placement_id.clone()],
            });
        }
        if chains.is_empty() {
            return Err(());
        }
        Ok(BodyPresenterTopology {
            presentation: selector,
            chains,
        })
    }

    pub(super) fn activate(
        &mut self,
        body_plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<PatchbayPresenterTopology, ()> {
        let topology = body_plan.presenter_topologies.first().ok_or(())?;
        let form = topology.presentation.form.as_ref();
        let expanded_form_id = form.and_then(|selected| {
            body_plan
                .forms
                .iter()
                .find(|candidate| &candidate.form == selected)
                .map(|candidate| candidate.plan.expanded_form_id.clone())
        });
        let presentation = Presentation::new(
            self.sequence,
            PresentationBasis {
                body_id: Some(body_plan.body_id.clone()),
                wake_id: Some(body_plan.wake_id.clone()),
                source_document_id: form.map(|form| form.source_document_id.clone()),
                checked_form_id: form.map(|form| form.checked_form_id.clone()),
                expanded_form_id,
                plan_id: Some(body_plan.plan_id.clone()),
                active_play_id: Some(play.active_play_id.clone()),
                sign_ids: vec![SignId::from(format!(
                    "conduitos/presenter/{}/presentation",
                    self.sequence
                ))],
            },
            vec![PresentationSubject {
                identity: "conduitos/patchbay/self".into(),
                role: PresentationRole::Document,
                label: "Patchbay".into(),
                accessibility_name: "Patchbay controlling its own Presenter topology".into(),
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
        .map_err(|_| ())?;
        let mut manifestations = Vec::new();
        for (index, stage) in self.graphical.iter().chain(self.speech.iter()).enumerate() {
            let fragment = stage.plan.fragments.first().ok_or(())?;
            let active = conduit_core::bind_active_play(
                &stage.plan.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                play.play_sequence,
            );
            let prepared = Manifestation::prepared(
                &presentation,
                &stage.plan,
                active,
                stage.placement_id.clone(),
                "conduitos/patchbay/self".into(),
                stage.target.clone(),
                SignId::from(format!("conduitos/presenter/{index}/prepared")),
            )
            .map_err(|_| ())?;
            manifestations.push(
                prepared
                    .transition(
                        ManifestationLifecycle::Available,
                        SignId::from(format!("conduitos/presenter/{index}/available")),
                    )
                    .map_err(|_| ())?,
            );
        }
        let mode = match (self.graphical.is_some(), self.speech.is_some()) {
            (true, false) => PatchbayPresenterMode::Graphical,
            (true, true) => PatchbayPresenterMode::GraphicalAndSpeech,
            (false, true) => PatchbayPresenterMode::Speech,
            (false, false) => return Err(()),
        };
        let mut stages = Vec::with_capacity(manifestations.len());
        for manifestation in &manifestations {
            let planned = self
                .graphical
                .iter()
                .chain(self.speech.iter())
                .find(|stage| stage.placement_id == manifestation.placement_id)
                .and_then(|stage| {
                    stage
                        .plan
                        .fragments
                        .iter()
                        .flat_map(|fragment| &fragment.placements)
                        .find(|placement| placement.placement_id == manifestation.placement_id)
                })
                .ok_or(())?;
            let resource = planned.resources.first().ok_or(())?;
            stages.push(PatchbayPresenterStage {
                manifestation_id: manifestation.manifestation_id.as_str().into(),
                implementation_id: manifestation.presenter_implementation_id.as_str().into(),
                host_id: manifestation.host_id.as_str().into(),
                boot_id: manifestation.boot_id.as_str().into(),
                resource_pool_id: resource.pool_id.as_str().into(),
                resource_class_id: resource.class_id.as_str().into(),
                reserved_units: resource.units,
                maximum_active_instances: planned.limits.max_active_instances,
                maximum_queue_items: planned.limits.max_queue_items,
                maximum_queue_bytes: planned.limits.max_queue_bytes,
                available: manifestation.lifecycle == ManifestationLifecycle::Available,
            });
        }
        let view = PatchbayPresenterTopology {
            presentation_id: presentation.identity.as_str().into(),
            body_plan_id: body_plan.plan_id.clone(),
            active_play_id: play.active_play_id.as_str().into(),
            mode,
            stages,
        };
        self.sequence = self.sequence.checked_add(1).ok_or(())?;
        self.presentation = Some(presentation);
        self.manifestations = manifestations;
        Ok(view)
    }
}

pub(super) fn patchbay_selector(plan: &BodyPlan) -> Result<BodyPresentationSelector, ()> {
    let resident = crate::native_workset::resident(crate::native_workset::NativeForm::Patchbay)
        .map_err(|_| ())?;
    let partition = plan
        .forms
        .iter()
        .find(|partition| partition.form == resident)
        .ok_or(())?;
    let placement = partition
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| {
            placement.kind_id.as_str()
                == conduit_semantic_catalog::APPLICATION_VIEW_PRESENTATION_KIND
        })
        .ok_or(())?;
    Ok(BodyPresentationSelector {
        form: Some(ResidentForm::new(
            resident.source_document_id,
            resident.checked_form_id,
        )),
        source_placement_id: placement.placement_id.clone(),
    })
}

#[derive(Clone, Copy)]
enum Adapter {
    Native,
    Speech,
}

fn prepare_stage(
    adapter: Adapter,
    host_id: &HostId,
    boot_id: &conduit_core::BootId,
    generation: u64,
    target: &str,
) -> Result<PresenterStage, ()> {
    let mut catalog = ProfileCatalog::new();
    catalog.insert(renderer_kind_definition()).map_err(|_| ())?;
    let form = parse(
        "form conduitos-presenter {\n    renderer: presentation/renderer\n}\n",
        &catalog,
    )
    .map_err(|_| ())?;
    let advertisement = renderer_host(adapter, host_id, boot_id, generation);
    let placements =
        default_placements(&form, core::slice::from_ref(&advertisement)).map_err(|_| ())?;
    let plan = plan(&form, &[advertisement], &placements, &[]).map_err(|_| ())?;
    let placement_id = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.kind_id.as_str() == conduit_presentation::RENDERER_KIND)
        .map(|placement| placement.placement_id.clone())
        .ok_or(())?;
    Ok(PresenterStage {
        plan,
        placement_id,
        target: target.into(),
    })
}

fn renderer_host(
    adapter: Adapter,
    host_id: &HostId,
    boot_id: &conduit_core::BootId,
    generation: u64,
) -> HostAdvertisement {
    let (capability, implementation, artifact, target_kind, resource_class) = match adapter {
        Adapter::Native => (
            "renderer-conduitos",
            "presentation/renderer-conduitos-native@1",
            "conduitos/native-compositor@1",
            "presentation/base/conduitos-surface@1",
            "conduit.resource/conduitos-surface@1",
        ),
        Adapter::Speech => (
            "renderer-test-speech",
            "presentation/renderer-test-speech@1",
            "conduitos/test-speech@1",
            "presentation/base/test-speech@1",
            "conduit.resource/test-speech-sink@1",
        ),
    };
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("conduitos/presenter-host@1"),
        bases: vec![],
        resources: vec![resource_offer(
            &format!("{}/{capability}", host_id.as_str()),
            resource_class,
            1,
        )],
        capabilities: vec![renderer_offer(RendererRealizationOffer {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from("conduitos/bounded-presenter@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_operation: HostOperationRequirement {
                contract_id: HostOperationContractId::from("conduit.host/present@1"),
                target_kind: Some(kind_id(target_kind)),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                maximum_output_bytes: MAX_RENDERER_VALUE_BYTES,
            },
            resource_requirement: resource_requirement(resource_class, 1),
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
            },
        })],
        planner_capabilities: Vec::new(),
    }
}

/// Exact native-presenter advertisement used by boot-time Body rendezvous.
pub fn native_host_advertisement(
    host_id: &HostId,
    boot_id: &conduit_core::BootId,
    generation: u64,
) -> HostAdvertisement {
    renderer_host(Adapter::Native, host_id, boot_id, generation)
}
