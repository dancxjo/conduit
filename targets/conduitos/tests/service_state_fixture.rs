//! Owning fixed formatter and consumed register/IRQ snapshot contracts.
pub use conduitos::sign_format;
#[path = "../src/ordinary_domain_baseline_format.rs"]
mod ordinary_domain_baseline_format;
#[path = "../src/ordinary_domain_proof/service_memory/baseline/state_format.rs"]
mod state_format;
#[path = "../src/arch/x86_64/domain_transition/state_observation.rs"]
mod state_observation;
