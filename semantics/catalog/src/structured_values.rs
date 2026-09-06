//! Exact generic entrances for one checked structured Info profile.

pub mod state_value;

#[cfg(feature = "form-catalog")]
use alloc::format;
#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, FaceStartupParameter, KindContractRevision, KindId,
    PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
#[cfg(feature = "form-catalog")]
use conduit_core::{ConfigurationValue, StructuredInfoValue};

pub const STRUCTURED_LITERAL_KIND: &str = "structured-info/literal";
pub const STRUCTURED_PRESENTATION_KIND: &str = "presentation/structured-info";
pub const STRUCTURED_LITERAL_REVISION: &str = "structured-info/literal@1";
pub const STRUCTURED_PRESENTATION_REVISION: &str = "presentation/structured-info@1";
pub const STRUCTURED_PRESENTATION_TARGET: &str = "presentation/structured-info";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredValueContract {
    pub startup_parameters: Vec<FaceStartupParameter>,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

pub fn structured_literal_contract(
    type_name: &str,
    value_type: &StructuredInfoType,
) -> StructuredValueContract {
    contract(type_name, value_type, true)
}

pub fn structured_presentation_contract(
    type_name: &str,
    value_type: &StructuredInfoType,
) -> StructuredValueContract {
    contract(type_name, value_type, false)
}

fn contract(
    type_name: &str,
    value_type: &StructuredInfoType,
    source: bool,
) -> StructuredValueContract {
    let profile = value_type
        .profile()
        .expect("checked structured type has a finite profile");
    let value_kind = profile.value_kind().clone();
    let port = PortDescriptor {
        port_id: port_id(if source { "value" } else { "input" }),
        value_kind: value_kind.clone(),
        direction: if source {
            PortDirection::Output
        } else {
            PortDirection::Input
        },
        temporal: PortTemporal::Value,
    };
    StructuredValueContract {
        startup_parameters: if source {
            vec![FaceStartupParameter {
                name: "value".into(),
                value_type: type_name.into(),
                has_default: false,
            }]
        } else {
            Vec::new()
        },
        kind_id: kind_id(if source {
            STRUCTURED_LITERAL_KIND
        } else {
            STRUCTURED_PRESENTATION_KIND
        }),
        kind_contract_revision: KindContractRevision::from(if source {
            STRUCTURED_LITERAL_REVISION
        } else {
            STRUCTURED_PRESENTATION_REVISION
        }),
        inputs: if source {
            Vec::new()
        } else {
            vec![port.clone()]
        },
        outputs: if source { vec![port] } else { Vec::new() },
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_structured_value_catalogs(
    type_name: &str,
    value_type: &StructuredInfoType,
    default_value: &StructuredInfoValue,
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };

    if default_value.value_type() != value_type {
        return Err("structured literal default has the wrong exact type".into());
    }
    let type_profile = value_type.profile().map_err(|error| format!("{error:?}"))?;
    let canonical = default_value
        .canonical_bytes()
        .map_err(|error| format!("{error:?}"))?;
    let literal = structured_literal_contract(type_name, value_type);
    let presenter = structured_presentation_contract(type_name, value_type);
    startup
        .insert_structured_type(type_name, value_type.clone())
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: STRUCTURED_LITERAL_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "value".into(),
                value_type: type_name.into(),
                default: None,
            }],
        })
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: STRUCTURED_PRESENTATION_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: literal.kind_id,
            kind_contract_revision: literal.kind_contract_revision,
            inputs: literal.inputs,
            outputs: literal.outputs,
            configuration: vec![ConfigurationField {
                key: "value".into(),
                default_value: ConfigurationValue::Structured(
                    conduit_core::StructuredConfigurationValue::new(
                        type_profile.value_kind().clone(),
                        canonical,
                    )
                    .ok_or_else(|| "structured literal default exceeds its bound".to_string())?,
                ),
                validation: ConfigurationRule::Structured {
                    profile: type_profile.value_kind().clone(),
                },
            }],
        })
        .map_err(|error| error.to_string())?;
    profile
        .insert(KindDefinition {
            kind_id: presenter.kind_id,
            kind_contract_revision: presenter.kind_contract_revision,
            inputs: presenter.inputs,
            outputs: presenter.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())
}
