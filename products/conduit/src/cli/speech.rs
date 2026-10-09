//! Explicit local speech equipment selection arguments.
use clap::Args;
use std::path::PathBuf;

/// Exact local equipment selected for every fresh installed Host Boot.
#[derive(Debug, Default, Args)]
pub(crate) struct InstalledSpeechOptions {
    /// Select a currently observed speaker and verified eSpeak NG provider.
    #[arg(long, group = "selected_speech_mode", requires_all = ["speaker_card", "speaker_device", "speech_executable", "speech_data", "speech_engine", "speech_language_coverage"])]
    pub(crate) selected_speech: bool,
    /// Select the verified voice and retained WAV artifacts without a speaker.
    #[arg(long, group = "selected_speech_mode", requires_all = ["speech_executable", "speech_data", "speech_engine", "speech_language_coverage"])]
    pub(crate) selected_artifact_speech: bool,
    /// Remove a previously retained speech selection on reinstall.
    #[arg(long, conflicts_with_all = ["selected_speech", "selected_artifact_speech"])]
    pub(crate) without_selected_speech: bool,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speaker_card: Option<String>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speaker_device: Option<u16>,
    #[arg(long, requires = "selected_speech_mode")]
    pub(crate) speech_executable: Option<PathBuf>,
    #[arg(long, requires = "selected_speech_mode")]
    pub(crate) speech_data: Option<PathBuf>,
    #[arg(long, requires = "selected_speech_mode", num_args = 1..)]
    pub(crate) speech_engine: Vec<PathBuf>,
    #[arg(long, requires = "selected_speech_mode")]
    pub(crate) speech_voice: Option<String>,
    /// Native LanguageCoverage bound to the exact selected provider source.
    #[arg(long, requires = "selected_speech_mode")]
    pub(crate) speech_language_coverage: Option<PathBuf>,
}

/// Explicit local synthesis and speaker selection for a screen-free session.
#[derive(Debug, Default, Args)]
pub(crate) struct BirthSpeechOptions {
    /// Speak through one selected, currently discovered ALSA speaker.
    #[arg(long, requires_all = ["speaker_card", "speaker_device", "speech_executable", "speech_data", "speech_engine", "speech_language_coverage"])]
    pub(crate) speak: bool,
    /// ALSA card ID from `conduit body speech-options`.
    #[arg(long, requires = "speak")]
    pub(crate) speaker_card: Option<String>,
    /// ALSA device number on the selected card.
    #[arg(long, requires = "speak")]
    pub(crate) speaker_device: Option<u16>,
    /// Exact installed eSpeak NG executable.
    #[arg(long, requires = "speak")]
    pub(crate) speech_executable: Option<PathBuf>,
    /// Exact installed espeak-ng-data directory.
    #[arg(long, requires = "speak")]
    pub(crate) speech_data: Option<PathBuf>,
    /// Exact eSpeak NG engine library and any same-directory dependencies.
    #[arg(long, requires = "speak", num_args = 1..)]
    pub(crate) speech_engine: Vec<PathBuf>,
    /// Voice within the selected installed data tree (default: en-us).
    #[arg(long, requires = "speak")]
    pub(crate) speech_voice: Option<String>,
    /// Native LanguageCoverage bound to the exact selected provider source.
    #[arg(long, requires = "speak")]
    pub(crate) speech_language_coverage: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    #[derive(Parser)]
    struct SelectionCommand {
        #[command(flatten)]
        speech: InstalledSpeechOptions,
    }
    #[test]
    fn artifact_speech_requires_provider_without_inventing_speaker() {
        let provider = [
            "--speech-executable",
            "/bin/espeak-ng",
            "--speech-data",
            "/data",
            "--speech-engine",
            "/lib/engine.so",
            "--speech-language-coverage",
            "coverage.native",
        ];
        let parse = |flags: &[&str]| {
            SelectionCommand::try_parse_from(
                std::iter::once("selection")
                    .chain(flags.iter().copied())
                    .chain(provider),
            )
        };
        let selected = parse(&["--selected-artifact-speech"]).unwrap().speech;
        assert!(selected.selected_artifact_speech);
        assert!(!selected.selected_speech);
        assert!(selected.speaker_card.is_none());
        assert!(selected.speaker_device.is_none());
        for flags in [
            vec![],
            vec!["--selected-speech"],
            vec!["--selected-artifact-speech", "--selected-speech"],
            vec!["--selected-artifact-speech", "--without-selected-speech"],
            vec!["--selected-artifact-speech", "--speaker-card", "card"],
        ] {
            assert!(parse(&flags).is_err(), "{flags:?}");
        }
        assert!(
            SelectionCommand::try_parse_from(["selection", "--selected-artifact-speech"]).is_err()
        );
        assert!(SelectionCommand::try_parse_from(["selection"]).is_ok());
    }
}
