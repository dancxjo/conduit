//! Target-owned realization of the body's selected Presenter chains.

use alloc::{format, string::String, vec, vec::Vec};
use conduit_body::{
    BodyPlan, BodyPlayIdentity, BodyPresentationSelector, BodyPresenterChainPlan,
    BodyPresenterTopology, ResidentForm,
};
use conduit_core::{
    ArtifactId, BaseImplementationId, CapabilityId, CapabilityLimits, ExecutionProfileId,
    HostAdvertisement, HostCallContractId, HostCallRequirement, HostId, HostProfileId,
    ImplementationId, ImplementationOffer, OfferGeneration, PROTOCOL_VERSION, SignId, kind_id,
    resource_offer, resource_requirement,
};
use conduit_form::{
    KindSignature, ProfileCatalog, StartupCatalog, check_syntax_document,
    expand_canonical_form_for_authoring, parse_syntax_document,
};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical};
use conduit_presentation::{
    InteractionRealizationOffer, MAX_RENDERER_VALUE_BYTES, ManifestationLifecycle, MaskForm,
    MaskShow, PlannedMaskForm, Presentation, PresentationBasis, PresentationRole,
    PresentationSubject, RendererRealizationOffer, install_mask_form_value_aliases,
    interaction_kind_projection, interaction_offer, presentation_tee_kind_projection,
    presentation_tee_offer, renderer_kind_projection, renderer_offer,
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
    shows: Vec<MaskShow>,
}

