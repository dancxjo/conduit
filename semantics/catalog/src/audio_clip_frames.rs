//! Catalog decoration over the Audio-owned finite clip expansion.
use crate::{KindTerminalBehavior, StandardKindContract};
use alloc::string::{String, ToString};
pub use conduit_audio::{
    pcm_clip_frames_semantic_contract, PCM_CLIP_FRAMES_KIND, PCM_CLIP_FRAMES_REVISION,
};
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog};

pub fn audio_clip_frames_contract() -> StandardKindContract {
    let kind = pcm_clip_frames_semantic_contract();
    StandardKindContract {
        kind_id: kind.kind_id, plain_name: "PCM clip frames".into(),
        summary: "Drain one validated finite clip into its original PCM frames in order; retain clock, offsets and short tail.".into(),
        inputs: kind.inputs, outputs: kind.outputs, configuration: kind.configuration, limits: kind.limits,
        terminal_behavior: KindTerminalBehavior::MirrorsInputTerminal,
        hosted_implementation_required: true, browser_manifestation_honest: false, pico_manifestation_honest: false,
        example: "frames: audio/clip-to-frames".into(),
    }
}
pub fn install_audio_clip_frames_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias(
        "PcmClip",
        conduit_core::kind_id(conduit_audio::AUDIO_PCM_CLIP_INFO_ID),
    )?;
    startup.insert_value_kind_alias(
        "PcmFrames",
        conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
    )?;
    let kind = pcm_clip_frames_semantic_contract();
    startup.insert(KindSignature {
        kind: PCM_CLIP_FRAMES_KIND.into(),
        startup_parameters: alloc::vec![],
    })?;
    startup.insert_fore(PCM_CLIP_FRAMES_KIND, kind.checked_front())?;
    profile.insert_kind(kind).map_err(|error| error.to_string())
}
