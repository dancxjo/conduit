//! Explicit local speech equipment selection arguments.
use clap::Args;
use std::path::PathBuf;

/// Exact local equipment selected for every fresh installed Host Boot.
#[derive(Debug, Default, Args)]
pub(crate) struct InstalledSpeechOptions {
    /// Select a currently observed speaker and verified eSpeak NG provider.
    #[arg(long, requires_all = ["speaker_card", "speaker_device", "speech_executable", "speech_data", "speech_engine"])]
    pub(crate) selected_speech: bool,
    /// Remove a previously retained speech selection on reinstall.
    #[arg(long, conflicts_with = "selected_speech")]
    pub(crate) without_selected_speech: bool,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speaker_card: Option<String>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speaker_device: Option<u16>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speech_executable: Option<PathBuf>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speech_data: Option<PathBuf>,
    #[arg(long, requires = "selected_speech", num_args = 1..)]
    pub(crate) speech_engine: Vec<PathBuf>,
    #[arg(long, requires = "selected_speech")]
    pub(crate) speech_voice: Option<String>,
}

/// Explicit local synthesis and speaker selection for a screen-free session.
#[derive(Debug, Default, Args)]
pub(crate) struct BirthSpeechOptions {
    /// Speak through one selected, currently discovered ALSA speaker.
    #[arg(long, requires_all = ["speaker_card", "speaker_device", "speech_executable", "speech_data", "speech_engine"])]
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
}

