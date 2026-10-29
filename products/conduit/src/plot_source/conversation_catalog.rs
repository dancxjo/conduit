//! Portable conversation meaning for public authoring, without provider offers.

use conduit_plot::{ProfileCatalog, StartupCatalog};

pub(super) fn install(
    startup: &mut StartupCatalog,
    profiles: &mut ProfileCatalog,
) -> Result<(), String> {
    conduit_tongues::install_speech_recognition_catalog(startup, profiles)?;
    // The sound catalog owns audio/play; do not install it a second time.
    conduit_tongues::install_speech_synthesis_catalog(startup, profiles)?;
    conduit_tongues::install_speech_commit_catalog(startup, profiles)?;
    conduit_chat::install_body_chat_catalog(startup, profiles)?;
    conduit_ai::install_llm_semantic_catalog(startup, profiles)?;
    conduit_ai::install_model_text_catalog(startup, profiles)?;
    // The existing sound catalog also owns push-to-talk capture.
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn public_catalog_checks_and_expands_the_complete_shipped_conversation() {
        let source = super::super::parse(include_str!(
            "../../../../plots/live-conversation/main.conduit"
        ))
        .unwrap();
        source.check().unwrap();
        let authored = source.expand_entry_for_authoring().unwrap();
        assert_eq!(authored.expanded.name, "spoken-live-conversation");
        for kind in [
            "speech/recognize-stream",
            "speech/commit-recognized-turn",
            "llm/generate-stream",
            "speech/commit-generated-text",
            "speech/synthesize-stream",
            "audio/capture-push-to-talk",
            "audio/play",
        ] {
            assert!(
                authored
                    .expanded
                    .gears
                    .iter()
                    .any(|gear| gear.kind_id.as_str() == kind),
                "missing {kind}"
            );
        }
    }

    #[test]
    fn public_catalog_checks_and_expands_the_committed_transcript() {
        let source = super::super::parse(include_str!(
            "../../../../plots/live-transcript/main.conduit"
        ))
        .unwrap();
        source.check().unwrap();
        let authored = source.expand_entry_for_authoring().unwrap();
        assert_eq!(authored.expanded.name, "live-transcript");
        assert!(authored
            .expanded
            .gears
            .iter()
            .any(|gear| gear.kind_id.as_str() == "face/text"));
    }

    #[test]
    fn conversation_registration_does_not_accept_an_unknown_provider_kind() {
        let source =
            super::super::parse("plot unknown { model: ollama/ambient-generate }.").unwrap();
        assert!(source.check().is_err());
    }
}
