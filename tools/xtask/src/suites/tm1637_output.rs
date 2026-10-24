//! Finite electrical output possession; pins, admission and authority are fixtures.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "tm1637-output.wire-only",
        "Check the portable wire driver without native ownership dependencies",
        "cargo",
        &["check", "--locked", "--manifest-path", "mechanisms/io/tm1637/Cargo.toml", "--no-default-features", "--lib"],
    ),
    Step::new(
        "tm1637-output.contract",
        "Verify exact selected and assigned output, finite kernel execution and electrical failures",
        "cargo",
        &["test", "--locked", "--manifest-path", "mechanisms/io/tm1637/Cargo.toml", "--features", "host-call"],
    ),
    Step::new(
        "tm1637-output.lint",
        "Lint output ownership, wire transport and its contract fixtures",
        "cargo",
        &["clippy", "--locked", "--manifest-path", "mechanisms/io/tm1637/Cargo.toml", "--features", "host-call", "--all-targets", "--", "-D", "warnings"],
    ),
    Step::new(
        "tm1637-output.source",
        "Execute the prepared Source packet encoder and exact output codec contract",
        "cargo",
        &["test", "--locked", "--manifest-path", "bodies/environment/runtime/Cargo.toml", "--test", "display_packets"],
    ),
    Step::new(
        "tm1637-output.source-lint",
        "Lint prepared Source packet encoding and its allocation proof",
        "cargo",
        &["clippy", "--locked", "--manifest-path", "bodies/environment/runtime/Cargo.toml", "--test", "display_packets", "--", "-D", "warnings"],
    ),
];
