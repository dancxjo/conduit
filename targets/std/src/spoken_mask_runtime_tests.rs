use crate::hosted_local_model::{
    HostedLocalModelAdapter, LocalModelAdapterTerminal, LocalModelKindProfile,
};
use crate::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost,
    StdHostComposition, StdHostConfig, TimerAdapter,
};
use conduit_ai::{
    LlmDeterminismProfile, LlmWorkBounds, LocalModelCachePolicy, LocalModelComputeNeed,
    LocalModelIdentity, LocalModelLifecycleState, LocalModelLimits, LocalModelOffer,
};
use conduit_body::Body;
use conduit_core::{
    BaseImplementationId, BootId, CheckedPlotId, ComputeServiceGuarantee, ConnectionTrack,
    ExpandedPlotId, HostId, OfferGeneration, PlanId, PlannedGear, PoolRealizationHealth,
    PortDirection, SignId, SourceDocumentId,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    ArtifactAcknowledgedSpokenShow, GeneratedContentRole, GeneratedContentSegment,
    GeneratedManifestationCandidate, GeneratedManifestationDisposition,
    GeneratedSemanticCorrelation, GenerativeNarratorRole, GenerativePresenterBounds,
    GenerativePresenterPolicy, GenerativePresenterRequest, ManifestationLifecycle, MaskPlot,
    PlannedMaskPlot, Presentation, PresentationBasis, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationRole, PresentationSubject, PresentationText,
};
use std::collections::BTreeMap;

struct FixturePresenter {
    offer: LocalModelOffer,
}

impl HostedLocalModelAdapter for FixturePresenter {
    fn offer(&self) -> &LocalModelOffer {
        &self.offer
    }

    fn current_pool_health(&self) -> PoolRealizationHealth {
        PoolRealizationHealth::Ready
    }

    fn execute(
        &mut self,
        placement: &PlannedGear,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> LocalModelAdapterTerminal {
        if placement.kind_id.as_str() != conduit_ai::LLM_PRESENT_KIND {
            return LocalModelAdapterTerminal::Refused;
        }
        let request: GenerativePresenterRequest = serde_json::from_slice(input).unwrap();
        let mut manifestation = GeneratedManifestationCandidate {
            candidate_identity: String::new(),
            request_identity: request.request_identity.clone(),
            source_presentation_identity: request
                .semantic_data
                .source_presentation_identity
                .clone(),
            source_presentation_revision: request.semantic_data.source_presentation_revision,
            presenter_implementation_identity: "fixture/presenter@1".into(),
            provider_identity: "fixture/provider".into(),
            model_identity: "fixture/model".into(),
            template_contract_revision: request.policy.template_contract_revision.clone(),
            mask_contract_revision: conduit_presentation::SPOKEN_MASK_CONTRACT_REVISION.into(),
            generation_run_identity: "run/spoken-mask".into(),
            disposition: GeneratedManifestationDisposition::Produced,
            content: vec![GeneratedContentSegment {
                role: GeneratedContentRole::Speech,
                source_text_index: 0,
                bytes: request.semantic_data.presentation.text[0]
                    .text
                    .as_bytes()
                    .to_vec(),
            }],
            affordances: vec![],
            correlations: vec![
                GeneratedSemanticCorrelation::Subject {
                    index: 0,
                    identity: request.semantic_data.presentation.subjects[0]
                        .identity
                        .clone(),
                },
                GeneratedSemanticCorrelation::Text {
                    index: 0,
                    subject: request.semantic_data.presentation.text[0].subject.clone(),
                },
            ],
            raw_provider_output: None,
            wording_proposal: None,
        };
        manifestation.candidate_identity = manifestation.digest();
        let manifestation = serde_json::to_vec(&manifestation).unwrap();
        output.clear();
        output.extend_from_slice(&manifestation);
        LocalModelAdapterTerminal::Produced
    }
}

fn presenter_offer() -> LocalModelOffer {
    LocalModelOffer {
        identity: LocalModelIdentity {
            runtime_name: "fixture-runtime".into(),
            runtime_version: "1".into(),
            runtime_build_identity: "fixture-runtime/build-1".into(),
            model_name: "fixture-model".into(),
            model_content_identity: "sha256-fixture".into(),
            architecture: "fixture".into(),
            parameter_profile: "tiny".into(),
            quantization: "exact".into(),
        },
        limits: LocalModelLimits {
            work: LlmWorkBounds::reviewed_default(),
            model_bytes: 1,
            admitted_memory_mib: 1,
            compute: LocalModelComputeNeed {
                minimum_lanes: 1,
                preferred_lanes: 1,
                maximum_lanes: 1,
                minimum_service_guarantee: ComputeServiceGuarantee::Shared,
            },
            maximum_in_flight: 1,
            maximum_queue_items: 1,
            maximum_queue_bytes: 262_144,
            cancellation_supported: true,
            cache_policy: LocalModelCachePolicy::OneLoadedModelUntilShutdown,
        },
        supported_profiles: vec![LocalModelKindProfile::PresentSemanticFront],
        initialized: true,
        lifecycle: LocalModelLifecycleState::Ready,
        determinism: LlmDeterminismProfile::ProviderNondeterministic,
    }
}

#[derive(Default)]
struct Collector(Vec<ExternalForeDelivery>);

impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(output);
        Ok(())
    }
}

