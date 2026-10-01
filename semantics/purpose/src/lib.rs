#![no_std]

extern crate alloc;

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));
}

pub use generated::{
    FulfillmentReadiness, FulfillmentReadinessDisposition, FulfillmentReadinessReasonIdentities,
    PurposeEvidenceSignIdentities, PurposeObligation, PurposeObligationState, PurposeObligations,
    PurposeState,
};

pub fn purpose_state_type() -> conduit_core::StructuredInfoType {
    PurposeState::semantic_type().expect("checked native PurposeState Type")
}

pub fn fulfillment_readiness_type() -> conduit_core::StructuredInfoType {
    FulfillmentReadiness::semantic_type().expect("checked native FulfillmentReadiness Type")
}
