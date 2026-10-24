//! Exact reference info equality, without resource read authority.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "resource-reference-equality.values",
        "Check exact reference equality, malformed refusals and prepared allocation stability",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-plot",
            "--test",
            "resource_reference_equality",
            "--test",
            "structured_expression_equality",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "resource-reference-equality.kernel",
        "Execute checked reference correlation through the installed ordinary kernel",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-std-host",
            "--test",
            "resource_reference_equality",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
];
