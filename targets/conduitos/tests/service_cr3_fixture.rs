//! Actual fixed formatter and single-consumer snapshot modules.
pub use conduitos::sign_format;
#[path = "../src/ordinary_domain_proof/service_memory/baseline/cr3_format.rs"]
mod cr3_format;
#[path = "../src/arch/x86_64/domain_transition/cr3_observation.rs"]
mod cr3_observation;
#[path = "../src/ordinary_domain_baseline_format.rs"]
mod ordinary_domain_baseline_format;
