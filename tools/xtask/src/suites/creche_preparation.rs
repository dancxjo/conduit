//! Ordinary preparation contracts and trusted-source Linux supervision.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "creche-preparation.contract",
        "Check typed Source and actual transactional kernel preparation",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-body-make",
            "--test",
            "deployment_runtime_contract",
            "--test",
            "deployment_runtime_kernel",
        ],
    ),
    Step::new(
        "creche-preparation.linux",
        "Check reviewed Linux source builder refusal and supervision",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-host-hosted",
            "--lib",
            "linux_builder::",
        ],
    ),
    Step::new(
        "creche-preparation.product-source",
        "Check ordinary Crèche through the product Source catalogue",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit",
            "--bin",
            "conduit",
            "plot_source::creche_tests::",
        ],
    ),
];
