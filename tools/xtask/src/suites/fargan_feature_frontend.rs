//! Public analysis coefficients and checked Source/generic DSP conformance.
use crate::process::Step;

pub const STEPS: &[Step] = &[
    Step::new(
        "fargan-feature-front-end.coefficients",
        "Regenerate and verify pinned public analysis coefficients without downloads",
        "python3",
        &[
            "proof/fargan/feature-profile/generate_coefficients.py",
            "proof/fargan/feature-profile/generated",
        ],
    ),
    Step::new(
        "fargan-feature-front-end.source-dsp",
        "Compare checked preprocessing and generic DSP against independent equations",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-ai",
            "--features",
            "kernel-operation-owners,hosted-catalog-cache",
            "--test",
            "fargan_feature_frontend",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
];