struct NoopTimer;
impl TimerAdapter for NoopTimer {
    fn wait(&mut self, _: std::time::Duration) {}
}

fn presentation() -> Presentation {
    let body = Body::born(
        SourceDocumentId::from("source/presentation-fixture"),
        CheckedPlotId::from("checked/presentation-fixture"),
        1,
        SignId::from("sign/presentation-fixture/born"),
    )
    .unwrap();
    Presentation::new_with_semantics(
        1,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: None,
            source_document_id: Some(SourceDocumentId::from("source/presentation-fixture")),
            checked_plot_id: Some(CheckedPlotId::from("checked/presentation-fixture")),
            expanded_plot_id: Some(ExpandedPlotId::from("expanded/presentation-fixture")),
            plan_id: Some(PlanId::from("plan/presentation-fixture")),
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "body/current".into(),
            role: PresentationRole::Body,
            name: "Current body".into(),
        }],
        vec![],
        vec![],
        vec![PresentationText {
            subject: "body/current".into(),
            text: "The Body has awakened.".into(),
        }],
        vec![],
        vec![PresentationDisclosure {
            subject: "body/current".into(),
            level: PresentationDisclosureLevel::Primary,
        }],
    )
    .unwrap()
}

fn execute_spoken_mask(
    plot_name: &str,
    host_id: &str,
    boot_id: &str,
    grant_id: &str,
    presentation: Presentation,
) -> (ArtifactAcknowledgedSpokenShow, conduit_core::Plan, MaskPlot) {
    execute_spoken_mask_with_final_slot(plot_name, host_id, boot_id, grant_id, presentation, false)
}

