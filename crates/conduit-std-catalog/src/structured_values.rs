//! Exact generic entrances for one checked structured Info profile.

#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::{format, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, resource_requirement, ArtifactId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId, FaceStartupParameter,
    ImplementationId, ImplementationOffer, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    PRESENTATION_RESOURCE_CLASS,
};
#[cfg(feature = "form-catalog")]
use conduit_core::{ConfigurationValue, StructuredInfoValue};

pub const STRUCTURED_LITERAL_KIND: &str = "structured-info/literal";
pub const STRUCTURED_PRESENTATION_KIND: &str = "presentation/structured-info";
pub const STRUCTURED_LITERAL_REVISION: &str = "structured-info/literal@1";
pub const STRUCTURED_PRESENTATION_REVISION: &str = "presentation/structured-info@1";
pub const STRUCTURED_LITERAL_STD_PROFILE: &str = "std/structured-literal-kernel@1";
pub const STRUCTURED_PRESENTATION_STD_PROFILE: &str = "std/structured-presentation-kernel@1";
pub const STRUCTURED_LITERAL_STD_IMPLEMENTATION: &str = "std/kernel-structured-literal@1";
pub const STRUCTURED_PRESENTATION_STD_IMPLEMENTATION: &str = "std/kernel-structured-presentation@1";
pub const STRUCTURED_LITERAL_STD_ARTIFACT: &str = "conduit-core/structured-info@1";
pub const STRUCTURED_PRESENTATION_STD_ARTIFACT: &str = "conduit-presentation/structured-info@1";
pub const STRUCTURED_PRESENTATION_TARGET: &str = "presentation/structured-info";

pub fn structured_literal_std_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
) -> CapabilityOffer {
    offer(type_name, value_type, true)
}

pub fn structured_presentation_std_offer(
    type_name: &str,
    value_type: &StructuredInfoType,
) -> CapabilityOffer {
    offer(type_name, value_type, false)
}

fn offer(type_name: &str, value_type: &StructuredInfoType, source: bool) -> CapabilityOffer {
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
    CapabilityOffer {
        startup_parameters: if source {
            vec![FaceStartupParameter {
                name: "value".into(),
                value_type: type_name.into(),
                has_default: false,
            }]
        } else {
            Vec::new()
        },
        shorthand: None,
        capability_id: CapabilityId::from(format!(
            "std-{}-{}",
            if source {
                "structured-literal"
            } else {
                "structured-presentation"
            },
            profile.value_kind().as_str()
        )),
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
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(if source {
                STRUCTURED_LITERAL_STD_PROFILE
            } else {
                STRUCTURED_PRESENTATION_STD_PROFILE
            }),
            implementation_id: ImplementationId::from(if source {
                STRUCTURED_LITERAL_STD_IMPLEMENTATION
            } else {
                STRUCTURED_PRESENTATION_STD_IMPLEMENTATION
            }),
            artifact_id: ArtifactId::from(if source {
                STRUCTURED_LITERAL_STD_ARTIFACT
            } else {
                STRUCTURED_PRESENTATION_STD_ARTIFACT
            }),
        },
        inputs: if source {
            Vec::new()
        } else {
            vec![port.clone()]
        },
        outputs: if source { vec![port] } else { Vec::new() },
        host_operations: if source {
            Vec::new()
        } else {
            vec![present_host_operation_requirement(
                kind_id(STRUCTURED_PRESENTATION_TARGET),
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            )]
        },
        resource_requirements: if source {
            Vec::new()
        } else {
            vec![resource_requirement(PRESENTATION_RESOURCE_CLASS, 1)]
        },
        authority_requirements: Vec::new(),
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
    let literal = structured_literal_std_offer(type_name, value_type);
    let presenter = structured_presentation_std_offer(type_name, value_type);
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
