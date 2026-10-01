//! Retained State over one exact checked structured-Info specialization.
//!
//! Initialization is authored meaning. Host storage limits and replacement
//! authority are separate admission facts; no implementation is installed here.

use super::StructuredValueContract;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, FrontStartupParameter, Kind,
    KindConfigurationField, KindConfigurationRule, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredConfigurationValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

pub use conduit_core::{STATE_VALUE_KIND, STATE_VALUE_REVISION};

/// One typed current/next cell. It emits authored initialization, accepts one
/// next value per activation, and retains the resulting current value. Waiting
/// for another activation has no predetermined semantic transition count.
pub fn state_value_contract(
    _type_name: &str,
    value_type: &StructuredInfoType,
) -> Result<StructuredValueContract, StructuredInfoRefusal> {
    let profile = value_type.profile()?;
    let value_kind = match value_type.shape() {
        conduit_core::StructuredInfoTypeShape::Leaf(kind) => kind.clone(),
        _ => profile.value_kind().clone(),
    };
    let port = |name, direction, temporal| PortDescriptor {
        port_id: port_id(name),
        value_kind: value_kind.clone(),
        direction,
        temporal,
        abnormal_kind: None,
    };
    Ok(StructuredValueContract {
        startup_parameters: vec![FrontStartupParameter {
            name: "initial".into(),
            value_type: profile.value_kind().clone(),
            has_default: false,
        }],
        kind_id: kind_id(STATE_VALUE_KIND),
        kind_contract_revision: KindIdentity::from(STATE_VALUE_REVISION),
        inputs: vec![port("next", PortDirection::Input, PortTemporal::Value)],
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

pub fn state_value_semantic_contract(
    type_name: &str,
    value_type: &StructuredInfoType,
    default_value: &StructuredInfoValue,
) -> Result<Kind, alloc::string::String> {
    if default_value.value_type() != value_type {
        return Err("State initialization has the wrong exact structured type".into());
    }
    let mut kind: Kind = state_value_contract(type_name, value_type)
        .map_err(|error| alloc::format!("{error:?}"))?
        .into();
    let profile = value_type
        .profile()
        .map_err(|error| alloc::format!("{error:?}"))?;
    let initial = StructuredConfigurationValue::new(
        profile.value_kind().clone(),
        default_value
            .canonical_bytes()
            .map_err(|error| alloc::format!("{error:?}"))?,
    )
    .ok_or_else(|| alloc::string::String::from("invalid finite State initialization"))?;
    kind.configuration = vec![KindConfigurationField {
        key: "initial".into(),
        rule: KindConfigurationRule::Structured {
            profile: initial.profile().clone(),
        },
        default_value: ConfigurationValue::Structured(initial),
    }];
    Ok(kind)
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
