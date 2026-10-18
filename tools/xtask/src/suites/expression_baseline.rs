//! Source-pinned native diagnostics and executable portable-WASM parity.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "expression.baseline",
        "Report fixed 1/3/8/32 prepared expression parity and cold/warm timings",
        "cargo",
        &[
            "test",
            "-p",
            "conduit-plot",
            "--test",
            "expression_baseline",
            "--locked",
            "--",
            "--nocapture",
        ],
    ),
    Step::new(
        "expression.wasm-build",
        "Compile the portable scalar-tape proof for WASM",
        "cargo",
        &[
            "build",
            "--locked",
            "-p",
            "conduit-plot",
            "--example",
            "scalar_tape_wasm",
            "--target",
            "wasm32-unknown-unknown",
        ],
    ),
    Step::new(
        "expression.wasm-parity",
        "Execute integer/Boolean parity, refusals and allocation checks in WASM",
        "node",
        &["proof/expression/scalar-tape-wasm.mjs"],
    ),
];
