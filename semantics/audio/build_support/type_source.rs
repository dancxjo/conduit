//! One exact owner Type Source composition for generation and explicit custody preparation.
extern crate alloc;
use alloc::string::String;

pub fn source() -> String {
    [
        include_str!("../types.conduit"),
        include_str!("../trajectory_types.conduit"),
        include_str!("../trajectory_common_basis_types.conduit"),
        include_str!("../rate_types.conduit"),
        include_str!("../decibel_types.conduit"),
        include_str!("../exact_control_types.conduit"),
        include_str!("../time_analysis_types.conduit"),
        include_str!("../dsp/periodic-types.conduit"),
        include_str!("../spectral_analysis_types.conduit"),
        include_str!("../pitch_analysis_types.conduit"),
        include_str!("../pcm_packing_types.conduit"),
        include_str!("../clock_projection_types.conduit"),
        include_str!("../control_projection_types.conduit"),
    ]
    .join("\n")
}
