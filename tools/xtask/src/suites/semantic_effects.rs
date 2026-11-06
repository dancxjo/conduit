//! Existing semantic work laws, checked nested Source and ordinary local Play.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "effects.core",
        "Check independent semantic facts and exact Back eligibility boundaries",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-core",
            "--test",
            "kind_effects",
        ],
    ),
    Step::new(
        "effects.source",
        "Check nested Source inference, provenance, unknowns and annotation refusals",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-plot",
            "--test",
            "semantic_effects",
            "--test",
            "pure_expression_types",
            "--test",
            "semantic_expression_call",
        ],
    ),
    Step::new(
        "effects.play",
        "Execute the inferred pure path and refuse incompatible realization effects",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "local_expression_child",
        ],
    ),
];
