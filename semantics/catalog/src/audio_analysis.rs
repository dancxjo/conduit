#[path = "audio_pitch_analysis.rs"]
mod pitch;
pub use pitch::*;
#[path = "audio_spectral_analysis.rs"]
mod spectral;
#[path = "audio_time_analysis.rs"]
mod time;
pub use spectral::*;
pub use time::*;
pub(crate) fn audio_analysis_palette_metadata(
    kind: &conduit_core::KindId,
) -> Option<crate::PaletteMetadata> {
    time::audio_time_analysis_palette_metadata(kind)
        .or_else(|| spectral::audio_spectral_analysis_palette_metadata(kind))
        .or_else(|| pitch::audio_pitch_analysis_palette_metadata(kind))
}
