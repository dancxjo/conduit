//! Ordinary direct speech Mask: one Face, one bounded opening artifact Show.
//!
//! The source has no device or provider facts. The selected Host must offer
//! its wording, voice, conversion, and create-new artifact Backs before Plan.
//! A selected speaker reads the complete Face in separate bounded Plays
//! sourced from the available Show; this artifact alone proves the opening.

use std::collections::BTreeMap;

use conduit_core::{AuthorityGrant, BaseImplementationId, ConnectionTrack, Plan, PortDirection};
use conduit_planner::{
    default_expanded_placements, plan_expanded_authoring_with_options, ConnectionQueueLimits,
    ForeBoundaryKey, PlanningOptions,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{MaskPlot, PlannedMaskPlot, Presentation};

pub struct PreparedDirectSpokenMask {
    pub plan: Plan,
    pub mask: MaskPlot,
    pub planned_mask: PlannedMaskPlot,
    pub authority_grants: Vec<AuthorityGrant>,
}

impl crate::StdHost {
    /// Plan against this installed Host, its exact selected voice, and its
    /// one create-new artifact resource. No replay Host or BodyPlan is made.
    pub fn prepare_direct_spoken_mask(
        &self,
        face: &Presentation,
    ) -> Result<PreparedDirectSpokenMask, String> {
        face.validate()
            .map_err(|error| format!("prepare direct Face: {error:?}"))?;
        // Refuse an oversized reading before sealing an advertised route.
        crate::direct_spoken_mask_runtime::prepare_wording_items(face)?;
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        conduit_presentation::install_mask_plot_value_aliases(&mut startup)?;
        conduit_presentation::install_mask_mechanism_catalog(&mut startup, &mut profiles)?;
        conduit_presentation::install_spoken_mask_catalog(&mut startup, &mut profiles)?;
        conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles)?;
        conduit_tongues::install_speech_commit_catalog(&mut startup, &mut profiles)?;
        conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
        let plot_name = "owner_direct_spoken_mask";
        let checked = check_syntax_document(&parse_syntax_document(&source(plot_name)), &startup)
            .map_err(|error| format!("check direct spoken Mask: {error:?}"))?;
        let authoring = expand_canonical_plot_for_authoring(&checked, plot_name, &profiles)
            .map_err(|error| format!("expand direct spoken Mask: {error:?}"))?;
        let mask = MaskPlot::admit(&authoring)
            .map_err(|error| format!("admit direct spoken Mask: {error:?}"))?;
        let hosts = [self.advertisement().clone()];
        let placements = default_expanded_placements(&authoring.expanded, &hosts)
            .map_err(|error| format!("place direct spoken Mask: {error:?}"))?;
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
        let authority_grants = vec![
            self.spoken_mask_artifact_authority_grant("grant/owner-direct-speech/artifact")?,
            self.streaming_speech_authority_grant()?,
        ];
        let plan = plan_expanded_authoring_with_options(
            &authoring,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 16_384,
                authority_grants: &authority_grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundary_limits,
        )
        .map_err(|error| format!("plan direct spoken Mask: {error:?}"))?;
        let planned_mask = PlannedMaskPlot::admit(&mask, &plan)
            .map_err(|error| format!("seal direct spoken Mask: {error:?}"))?;
        Ok(PreparedDirectSpokenMask {
            plan,
            mask,
            planned_mask,
            authority_grants,
        })
    }
}

