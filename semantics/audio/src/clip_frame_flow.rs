//! One finite clip drained into original PCM frames, without device or clock invention.
use alloc::{vec, vec::Vec};
use conduit_core::*;

pub const PCM_CLIP_FRAMES_KIND: &str = "audio/clip-to-frames";
pub const PCM_CLIP_FRAMES_REVISION: &str = "conduit.audio/clip-to-frames@1";
// The current frame Type admits at most 2048 frames, two channels and four
// bytes per sample. This bound includes the original encoded header.
pub const PCM_CLIP_FRAMES_MAXIMUM_OUTPUT_BYTES: u32 =
    crate::PCM_FRAME_HEADER_ENCODED_LEN as u32 + crate::MAXIMUM_PCM_FRAMES_PER_BLOCK as u32 * 2 * 4;

/// Validate the complete clip before its first emission, then emit every
/// original frame in order. Pressure waits without consuming the clip; normal
/// completion follows its last frame. No padding, resampling or tail dropping.
pub fn pcm_clip_frames_semantic_contract() -> Kind {
    let input = PortDescriptor {
        port_id: port_id("clip"),
        value_kind: kind_id(crate::AUDIO_PCM_CLIP_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let output = PortDescriptor {
        port_id: port_id("pcm"),
        value_kind: kind_id(crate::AUDIO_PCM_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(PCM_CLIP_FRAMES_KIND),
        kind_contract_revision: PCM_CLIP_FRAMES_REVISION.into(),
        // Existing opaque PCM codec ports have nominal contracts without
        // optional structured/byte refinements. Their finite codec envelope,
        // queue limits and selected Back budget remain the admission bounds.
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::MirrorsInputTerminal,
        )],
        inputs: vec![input],
        outputs: vec![output],
        configuration: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: crate::MAXIMUM_PCM_CLIP_BYTES as u32,
        },
    }
}
