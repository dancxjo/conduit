//! Ordinary direct speech Mask: one Face, one exact artifact Show.
//!
//! The source has no device or provider facts. The selected Host must offer
//! its wording, voice, conversion, and create-new artifact Backs before Plan.

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

/// A finite direct route. The wording Back streams only exact Face clauses;
/// the Show Back accepts the artifact receipt after synthesis completes.
pub fn source(plot_name: &str) -> String {
    format!(
        r#"plot {plot_name} (
 >> face: Presentation
 interaction: FaceInteraction...| >>
 show: Show >>
) {{
 wording: presentation/direct-face-wording
 commit: speech/commit-generated-text
 voice: speech/synthesize-stream(maximum-output-bytes = 1323000, maximum-audio-millis = 30000, maximum-segments = 32)
 convert: audio/convert-pcm-profile(output-sample-rate-hz = 48000, output-channel-layout = "stereo-left-right", maximum-blocks = 32768, maximum-audio-millis = 30000)
 artifact: presentation/spoken-artifact(maximum-blocks = 32768, maximum-audio-millis = 30000)
 shown: presentation/direct-artifact-show
 no-input: presentation/no-interaction
 face >> wording.presentation
 face >> shown.presentation
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
        let mask = MaskPlot::admit(&authoring).unwrap();
        assert_eq!(mask.plot_name, "direct_spoken_test");
    }
}
