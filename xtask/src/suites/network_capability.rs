use crate::{process::Step, proof::ProofClass};

pub const NETWORK_CAPABILITY_STEPS: &[Step] = &[
    Step::new(
        "check.no-std.net",
        "Network capability no-default-features check",
        "cargo",
        &["check", "-p", "conduit-net", "--no-default-features"],
    ),
    Step::typed(
        "check.thumb.net",
        "Network capability Thumb target check",
        "cargo",
        &[
            "check",
            "-p",
            "conduit-net",
            "--no-default-features",
            "--target",
            "thumbv6m-none-eabi",
        ],
        None,
        Some("thumbv6m-none-eabi"),
        Some(ProofClass::ContractCompile),
        &[],
    ),
];