fn execute_spoken_mask_with_final_slot(
    plot_name: &str,
    host_id: &str,
    boot_id: &str,
    grant_id: &str,
    presentation: Presentation,
    final_slot: bool,
) -> (ArtifactAcknowledgedSpokenShow, conduit_core::Plan, MaskPlot) {
    let config = StdHostConfig {
        host_id: HostId::from(host_id),
        boot_id: BootId::from(boot_id),
        offer_generation: OfferGeneration(1),
    };
    let mut host = StdHost::new_with_local_model(
        config.clone(),
        StdHostComposition::minimal(),
        Box::new(FixturePresenter {
            offer: presenter_offer(),
        }),
    )
    .unwrap();
    let destination = std::env::temp_dir().join(format!(
        "conduit-spoken-mask-{}-{}-{}.wav",
        std::process::id(),
        std::thread::current().name().unwrap_or("test"),
        config.host_id.as_str().replace('/', "-")
    ));
    let _ = std::fs::remove_file(&destination);
    let artifact_root = destination.with_extension("artifacts");
    let selection = if final_slot {
        let _ = std::fs::remove_dir_all(&artifact_root);
        std::fs::create_dir(&artifact_root).unwrap();
        for index in 0..63 {
            std::fs::write(
                artifact_root.join(format!("prior-{index}.wav")),
                b"retained",
            )
            .unwrap();
        }
        crate::hosted_wav_artifact::WavArtifactSelection::per_play_root(
            &artifact_root,
            config.boot_id.clone(),
            config.offer_generation,
        )
        .unwrap()
    } else {
        crate::hosted_wav_artifact::WavArtifactSelection::new(
            &destination,
            config.boot_id.clone(),
            config.offer_generation,
        )
        .unwrap()
    };
    host.attach_deterministic_speech_and_wav_artifact(selection.clone())
        .unwrap();

    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_presentation::install_mask_plot_value_aliases(&mut startup).unwrap();
    conduit_presentation::install_spoken_mask_catalog(&mut startup, &mut profiles).unwrap();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles).unwrap();
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles).unwrap();
    let source = r#"plot PLOT_NAME (
 >> face: Presentation
 interaction: FaceInteraction...| >>
 show: Show >>
) {
 request: presentation/adapt-generative-request
 language: llm/present
 envelope: presentation/build-generated-validation-envelope
 validator: presentation/generated-semantic-validator
 accepted: presentation/retain-generated-validation
 speech: presentation/generated-manifestation-speech
 voice: speech/synthesize(maximum-output-bytes = 32768)
 convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = "stereo-left-right")
 artifact: presentation/spoken-artifact
 shown: presentation/artifact-acknowledged-show
 no-input: presentation/no-interaction
 face >> request.presentation
 request.request >> language.request
 request.request >> envelope.request
 language.result >> envelope.candidate
 language.result >> accepted.candidate
 envelope.envelope >> validator.envelope
 validator.assessment >> accepted.assessment
 accepted.manifestation >> speech.manifestation
 accepted.manifestation >> shown.manifestation
 speech.speech >> voice.text
 voice.audio >> convert.audio
 convert.converted >> artifact.audio
 artifact.receipt >> shown.artifact
 shown.show >> show
 no-input.interaction >> interaction
}
"#
    .replace("PLOT_NAME", plot_name);
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let authoring = expand_canonical_plot_for_authoring(&checked, plot_name, &profiles).unwrap();
    let mask = MaskPlot::admit(&authoring).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts).unwrap();
    let boundary_limits = authoring
        .front
        .inputs()
        .iter()
        .map(|port| (PortDirection::Input, port))
        .chain(
            authoring
                .front
                .outputs()
                .iter()
                .map(|port| (PortDirection::Output, port)),
        )
        .map(|(direction, port)| {
            (
                ForeBoundaryKey {
                    direction,
                    front_port_id: port.port_id.clone(),
                    track: ConnectionTrack::Payload,
                },
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 524_288,
                },
            )
        })
        .collect();
    let connection_bases = BTreeMap::new();
    let line_candidates = BTreeMap::new();
    let authority = host.spoken_mask_artifact_authority_grant(grant_id).unwrap();
    let plan = plan_expanded_authoring_with_options(
        &authoring,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &connection_bases,
            line_candidates: &line_candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: 16_384,
            authority_grants: std::slice::from_ref(&authority),
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .unwrap();
    let planned = PlannedMaskPlot::admit(&mask, &plan).unwrap();

    let request = GenerativePresenterRequest::from_presentation(
        "request/spoken-mask".into(),
        GenerativePresenterPolicy {
            template_contract_revision: "template/spoken-mask@1".into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Speak one truthful sentence from the supplied Presentation.".into(),
        },
        presentation.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .unwrap();
    let preparation = crate::spoken_mask_runtime::SpokenMaskPreparation {
        request,
        presentation: presentation.clone(),
        planned_mask: planned,
        front_subject: "body/current".into(),
        target_subject: "artifact/spoken-mask".into(),
        prepared_sign: SignId::from("sign/spoken-mask/prepared"),
        available_sign: SignId::from("sign/spoken-mask/available"),
    };
    let encoded = serde_json::to_vec(&presentation).unwrap();
    let mut collector = Collector::default();
    let report = host
        .run_spoken_mask_plot_to(
            plan.fragments[0].clone(),
            preparation,
            &[ExternalForeInput {
                front_port_id: conduit_core::port_id("face"),
                track: ConnectionTrack::Payload,
                bytes: encoded,
            }],
            &mut collector,
            &mut Vec::new(),
            &mut NoopTimer,
        )
        .unwrap();
    assert_eq!(collector.0.len(), 1);
    assert_eq!(collector.0[0].front_port_id.as_str(), "show");
    let shown: ArtifactAcknowledgedSpokenShow =
        serde_json::from_slice(&collector.0[0].bytes).unwrap();
    assert_eq!(shown.show.show.lifecycle, ManifestationLifecycle::Available);
    assert_eq!(shown.show.show.presentation_id, presentation.identity);
    assert_eq!(shown.accepted_wording, presentation.text[0].text);
    assert!(shown.artifact.pcm_bytes > 0);
    assert!(shown.artifact.blocks > 0);
    let kernel = report.kernel.unwrap();
    assert_eq!(shown.artifact.plan_id, plan.plan_id);
    assert_eq!(shown.artifact.active_play_id, kernel.active_play_id);
    if final_slot {
        assert!(
            !selection.is_unpublished(),
            "a full pool refuses the next Play"
        );
        shown.show.validate(&presentation).unwrap();
        let locator = shown.artifact.artifact_locator.as_ref().unwrap();
        assert!(std::path::Path::new(locator).is_file());
        assert!(std::fs::metadata(locator).unwrap().len() > 44);
        std::fs::remove_dir_all(artifact_root).unwrap();
    } else {
        assert!(destination.is_file());
        assert!(std::fs::metadata(&destination).unwrap().len() > 44);
        std::fs::remove_file(destination).unwrap();
    }
    (shown, plan, mask)
}

#[test]
fn final_artifact_slot_still_acks_its_completed_show() {
    execute_spoken_mask_with_final_slot(
        "spoken-final-slot",
        "host/spoken-final-slot",
        "boot/spoken-final-slot",
        "grant/spoken-final-slot",
        presentation(),
        true,
    );
}

#[test]
fn registered_spoken_mask_executes_to_an_artifact_acknowledged_show() {
    execute_spoken_mask(
        "spoken-generative",
        "host/spoken-mask-proof",
        "boot/spoken-mask-proof",
        "grant/spoken-mask-proof",
        presentation(),
    );
}

#[test]
fn producer_callable_replays_a_retained_live_manifestation_through_the_spoken_mask() {
    let mut retained = GeneratedManifestationCandidate {
        candidate_identity: String::new(),
        request_identity: "request/original-live-presenter".into(),
        source_presentation_identity: presentation().identity.as_str().into(),
        source_presentation_revision: 1,
        presenter_implementation_identity: "std/local-open-weight-model@1".into(),
        provider_identity: "fixture/live-provider".into(),
        model_identity: "fixture/live-model".into(),
        template_contract_revision: "template/spoken-mask@1".into(),
        mask_contract_revision: conduit_presentation::SPOKEN_MASK_CONTRACT_REVISION.into(),
        generation_run_identity: "run/retained-live-result".into(),
        disposition: GeneratedManifestationDisposition::Produced,
        content: vec![GeneratedContentSegment {
            role: GeneratedContentRole::Speech,
            source_text_index: 0,
            bytes: b"The Body has awakened.".to_vec(),
        }],
        affordances: vec![],
        correlations: vec![
            GeneratedSemanticCorrelation::Subject {
                index: 0,
                identity: "body/current".into(),
            },
            GeneratedSemanticCorrelation::Text {
                index: 0,
                subject: "body/current".into(),
            },
        ],
        raw_provider_output: None,
        wording_proposal: None,
    };
    retained.candidate_identity = retained.digest();
    let executed = crate::spoken_mask_journey::execute_retained_manifestation_mask(
        "spoken-producer-callable",
        "spoken-producer-callable",
        presentation(),
        retained,
    )
    .unwrap();
    assert_eq!(
        executed.shown.show.show.lifecycle,
        ManifestationLifecycle::Available
    );
    assert!(executed.shown.artifact.pcm_bytes > 0);
}

struct SpokenJourney {
    presentation: Presentation,
    wardrobe: conduit_presentation::MaskWardrobe,
    alternate_mask: conduit_core::PlotIdentity,
    initial: Option<(ArtifactAcknowledgedSpokenShow, conduit_core::Plan, MaskPlot)>,
    replacement: Option<(ArtifactAcknowledgedSpokenShow, conduit_core::Plan, MaskPlot)>,
    restoration: Option<(ArtifactAcknowledgedSpokenShow, conduit_core::Plan, MaskPlot)>,
}

impl SpokenJourney {
    fn observation(
        &self,
        action: conduit_presentation::MaskJourneyAction,
        concrete_event: &str,
        selected: Option<&ArtifactAcknowledgedSpokenShow>,
        plan_id: &conduit_core::PlanId,
        route: Option<&str>,
        receipts: Vec<String>,
    ) -> crate::spoken_mask_journey::SpokenMaskJourneyObservation {
        crate::spoken_mask_journey::SpokenMaskJourneyObservation {
            action_id: action.id().into(),
            concrete_event: concrete_event.into(),
            presentation_id: self.presentation.identity.as_str().into(),
            selected_mask_plot_id: selected
                .map(|show| show.show.mask_plot.checked_plot_id.as_str().to_string()),
            plan_id: plan_id.as_str().into(),
            selected_route_id: route.map(str::to_string),
            show_id: selected.map(|show| show.show.show_id.as_str().to_string()),
            receipt_ids: receipts,
        }
    }
}

impl conduit_presentation::MaskJourneyEmbodiment for SpokenJourney {
    type Outcome = crate::spoken_mask_journey::SpokenMaskJourneyObservation;
    type Error = String;

    fn perform(
        &mut self,
        action: conduit_presentation::MaskJourneyAction,
    ) -> Result<Self::Outcome, Self::Error> {
        use conduit_presentation::MaskJourneyAction::*;
        if action == InspectInitialShow {
            self.initial = Some(execute_spoken_mask(
                "spoken-initial",
                "host/spoken-mask-initial",
                "boot/spoken-mask-initial",
                "grant/spoken-mask-initial",
                self.presentation.clone(),
            ));
        }
        if action == AdmitReplacementPlan {
            self.replacement = Some(execute_spoken_mask(
                "spoken-generative",
                "host/spoken-mask-replacement",
                "boot/spoken-mask-replacement",
                "grant/spoken-mask-replacement",
                self.presentation.clone(),
            ));
        }
        if action == InspectRestoredShow {
            self.restoration = Some(execute_spoken_mask(
                "spoken-initial",
                "host/spoken-mask-replacement",
                "boot/spoken-mask-restoration",
                "grant/spoken-mask-restoration",
                self.presentation.clone(),
            ));
        }
        let (initial_show, initial_plan, _) = self
            .initial
            .as_ref()
            .ok_or_else(|| "initial spoken Show has not executed".to_string())?;
        let initial_receipts = || {
            vec![
                initial_show.artifact.artifact_identity.clone(),
                initial_show.artifact.content_sha256.clone(),
                initial_show.artifact.completion_sign_id.as_str().into(),
            ]
        };
        Ok(match action {
            InspectInitialShow => self.observation(
                action,
                "initial spoken semantic, synthesis, artifact, and Show Host Calls completed",
                Some(initial_show),
                &initial_plan.plan_id,
                Some("route/spoken-initial"),
                initial_receipts(),
            ),
            WearAlternateMask => {
                self.wardrobe = self
                    .wardrobe
                    .wear(self.wardrobe.revision, self.alternate_mask.clone())
                    .map_err(|error| format!("wear spoken Mask: {error:?}"))?;
                self.observation(
                    action,
                    "generative spoken Mask became worn; immutable Plan and current Show retained",
                    Some(initial_show),
                    &initial_plan.plan_id,
                    Some("route/spoken-initial"),
                    vec![format!("wardrobe/revision/{}", self.wardrobe.revision)],
                )
            }
            PreferAlternateMask => {
                self.wardrobe = self
                    .wardrobe
                    .prefer(self.wardrobe.revision, vec![self.alternate_mask.clone()])
                    .map_err(|error| format!("prefer spoken Mask: {error:?}"))?;
                self.observation(
                    action,
                    "generative spoken Mask preferred while the immutable current Plan and Show remain",
                    Some(initial_show),
                    &initial_plan.plan_id,
                    Some("route/spoken-initial"),
                    vec![format!("wardrobe/revision/{}", self.wardrobe.revision)],
                )
            }
            WithdrawSelectedRoute => self.observation(
                action,
                "selected spoken route withdrawn; no unsealed route was invented",
                None,
                &initial_plan.plan_id,
                None,
                vec!["route-withdrawal/host-spoken-mask-initial".into()],
            ),
            InspectUnavailableShow => self.observation(
                action,
                "no current human-perceptible Show; retained WAV is stale effect evidence only",
                None,
                &initial_plan.plan_id,
                None,
                vec![],
            ),
            AddFaceHost => self.observation(
                action,
                "replacement Host host/spoken-mask-replacement added but not selected by old Plan",
                None,
                &initial_plan.plan_id,
                None,
                vec!["host/spoken-mask-replacement".into()],
            ),
            AdmitReplacementPlan => {
                let (_, replacement_plan, _) = self.replacement.as_ref().unwrap();
                self.observation(
                    action,
                    "replacement Plan admitted with exact replacement Host placements",
                    None,
                    &replacement_plan.plan_id,
                    Some("route/spoken-replacement"),
                    vec![format!("supersedes/{}", initial_plan.plan_id.as_str())],
                )
            }
            InspectReplannedShow => {
                let (show, plan, _) = self.replacement.as_ref().unwrap();
                self.observation(
                    action,
                    "replacement Play completed semantic, synthesis, artifact, and Show Host Calls",
                    Some(show),
                    &plan.plan_id,
                    Some("route/spoken-replacement"),
                    vec![
                        show.artifact.artifact_identity.clone(),
                        show.artifact.content_sha256.clone(),
                        show.artifact.completion_sign_id.as_str().into(),
                    ],
                )
            }
            DoffAlternateMask => {
                self.wardrobe = self
                    .wardrobe
                    .doff(self.wardrobe.revision, &self.alternate_mask)
                    .map_err(|error| format!("doff spoken Mask: {error:?}"))?;
                let (_, plan, _) = self.replacement.as_ref().unwrap();
                self.observation(
                    action,
                    "generative spoken Mask doffed; replacement Show ceased being current",
                    None,
                    &plan.plan_id,
                    None,
                    vec![format!("wardrobe/revision/{}", self.wardrobe.revision)],
                )
            }
            InspectRestoredShow => {
                let (show, plan, _) = self.restoration.as_ref().unwrap();
                self.observation(
                    action,
                    "initial spoken Mask restored through a fresh exact Plan and acknowledged artifact Show",
                    Some(show),
                    &plan.plan_id,
                    Some("route/spoken-restored"),
                    vec![
                        show.artifact.artifact_identity.clone(),
                        show.artifact.content_sha256.clone(),
                        show.artifact.completion_sign_id.as_str().into(),
                    ],
                )
            }
        })
    }
}

#[test]
fn screen_free_spoken_producer_actualizes_the_shared_ten_action_journey() {
    let presentation = presentation();
    let initial = execute_spoken_mask(
        "spoken-initial",
        "host/spoken-mask-bootstrap",
        "boot/spoken-mask-bootstrap",
        "grant/spoken-mask-bootstrap",
        presentation.clone(),
    );
    let initial_identity = initial.2.plot_identity.clone();
    let alternate_identity = execute_spoken_mask(
        "spoken-generative",
        "host/spoken-mask-identity",
        "boot/spoken-mask-identity",
        "grant/spoken-mask-identity",
        presentation.clone(),
    )
    .2
    .plot_identity;
    let mut journey = SpokenJourney {
        presentation,
        wardrobe: conduit_presentation::MaskWardrobe::new(
            conduit_presentation::MaskWardrobeLifetime::Body,
            vec![initial_identity],
            vec![],
        )
        .unwrap(),
        alternate_mask: alternate_identity,
        initial: Some(initial),
        replacement: None,
        restoration: None,
    };
    let mut observations = Vec::new();
    conduit_presentation::actualize_mask_journey(&mut journey, |_, outcome| {
        observations.push(outcome.clone());
    })
    .unwrap();
    let evidence = crate::spoken_mask_journey::SpokenMaskJourneyEvidence {
        schema: crate::spoken_mask_journey::SpokenMaskJourneyEvidence::SCHEMA.into(),
        observations,
        human_hearing_observed: false,
        inward_face_interaction_observed: false,
    };
    evidence.validate().unwrap();
    assert_ne!(
        evidence.observations[5].receipt_ids[0],
        evidence.observations[0].plan_id
    );
    assert_ne!(
        evidence.observations[0].plan_id,
        evidence.observations[6].plan_id
    );
    assert!(evidence.observations[4].show_id.is_none());
    assert!(evidence.observations[7].show_id.is_some());
    assert_ne!(
        evidence.observations[7].selected_mask_plot_id,
        evidence.observations[9].selected_mask_plot_id
    );
    assert_ne!(
        evidence.observations[7].plan_id,
        evidence.observations[9].plan_id
    );
}
