//! Finite platform handoff only; no Zenoh session, protected Line or MCU proof.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "zenoh-platform.fmt",
        "Check the platform handoff source formatting",
        "cargo",
        &["fmt", "-p", "conduit-zenoh-pico-platform", "--", "--check"],
    ),
    Step::new(
        "zenoh-platform.test",
        "Check pressure, socket retirement and stale completion refusal",
        "cargo",
        &["test", "--locked", "-p", "conduit-zenoh-pico-platform"],
    ),
    Step::new(
        "zenoh-platform.lint",
        "Lint the no_std finite socket handoff",
        "cargo",
        &[
            "clippy",
            "--locked",
            "-p",
            "conduit-zenoh-pico-platform",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
    Step::new(
        "zenoh-platform.embassy-fmt",
        "Check the separately locked Embassy target adapter",
        "cargo",
        &[
            "fmt",
            "--manifest-path",
            "targets/embassy/zenoh-pico-platform/Cargo.toml",
            "--",
            "--check",
        ],
    ),
    Step::new(
        "zenoh-platform.embassy-test",
        "Exercise real Embassy TCP over finite in-memory packet queues",
        "cargo",
        &[
            "test",
            "--locked",
            "--manifest-path",
            "targets/embassy/zenoh-pico-platform/Cargo.toml",
            "--test",
            "socket_owner",
        ],
    ),
    Step::new(
        "zenoh-platform.embassy-lint",
        "Lint target adapter and actual-socket test",
        "cargo",
        &[
            "clippy",
            "--locked",
            "--manifest-path",
            "targets/embassy/zenoh-pico-platform/Cargo.toml",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    ),
];
