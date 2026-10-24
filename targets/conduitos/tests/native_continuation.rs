//! Hosted controls for the actual native continuation custody module.
//! Native IRQ capture, installed mapping admission and resume remain separate.
mod protected_region {
    pub use conduitos::protected_region::RegionBinding;
}
mod protection_domain {
    pub use conduitos::protection_domain::ProtectionDomainId;
}
#[path = "../src/arch/x86_64/domain_transition/continuation.rs"]
mod continuation;
