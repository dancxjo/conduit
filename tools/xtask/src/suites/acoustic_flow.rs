//! Deterministic authored acoustic transport proof; no device or physical claim.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "acoustic.authored-kernel",
    "Validate exact packing and execute authored oscillator/resonator state to finite PCM through the production kernel",
    "cargo",
    &[
        "test", "--locked",
        "-p", "conduit-audio", "--test", "pcm16_packing",
        "-p", "conduit-semantic-catalog", "--test", "pcm_packing",
        "-p", "conduit-std-host",
        "--test", "local_expression_child",
        "--test", "value_pair_child",
        "--test", "acoustic_value_child",
        "--test", "acoustic_scan_source",
        "--test", "pcm_packing_back",
        "--test", "acoustic_pcm_parent",
        "--", "--nocapture",
    ],
)];
