//! Shared authored speech/music components, finite execution and voice parity.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "dsp.checked-package",
        "Check the owned DSP package and typed source exports before broad installed regression",
        "cargo",
        &["test", "--locked", "-p", "conduit-plot", "-p", "conduit-std-host", "-p", "conduit-audio", "-p", "conduit-semantic-catalog", "--test", "dsp_package", "--test", "dsp_public_catalog", "--", "--nocapture"],
    ),
    Step::new(
        "dsp.shared-source-play",
        "Prove owned DSP programs, admitted IPA frication, periodic music, exact state/seed partitions and bounded execution",
        "cargo",
        &["test", "--locked", "-p", "conduit-plot", "-p", "conduit-std-host", "-p", "conduit-audio", "-p", "conduit-semantic-catalog",
          "--test", "seeded_noise", "--test", "dsp_shared_excitation",
          "--test", "dsp_shared_partition", "--test", "dsp_shared_speech", "--test", "one_pole", "--test", "dsp_package_installed", "--test", "dsp_package_frames", "--", "--nocapture"],
    ),
    Step::new(
        "dsp.product-authoring",
        "Prove product Source catalogs, existing authoring boundaries and public DSP Source Backs",
        "cargo",
        &["test", "--locked", "-p", "conduit", "--lib", "plot_source::tests", "--", "--nocapture"],
    ),
    Step::new(
        "dsp.compact-voice-parity",
        "Preserve the original compact voice phone histories against the frozen scalar reference",
        "cargo",
        &["test", "--locked", "-p", "conduit-plot", "-p", "conduit-std-host", "-p", "conduit-speech",
          "--lib", "frame_history_matches_the_original_checked_scalar_composition", "--", "--nocapture"],
    ),
];
