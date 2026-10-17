//! Typed deterministic provider models, never physical printer proof.
use crate::process::Step;
pub const STEPS: &[Step] = &[Step::new(
    "print-pool.typed-simulator",
    "Prove typed print eligibility, fresh costs, finite waiting, fairness and provider loss through the ordinary shared pool",
    "cargo",
    &["test", "--locked", "-p", "conduit-planner", "-p", "conduit-process", "--test", "print_pool", "--test", "printing", "--test", "print_registry", "--test", "print_lost_ack"],
)];
