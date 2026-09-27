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
use conduit_core::{
    BaseImplementationId, BootId, ComputeServiceGuarantee, ConnectionTrack, HostId,
    OfferGeneration, PlannedGear, PoolRealizationHealth, PortDirection, SignId,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_presentation::{
    ArtifactAcknowledgedSpokenShow, GeneratedContentRole, GeneratedContentSegment,
    GeneratedManifestation, GeneratedManifestationDisposition, GenerativeNarratorRole,
    GenerativePresenterBounds, GenerativePresenterPolicy, GenerativePresenterRequest,
    ManifestationLifecycle, MaskForm, PlannedMaskForm, Presentation, PresentationBasis,
    PresentationDisclosure, PresentationDisclosureLevel, PresentationRole, PresentationSubject,
    PresentationText,
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
        let manifestation = serde_json::to_vec(&GeneratedManifestation {
            manifestation_identity: "manifestation/spoken-mask".into(),
            request_identity: request.request_identity.clone(),
            source_presentation_identity: request.semantic_data.source_presentation_identity,
            source_presentation_revision: request.semantic_data.source_presentation_revision,
            presenter_implementation_identity: "fixture/presenter@1".into(),
            provider_identity: "fixture/provider".into(),
            model_identity: "fixture/model".into(),
            template_contract_revision: request.policy.template_contract_revision,
            generation_run_identity: "run/spoken-mask".into(),
            disposition: GeneratedManifestationDisposition::Produced,
            content: vec![GeneratedContentSegment {
                role: GeneratedContentRole::Speech,
                bytes: b"I am awake.".to_vec(),
            }],
            affordances: vec![],
        })
        .unwrap();
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

#[test]
fn registered_spoken_mask_executes_to_an_artifact_acknowledged_show() {
    let config = StdHostConfig {
        host_id: HostId::from("host/spoken-mask-proof"),
        boot_id: BootId::from("boot/spoken-mask-proof"),
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
        "conduit-spoken-mask-{}-{}.wav",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = std::fs::remove_file(&destination);
    host.attach_deterministic_speech_and_wav_artifact(
        crate::hosted_wav_artifact::WavArtifactSelection::new(
            &destination,
            config.boot_id.clone(),
            config.offer_generation,
        )
        .unwrap(),
    )
    .unwrap();

    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_presentation::install_mask_form_value_aliases(&mut startup).unwrap();
    conduit_presentation::install_spoken_mask_catalog(&mut startup, &mut profiles).unwrap();
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles).unwrap();
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles).unwrap();
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles).unwrap();
    let source = r#"form spoken-generative (
 >> presentation: Presentation
 interaction: FaceInteraction...| >>
 show: Show >>
) {
 request: presentation/adapt-generative-request
 language: llm/present
 speech: presentation/generated-manifestation-speech
 voice: speech/synthesize(maximum-output-bytes = 32768)
 convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = "stereo-left-right")
 artifact: presentation/spoken-artifact
 shown: presentation/artifact-acknowledged-show
 no-input: presentation/no-interaction
 presentation >> request.presentation
 request.request >> language.request
 language.result >> speech.manifestation
 language.result >> shown.manifestation
 speech.speech >> voice.text
 voice.audio >> convert.audio
 convert.converted >> artifact.audio
 artifact.receipt >> shown.artifact
 shown.show >> show
 no-input.interaction >> interaction
}
"#;
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let authoring =
        expand_canonical_form_for_authoring(&checked, "spoken-generative", &profiles).unwrap();
    let mask = MaskForm::admit(&authoring).unwrap();
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
    let authority = host
        .spoken_mask_artifact_authority_grant("grant/spoken-mask-proof")
        .unwrap();
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
    let planned = PlannedMaskForm::admit(&mask, &plan).unwrap();

    let presentation = Presentation::new_with_semantics(
        1,
        PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "body/current".into(),
            role: PresentationRole::Body,
            label: "Current body".into(),
            accessibility_name: "Current body".into(),
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
    .unwrap();
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
        .run_spoken_mask_form_to(
            plan.fragments[0].clone(),
            preparation,
            &[ExternalForeInput {
                front_port_id: conduit_core::port_id("presentation"),
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
    assert!(shown.artifact.pcm_bytes > 0);
    assert!(shown.artifact.blocks > 0);
    let kernel = report.kernel.unwrap();
    assert_eq!(shown.artifact.plan_id, plan.plan_id);
    assert_eq!(shown.artifact.active_play_id, kernel.active_play_id);
    assert!(destination.is_file());
    assert!(std::fs::metadata(&destination).unwrap().len() > 44);
    std::fs::remove_file(destination).unwrap();
}
