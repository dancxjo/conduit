#[path = "audio_pitch_analysis.rs"]
mod pitch;
pub use pitch::*;
#[path = "audio_clip_frames.rs"]
mod clip_frames;
#[path = "audio_spectral_analysis.rs"]
mod spectral;
pub use clip_frames::*;
#[path = "audio_time_analysis.rs"]
mod time;
pub use spectral::*;
pub use time::*;
pub(crate) fn audio_analysis_palette_metadata(
    kind: &conduit_core::KindId,
) -> Option<crate::PaletteMetadata> {
    if kind.as_str() == PCM_CLIP_FRAMES_KIND {
        return Some(crate::PaletteMetadata {
            category: crate::PaletteCategory::Transform,
            tags: &["audio", "pcm", "clip", "frames"],
            icon: crate::PaletteIconKey::ChartColumnsIncreasing,
        });
    }
    time::audio_time_analysis_palette_metadata(kind)
        .or_else(|| spectral::audio_spectral_analysis_palette_metadata(kind))
        .or_else(|| pitch::audio_pitch_analysis_palette_metadata(kind))
}
