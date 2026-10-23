//! Source/link inventory contracts; no VM or service-isolation claim.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "privilege-inventory.contract",
    "Reject stale graphical artifacts and unsupported privilege/isolation claims",
    "cargo",
    &[
        "test",
        "--locked",
        "-p",
        "xtask",
        "--bin",
        "xtask",
        "privilege_inventory::",
        "--",
        "--nocapture",
    ],
)];
