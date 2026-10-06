//! Shared preparation and controlled execution, before success-only artifact admission.
use super::*;
const LEGACY_SPOKEN_TEMPLATE_REVISION: &str = "template/spoken-mask@1";

fn policy_for_retained(
    candidate: &conduit_presentation::GeneratedManifestationCandidate,
) -> Result<conduit_presentation::GenerativePresenterPolicy, String> {
    use conduit_presentation::{GenerativeNarratorRole, GenerativePresenterPolicy};
    match candidate.template_contract_revision.as_str() {
        conduit_presentation::FINITE_FACE_WORDING_TEMPLATE_REVISION => {
            Ok(crate::hosted_local_model::finite_face_wording_presenter_policy())
        }
        LEGACY_SPOKEN_TEMPLATE_REVISION => Ok(GenerativePresenterPolicy {
            template_contract_revision: LEGACY_SPOKEN_TEMPLATE_REVISION.into(),
            narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
            instructions: "Speak one truthful sentence from the supplied Presentation.".into(),
        }),
        _ => Err("retained Presenter selected an unsupported spoken Mask template".into()),
    }
}

pub(super) struct MaskRun {
    pub report: crate::StdRunReport,
    pub deliveries: Vec<crate::ExternalForeDelivery>,
    pub plan: conduit_core::Plan,
    pub mask: conduit_presentation::MaskPlot,
    pub destination: std::path::PathBuf,
    pub real_speech: bool,
}
pub(super) fn run_mask(
    plot_name: &str,
    execution_id: &str,
    presentation: conduit_presentation::Presentation,
    retained: conduit_presentation::GeneratedManifestationCandidate,
    real: Option<(
        crate::hosted_speech_synthesis::EspeakDiscovery,
        &std::path::Path,
    )>,
    streaming: bool,
    control: &crate::RunControl,
    language: &conduit_language::LanguageRequest,
) -> Result<MaskRun, String> {
    use conduit_core::{
        BaseImplementationId, BootId, ConnectionTrack, HostId, OfferGeneration, PortDirection,
        SignId,
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
        GenerativePresenterBounds, GenerativePresenterRequest, MaskPlot, PlannedMaskPlot,
    };
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct Collector(Vec<crate::ExternalForeDelivery>);
    impl crate::ExternalForeOutputAdapter for Collector {
        fn deliver(&mut self, output: crate::ExternalForeDelivery) -> Result<(), String> {
            self.0.push(output);
            Ok(())
        }
    }
    struct NoopTimer;
    impl crate::TimerAdapter for NoopTimer {
        fn wait(&mut self, _: std::time::Duration) {}
    }

    let policy = policy_for_retained(&retained)?;
    let safe_id = execution_id.replace('/', "-");
    let config = crate::StdHostConfig {
        host_id: HostId::from(format!("host/{execution_id}")),
        boot_id: BootId::from(format!("boot/{execution_id}")),
        offer_generation: OfferGeneration(1),
    };
    let mut host = crate::StdHost::new_with_local_model(
        config.clone(),
        crate::StdHostComposition::minimal(),
        Box::new(Replay {
            offer: offer(&retained),
            retained,
        }),
    )?;
    let real_speech = real.is_some();
    let destination = real
        .as_ref()
        .map(|(_, path)| path.to_path_buf())
        .unwrap_or_else(|| {
            std::env::temp_dir().join(format!(
                "conduit-spoken-mask-{}-{safe_id}.wav",
                std::process::id()
            ))
        });
    if real_speech && destination.exists() {
        return Err("retained WAV destination already exists".into());
    }
    if !real_speech {
        let _ = std::fs::remove_file(&destination);
    }
    let artifact = crate::hosted_wav_artifact::WavArtifactSelection::new(
        &destination,
        config.boot_id.clone(),
        config.offer_generation,
    )?;
    if let Some((discovery, _)) = real {
        let adapter = discovery
            .initialize(
                config.host_id.clone(),
                config.boot_id.clone(),
                config.offer_generation,
                conduit_core::AuthorityGrantId::from(format!("grant/{execution_id}/speech")),
                std::time::Duration::from_secs(if streaming { 30 } else { 10 }),
            )
            .map_err(|error| error.to_string())?;
        host.attach_espeak_speech_and_wav_artifact(adapter, artifact)?;
    } else {
        host.attach_deterministic_speech_and_wav_artifact(artifact)?;
    }
    let maximum_output_bytes = if streaming {
        1_323_000
    } else if real_speech {
        131_072
    } else {
        32_768
    };
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_presentation::install_mask_plot_value_aliases(&mut startup)?;
    conduit_presentation::install_spoken_mask_catalog(&mut startup, &mut profiles)?;
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)?;
    conduit_tongues::install_speech_commit_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    let source = graph::source(plot_name, maximum_output_bytes, streaming, language);
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup)
        .map_err(|error| format!("check spoken Mask: {error:?}"))?;
    let authoring = expand_canonical_plot_for_authoring(&checked, plot_name, &profiles)
        .map_err(|error| format!("expand spoken Mask: {error:?}"))?;
    let mask = MaskPlot::admit(&authoring).map_err(|error| format!("admit Mask: {error:?}"))?;
    let hosts = [host.advertisement().clone()];
    let placements = default_expanded_placements(&authoring.expanded, &hosts)
        .map_err(|error| format!("place spoken Mask: {error:?}"))?;
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
    let grant_id = format!("grant/{execution_id}");
    let mut authority = vec![host.spoken_mask_artifact_authority_grant(&grant_id)?];
    if real_speech {
        authority.push(if streaming {
            host.streaming_speech_authority_grant()?
        } else {
            host.speech_synthesis_authority_grant()?
        });
    }
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
            authority_grants: &authority,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundary_limits,
    )
    .map_err(|error| format!("plan spoken Mask: {error:?}"))?;
    let planned = PlannedMaskPlot::admit(&mask, &plan)
        .map_err(|error| format!("seal spoken Mask: {error:?}"))?;
    let request = GenerativePresenterRequest::from_presentation(
        format!("request/{execution_id}"),
        policy,
        presentation.clone(),
        None,
        GenerativePresenterBounds::reviewed_default(),
    )
    .map_err(|error| format!("prepare Presenter request: {error:?}"))?;
    let front_subject = presentation
        .subjects
        .iter()
        .find(|subject| subject.role == conduit_presentation::PresentationRole::Body)
        .map(|subject| subject.identity.clone())
        .ok_or("spoken Mask Face has no Body subject")?;
    let preparation = crate::spoken_mask_runtime::SpokenMaskPreparation {
        request,
        presentation: presentation.clone(),
        planned_mask: planned,
        front_subject,
        target_subject: format!("artifact/{execution_id}"),
        prepared_sign: SignId::from(format!("sign/{execution_id}/prepared")),
        available_sign: SignId::from(format!("sign/{execution_id}/available")),
    };
    let mut collector = Collector::default();
    let report = host
        .run_spoken_mask_plot_controlled_to(
            plan.fragments[0].clone(),
            preparation,
            &[crate::ExternalForeInput {
                front_port_id: conduit_core::port_id("face"),
                track: ConnectionTrack::Payload,
                bytes: serde_json::to_vec(&presentation).map_err(|error| error.to_string())?,
            }],
            &mut collector,
            &mut Vec::new(),
            &mut NoopTimer,
            control,
        )
        .map_err(|error| format!("execute spoken Mask: {error:?}"))?;
    Ok(MaskRun {
        report,
        deliveries: collector.0,
        plan,
        mask,
        destination,
        real_speech,
    })
}
