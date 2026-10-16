//! Shared authored speech/music components, finite execution and voice parity.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "dsp.shared-source-play",
        "Prove owned DSP programs, admitted IPA frication, periodic music, exact state/seed partitions and bounded execution",
        "cargo",
        &["test", "--locked", "-p", "conduit-plot", "-p", "conduit-std-host",
          "--test", "seeded_noise", "--test", "dsp_shared_excitation",
          "--test", "dsp_shared_partition", "--test", "dsp_shared_speech", "--test", "one_pole", "--", "--nocapture"],
    ),
    Step::new(
        "dsp.compact-voice-parity",
        "Preserve the original compact voice phone histories against the frozen scalar reference",
        "cargo",
        &["test", "--locked", "-p", "conduit-plot", "-p", "conduit-std-host", "-p", "conduit-speech",
          "--lib", "frame_history_matches_the_original_checked_scalar_composition", "--", "--nocapture"],
    ),
];
