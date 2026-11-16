//! Checked observation meaning and decoding, without runtime or platform claims.
use crate::process::Step;

pub const STEPS: &[Step] = &[
    Step::new(
        "ambient-observations.source",
        "Check finite authored observations, rotation and native owner contracts",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-semantic-catalog",
            "--features",
            "plot-catalog",
            "--test",
            "ambient_display_plots",
        ],
    ),
    Step::new(
        "ambient-observations.weather",
        "Check bounded supplied weather responses and explicit freshness policy",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--lib",
            "ambient_weather::tests",
        ],
    ),
];