/// A finite direct route. The wording Back streams one exact Face-derived
/// opening; the Show Back accepts the artifact receipt after synthesis ends.
pub fn source(plot_name: &str) -> String {
    let language = conduit_language::LanguageRequest::new(
        conduit_language::LanguageId::new("language/english".into())
            .expect("direct spoken Mask Language"),
        None,
        conduit_language::LanguageVarietyPolicy::LanguageSufficient,
    )
    .expect("direct spoken Mask request");
    let language_request = crate::hosted_language::language_request_literal(&language);
    format!(
        r#"plot {plot_name} (
 >> face: Presentation
 interaction: FaceInteraction...| >>
 show: Show >>
) {{
 spread: presentation/tee
 wording: presentation/direct-face-wording
 commit: speech/commit-generated-text
 voice: speech/synthesize-stream(language-request = {language_request}, maximum-output-bytes = 1323000, maximum-audio-millis = 30000, maximum-segments = 32)
 convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = "stereo-left-right", maximum-blocks = 32768, maximum-audio-millis = 30000)
 artifact: presentation/spoken-artifact(maximum-blocks = 32768, maximum-audio-millis = 30000)
 shown: presentation/direct-artifact-show
 no-input: presentation/no-interaction
 face >> spread.source
 spread.presentation >> wording.presentation
 spread.presentation >> shown.presentation
 wording.speech >> commit.generated
 commit.segments >> voice.text
 voice.audio >> convert.audio
 convert.converted >> artifact.audio
 artifact.receipt >> shown.artifact
 shown.show >> show
 no-input.interaction >> interaction
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_face_artifact_graph_is_an_ordinary_checked_mask() {
        let mut startup = StartupCatalog::new();
        let mut profiles = ProfileCatalog::new();
        conduit_presentation::install_mask_plot_value_aliases(&mut startup).unwrap();
        conduit_presentation::install_mask_mechanism_catalog(&mut startup, &mut profiles).unwrap();
        conduit_presentation::install_spoken_mask_catalog(&mut startup, &mut profiles).unwrap();
        conduit_tongues::install_speech_synthesis_catalog(&mut startup, &mut profiles).unwrap();
        conduit_tongues::install_speech_commit_catalog(&mut startup, &mut profiles).unwrap();
        conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles).unwrap();
        let checked = check_syntax_document(
            &parse_syntax_document(&source("direct_spoken_test")),
            &startup,
        )
        .unwrap();
        let authoring =
            expand_canonical_plot_for_authoring(&checked, "direct_spoken_test", &profiles).unwrap();
        assert_eq!(authoring.front.inputs().len(), 1);
        assert_eq!(authoring.front.outputs().len(), 2);
        assert_eq!(authoring.input_bindings.len(), 1);
        assert_eq!(authoring.output_bindings.len(), 2);
        let mask = MaskPlot::admit(&authoring).unwrap();
        assert_eq!(mask.plot_name, "direct_spoken_test");
    }

    #[test]
    fn attached_spoken_output_advertises_the_installed_face_tee() {
        let mut host = crate::StdHost::new_with_composition(
            crate::StdHostConfig {
                host_id: "host/direct-spoken-tee".into(),
                boot_id: "boot/direct-spoken-tee".into(),
                offer_generation: conduit_core::OfferGeneration(1),
            },
            crate::StdHostComposition::minimal(),
        );
        assert!(!host.advertisement().capabilities.iter().any(|offer| {
            offer.kind_id.as_str() == conduit_presentation::PRESENTATION_TEE_KIND
        }));
        let artifact = crate::hosted_wav_artifact::WavArtifactSelection::new(
            std::env::temp_dir().join("conduit-direct-spoken-tee-offer.wav"),
            host.advertisement().boot_id.clone(),
            host.advertisement().offer_generation,
        )
        .unwrap();
        host.attach_deterministic_speech_and_wav_artifact(artifact)
            .unwrap();
        let offered = host
            .advertisement()
            .capabilities
            .iter()
            .find(|offer| offer.kind_id.as_str() == conduit_presentation::PRESENTATION_TEE_KIND)
            .expect("attached spoken Host offers its installed Face tee");
        assert_eq!(
            offered.implementation.implementation_id.as_str(),
            conduit_std_offers::PRESENTATION_TEE_IMPLEMENTATION
        );
        assert!(offered.host_calls.is_empty());
        assert!(offered.resource_requirements.is_empty());
    }

    #[test]
    #[ignore = "requires installed eSpeak NG and data; proves live-provider planning, not audio playback"]
    fn installed_espeak_admits_the_direct_face_mask_without_terminal_attachment() {
        use conduit_core::{CheckedPlotId, SignId, SourceDocumentId};
        use conduit_presentation::{
            PresentationBasis, PresentationRole, PresentationSubject, PresentationText,
        };
        use std::{path::Path, time::Duration};

        let config = crate::StdHostConfig {
            host_id: "host/direct-spoken-plan".into(),
            boot_id: "boot/direct-spoken-plan".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        };
        let engine = std::fs::canonicalize("/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1").unwrap();
        let discovery = crate::hosted_speech_synthesis::EspeakDiscovery::inspect(
            Path::new("/usr/bin/espeak-ng"),
            Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
            "en-us",
            &[engine],
        )
        .unwrap();
        let coverage = crate::hosted_language::tests::fixture_coverage(
            &discovery.provider_identity(),
            "en-us",
            "language/english",
        );
        let adapter = discovery
            .declare_language_coverage(coverage)
            .unwrap()
            .initialize(
                config.host_id.clone(),
                config.boot_id.clone(),
                config.offer_generation,
                "grant/direct-spoken-plan".into(),
                Duration::from_secs(30),
            )
            .unwrap();
        let artifact = crate::hosted_wav_artifact::WavArtifactSelection::new(
            std::env::temp_dir().join("conduit-direct-spoken-plan.wav"),
            config.boot_id.clone(),
            config.offer_generation,
        )
        .unwrap();
        let mut host =
            crate::StdHost::new_with_composition(config, crate::StdHostComposition::minimal());
        host.attach_espeak_speech_and_wav_artifact(adapter, artifact)
            .unwrap();
        let body = conduit_body::Body::born(
            SourceDocumentId::from("source/direct-spoken-plan"),
            CheckedPlotId::from("checked/direct-spoken-plan"),
            1,
            SignId::from("sign/direct-spoken-plan/born"),
        )
        .unwrap();
        let face = Presentation::new(
            1,
            PresentationBasis {
                body_id: Some(body.body_id),
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![PresentationSubject {
                identity: "body/current".into(),
                role: PresentationRole::Body,
                name: "Current Body".into(),
            }],
            vec![],
            vec![],
            vec![PresentationText {
                subject: "body/current".into(),
                text: "Ready to speak.".into(),
            }],
        )
        .unwrap();
        let prepared = host.prepare_direct_spoken_mask(&face).unwrap();
        assert!(prepared
            .plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .any(|placement| {
                placement.kind_id.as_str() == conduit_presentation::PRESENTATION_TEE_KIND
                    && placement.implementation_id.as_str()
                        == conduit_std_offers::PRESENTATION_TEE_IMPLEMENTATION
            }));
    }
}
