//! Explicit local speech selection for producer-owned documentary audio.
use clap::Args;
use conduit_std_host::hosted_speech_synthesis::EspeakDiscovery;
use std::path::PathBuf;

#[derive(Args, Debug, Default)]
pub(super) struct SpeechOptions {
    /// Local eSpeak NG executable; no audio device is opened.
    #[arg(long, requires_all = ["speech_data", "speech_engine", "speech_language_coverage", "journey_documentary"])]
    speech_executable: Option<PathBuf>,
    /// Exact espeak-ng-data directory to admit with the engine.
    #[arg(long, requires = "speech_executable")]
    speech_data: Option<PathBuf>,
    /// Exact engine library and optional additional runtime dependencies.
    #[arg(long, requires = "speech_executable", num_args = 1..)]
    speech_engine: Vec<PathBuf>,
    /// Native LanguageCoverage bound to this exact installed provider source.
    #[arg(long, requires = "speech_executable")]
    speech_language_coverage: Option<PathBuf>,
    /// Installed voice selected from the admitted data directory.
    #[arg(long, default_value = "en-us", requires = "speech_executable")]
    speech_voice: String,
}

impl SpeechOptions {
    pub(super) fn discover(&self) -> Result<Option<EspeakDiscovery>, Box<dyn std::error::Error>> {
        let Some(executable) = &self.speech_executable else {
            return Ok(None);
        };
        let data = self.speech_data.as_ref().ok_or("speech data is required")?;
        let coverage = conduit_std_host::hosted_speech_synthesis::read_language_coverage(
            self.speech_language_coverage
                .as_ref()
                .ok_or("speech Language coverage is required")?,
        )?;
        Ok(Some(
            EspeakDiscovery::inspect(executable, data, &self.speech_voice, &self.speech_engine)?
                .declare_language_coverage(coverage)?,
        ))
    }
}
