//! Retained State over one exact checked structured-Info specialization.
//!
//! Initialization is authored meaning. Host storage limits and replacement
//! authority are separate admission facts; no implementation is installed here.

use super::StructuredValueContract;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, FaceStartupParameter, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal, StructuredInfoRefusal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub const STATE_VALUE_KIND: &str = "state/value";
pub const STATE_VALUE_REVISION: &str = "conduit.state/value@1";

/// One typed current/next cell. It emits authored initialization, accepts one
/// next value at a time, and completes only when the next input closes. Waiting
/// for input has no predetermined semantic transition count.
pub fn state_value_contract(
    type_name: &str,
    value_type: &StructuredInfoType,
) -> Result<StructuredValueContract, StructuredInfoRefusal> {
    let profile = value_type.profile()?;
    let port = |name, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: profile.value_kind().clone(),
        direction,
        temporal,
    };
    Ok(StructuredValueContract {
        startup_parameters: vec![FaceStartupParameter {
            name: "initial".into(),
            value_type: type_name.into(),
            has_default: false,
        }],
        kind_id: kind_id(STATE_VALUE_KIND),
        kind_contract_revision: KindContractRevision::from(STATE_VALUE_REVISION),
        inputs: vec![port(
            "next",
            PortDirection::Input,
            PortTemporal::Flow { closes: true },
        )],
        outputs: vec![port(
            "current",
            PortDirection::Output,
            PortTemporal::Current,
        )],
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    })
}

#[cfg(feature = "form-catalog")]
mod catalog;
#[cfg(feature = "form-catalog")]
pub use catalog::{
    derive_state_boundary, install_state_value_kind, validate_state_placement,
    StateValueAdmissionError,
};

#[cfg(all(test, feature = "form-catalog"))]
mod tests;