#[derive(Clone)]
struct PresenterStage {
    planned_mask: PlannedMaskForm,
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
            shows: Vec::new(),
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
        self.shows.clear();
        Ok(())
    }

    pub(super) fn topology(
        &self,
        selector: BodyPresentationSelector,
    ) -> Result<BodyPresenterTopology, ()> {
        let mut chains = Vec::new();
        for stage in self.graphical.iter().chain(self.speech.iter()) {
            chains.push(BodyPresenterChainPlan {
                plan: stage.planned_mask.plan.clone(),
                // BodyPresenterChainPlan is the legacy linear projection used by
                // the Patchbay topology view. The ordinary Mask Plan above owns
                // the complete branched renderer/interaction graph; this path
                // names only its exact Show-producing stage.
                stage_placement_ids: vec![stage.planned_mask.show_placement().placement_id.clone()],
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
        let mut shows = Vec::new();
        for (index, stage) in self.graphical.iter().chain(self.speech.iter()).enumerate() {
            let fragment = stage.planned_mask.plan.fragments.first().ok_or(())?;
            let active = conduit_core::bind_active_play(
                &stage.planned_mask.plan.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                play.play_sequence,
            );
            let prepared = MaskShow::prepared(
                &stage.planned_mask,
                &presentation,
                active,
                "conduitos/patchbay/self".into(),
                stage.target.clone(),
                SignId::from(format!("conduitos/presenter/{index}/prepared")),
            )
            .map_err(|_| ())?;
            shows.push(
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
        let mut stages = Vec::with_capacity(shows.len());
        for show in &shows {
            let manifestation = &show.show;
            let planned = self
                .graphical
                .iter()
                .chain(self.speech.iter())
                .find(|stage| {
                    stage.planned_mask.show_placement().placement_id == manifestation.placement_id
                })
                .and_then(|stage| {
                    stage
                        .planned_mask
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
        self.shows = shows;
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
    let mut startup = StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    install_mask_form_value_aliases(&mut startup).map_err(|_| ())?;
    for projection in [
        renderer_kind_projection(),
        interaction_kind_projection(),
        presentation_tee_kind_projection(),
    ] {
        startup
            .insert(KindSignature {
                kind: projection.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .map_err(|_| ())?;
        catalog.insert(projection).map_err(|_| ())?;
    }
    let (source, entry) = match adapter {
        Adapter::Native => (
            include_str!("../../../forms/native-graphical-mask/main.conduit"),
            "native-graphical",
        ),
        Adapter::Speech => (
            include_str!("../../../forms/spoken-mask/main.conduit"),
            "spoken",
        ),
    };
    let checked =
        check_syntax_document(&parse_syntax_document(source), &startup).map_err(|_| ())?;
    let authoring =
        expand_canonical_form_for_authoring(&checked, entry, &catalog).map_err(|_| ())?;
    let mask = MaskForm::admit(&authoring).map_err(|_| ())?;
    let advertisement = renderer_host(adapter, host_id, boot_id, generation);
    let placements =
        default_expanded_placements(&authoring.expanded, core::slice::from_ref(&advertisement))
            .map_err(|_| ())?;
    let plan = plan_expanded_canonical(
        &authoring.expanded,
        &[advertisement],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .map_err(|_| ())?;
    Ok(PresenterStage {
        planned_mask: PlannedMaskForm::admit(&mask, &plan).map_err(|_| ())?,
        target: target.into(),
    })
}

fn renderer_host(
    adapter: Adapter,
    host_id: &HostId,
    boot_id: &conduit_core::BootId,
    generation: u64,
) -> HostAdvertisement {
    let (capability, implementation, artifact, target_kind, resource_class, input_resource) =
        match adapter {
            Adapter::Native => (
                "renderer-conduitos",
                "presentation/renderer-conduitos-native@1",
                "conduitos/native-compositor@1",
                "presentation/base/conduitos-surface@1",
                "conduit.resource/conduitos-surface@1",
                "conduit.resource/conduitos-human-input@1",
            ),
            Adapter::Speech => (
                "renderer-test-speech",
                "presentation/renderer-test-speech@1",
                "conduitos/test-speech@1",
                "presentation/base/test-speech@1",
                "conduit.resource/test-speech-sink@1",
                "conduit.resource/test-speech-input@1",
            ),
        };
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: host_id.clone(),
        boot_id: boot_id.clone(),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from("conduitos/presenter-host@1"),
        bases: vec![],
        resources: vec![
            resource_offer(
                &format!("{}/{capability}", host_id.as_str()),
                resource_class,
                1,
            ),
            resource_offer(
                &format!("{}/{capability}-input", host_id.as_str()),
                input_resource,
                1,
            ),
        ],
        capabilities: vec![
            presentation_tee_offer(
                CapabilityId::from(format!("{capability}-tee")),
                ImplementationOffer {
                    execution_profile_id: ExecutionProfileId::from("conduitos/bounded-presenter@1"),
                    implementation_id: ImplementationId::from("conduit.presentation/tee-kernel@1"),
                    artifact_id: ArtifactId::from("conduitos/presentation-tee@1"),
                },
                CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 1,
                    max_queue_bytes: MAX_RENDERER_VALUE_BYTES,
                },
            ),
            renderer_offer(RendererRealizationOffer {
                capability_id: CapabilityId::from(capability),
                execution_profile_id: ExecutionProfileId::from("conduitos/bounded-presenter@1"),
                implementation_id: ImplementationId::from(implementation),
                artifact_id: ArtifactId::from(artifact),
                host_call: HostCallRequirement {
                    contract_id: HostCallContractId::from("conduit.host/present@1"),
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
            }),
            interaction_offer(InteractionRealizationOffer {
                capability_id: CapabilityId::from(format!("{capability}-input")),
                execution_profile_id: ExecutionProfileId::from("conduitos/bounded-interaction@1"),
                implementation_id: ImplementationId::from(match adapter {
                    Adapter::Native => "presentation/conduitos-human-input@1",
                    Adapter::Speech => "presentation/test-speech-input@1",
                }),
                artifact_id: ArtifactId::from(match adapter {
                    Adapter::Native => "conduitos/native-compositor-input@1",
                    Adapter::Speech => "conduitos/test-speech-input@1",
                }),
                host_call: HostCallRequirement {
                    contract_id: HostCallContractId::from(
                        "conduit.host/presentation-interaction@1",
                    ),
                    target_kind: Some(kind_id(
                        conduit_presentation::PRESENTATION_INTERACTION_VALUE_KIND,
                    )),
                    maximum_in_flight: 1,
                    maximum_input_bytes: MAX_RENDERER_VALUE_BYTES,
                    maximum_output_bytes: conduit_presentation::MAX_PRESENTATION_INTERACTION_BYTES
                        as u32,
                },
                resource_requirement: resource_requirement(input_resource, 1),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 8,
                    max_queue_bytes: conduit_presentation::MAX_PRESENTATION_INTERACTION_BYTES
                        as u32
                        * 8,
                },
            }),
        ],
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_presenter_is_the_exact_ordinary_mask_form_and_keeps_its_branched_plan() {
        let host = HostId::from("conduitos/test-host");
        let boot = conduit_core::BootId::from("conduitos/test-boot");
        let control = PresenterControl::graphical(host, boot).unwrap();
        let stage = control.graphical.as_ref().unwrap();

        assert_eq!(stage.planned_mask.mask.form_name, "native-graphical");
        assert_eq!(
            stage.planned_mask.plan.source_document_id,
            stage.planned_mask.mask.form_identity.source_document_id
        );
        assert_eq!(
            stage.planned_mask.plan.checked_form_id,
            stage.planned_mask.mask.form_identity.checked_form_id
        );
        assert_eq!(
            stage.planned_mask.plan.expanded_form_id,
            stage.planned_mask.mask.form_identity.expanded_form_id
        );
        assert_eq!(
            [
                &stage.planned_mask.mask.presentation_input,
                &stage.planned_mask.mask.interaction_output,
                &stage.planned_mask.mask.show_output,
            ]
            .len(),
            3,
            "the Mask retains one Presentation input and its interaction and Show outputs"
        );
        assert_eq!(
            stage
                .planned_mask
                .plan
                .fragments
                .iter()
                .map(|fragment| fragment.connections.len())
                .sum::<usize>(),
            3,
            "the planned Mask retains both Presentation branches and the renderer-to-interaction Show correlation"
        );

        let topology = control
            .topology(BodyPresentationSelector {
                form: None,
                source_placement_id: conduit_core::PlacementId::from("application/presentation"),
            })
            .unwrap();
        assert_eq!(topology.chains.len(), 1);
        assert_eq!(topology.chains[0].stage_placement_ids.len(), 1);
        assert_eq!(
            topology.chains[0].stage_placement_ids[0],
            stage.planned_mask.show_placement().placement_id
        );
        assert_eq!(topology.chains[0].plan, stage.planned_mask.plan);
    }
}
