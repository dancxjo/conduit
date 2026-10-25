//! Actual pure entry-policy source with real shared types. This does not execute
//! Budget::arm, RTC IRQs, assembly entry or the ServiceDomain integration.
mod protected_region {
    pub use conduitos::protected_region::{DomainCost, DomainFault, DomainRefusal, DomainReturn};
}
mod service_memory {
    pub use conduitos::service_memory::ServiceProfile;
}
#[path = "../src/arch"]
mod arch {
    pub mod x86_64 {
        pub mod domain_budget {
            #[path = "entry.rs"]
            mod entry;
            #[path = "fault_pc.rs"]
            mod fault_pc;
        }
    }
}
