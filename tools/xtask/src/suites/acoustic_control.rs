//! Existing Audio laws and their installed Host projection evidence.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "acoustic-control.references",
        "Check independent exact acoustic conversion and trajectory references",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-audio",
            "--test",
            "acoustic_quantities",
            "--test",
            "amplitude_power",
            "--test",
            "decibel_projection",
            "--test",
            "quantity_trajectory",
        ],
    ),
    Step::new(
        "acoustic-control.installed",
        "Execute checked acoustic Source through the normal installed Host kernel",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "acoustic_control_installed",
            "--",
            "--nocapture",
        ],
    ),
];
