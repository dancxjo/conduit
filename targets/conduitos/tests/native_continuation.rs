//! Hosted controls for the actual native continuation custody module.
//! Native IRQ capture, installed mapping admission and resume remain separate.
mod protected_region {
    pub use conduitos::protected_region::RegionBinding;
}
mod protection_domain {
    pub use conduitos::protection_domain::ProtectionDomainId;
}
// Preserve the production module ancestry and its restricted visibility while
// compiling the actual custody and scoped IRQ-sink source on the hosted target.
#[path = "../src/arch"]
mod arch {
    pub mod x86_64 {
        pub mod domain_transition {
            #[path = "capture.rs"]
            mod capture;
            #[path = "continuation.rs"]
            mod continuation;
        }
    }
}
