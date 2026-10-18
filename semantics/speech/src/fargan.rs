//! Source-owned FARGAN conditioning and recurrent 40-sample synthesis.
//!
//! Callers explicitly supply admitted immutable tensors, conditioning/history,
//! and recurrent state. Accept `FarganSubframeResult` atomically before advancing
//! state. These sources contain no model parameters or platform bindings.
//! The period/feature producer and PCM device remain separately planned work.

/// Sources must be checked together against the AI fixed numeric catalogues.
pub const SOURCES: [&str; 5] = [
    include_str!("../fargan_conditioning.conduit"),
    include_str!("../fargan_signal.conduit"),
    include_str!("../fargan_pitch_history.conduit"),
    include_str!("../fargan_subframe.conduit"),
    include_str!("../fargan_frame.conduit"),
];
/// A frame-rate conditioner with explicit two-frame history and delayed period.
pub const CONDITIONING_ENTRY: &str = "speech/fargan-conditioning";
/// Full recurrent synthesis and de-emphasis for forty float PCM samples.
pub const SUBFRAME_ENTRY: &str = "speech/fargan-subframe";

/// Conditioner and four sequential recurrent subframes for 160 PCM samples.
pub const FRAME_ENTRY: &str = "speech/fargan-frame";
