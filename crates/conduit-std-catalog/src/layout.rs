//! Exact finite layout contracts for portable presenter Backs.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, KindContractRevision, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_presentation::{
    LAYOUT_FRAME_KIND, MAX_LAYOUT_CHILDREN, MAX_LAYOUT_EXTENT, MAX_LAYOUT_FRAME_BYTES,
};

pub const LAYOUT_VIEWPORT_KIND: &str = "layout/viewport";
pub const LAYOUT_INSET_KIND: &str = "layout/inset";
pub const LAYOUT_ROW_KIND: &str = "layout/row";
pub const LAYOUT_COLUMN_KIND: &str = "layout/column";
pub const LAYOUT_STACK_KIND: &str = "layout/stack";
pub const LAYOUT_ALIGN_KIND: &str = "layout/align";
pub const LAYOUT_HOST_OPERATION: &str = "conduit.host/layout-frame-transform@1";
pub const LAYOUT_INPUT_PORT: &str = "frame";
pub const LAYOUT_OUTPUT_PORT: &str = "placements";
pub const WIDTH_KEY: &str = "width";
pub const HEIGHT_KEY: &str = "height";
pub const CHILDREN_KEY: &str = "children";
pub const CHILD_WIDTH_KEY: &str = "child-width";
pub const CHILD_HEIGHT_KEY: &str = "child-height";
pub const INSET_KEY: &str = "inset";
pub const GAP_KEY: &str = "gap";
pub const HORIZONTAL_KEY: &str = "horizontal";
pub const VERTICAL_KEY: &str = "vertical";

const REVISION: &str = "conduit.std/layout-frame@1";
const PROFILE: &str = "conduit.std/layout-frame-kernel@1";
const ARTIFACT: &str = "conduit-std-host/layout-frame@1";
pub const LAYOUT_VIEWPORT_IMPLEMENTATION: &str = "std/layout/viewport-implementation@1";
pub const LAYOUT_INSET_IMPLEMENTATION: &str = "std/layout/inset-implementation@1";
pub const LAYOUT_ROW_IMPLEMENTATION: &str = "std/layout/row-implementation@1";
pub const LAYOUT_COLUMN_IMPLEMENTATION: &str = "std/layout/column-implementation@1";
pub const LAYOUT_STACK_IMPLEMENTATION: &str = "std/layout/stack-implementation@1";
pub const LAYOUT_ALIGN_IMPLEMENTATION: &str = "std/layout/align-implementation@1";

pub fn layout_viewport_contract() -> StandardKindContract {
    contract(LAYOUT_VIEWPORT_KIND, "Layout viewport", "Create one finite available extent and bounded child descriptors.", viewport_fields(), false, "root: layout/viewport(width = 960, height = 540, children = 3, child-width = 120, child-height = 80)")
}
pub fn layout_inset_contract() -> StandardKindContract {
    contract(
        LAYOUT_INSET_KIND,
        "Layout inset",
        "Inset and clip a bounded layout frame.",
        vec![u16_field(INSET_KEY, 8)],
        true,
        "inner: layout/inset(inset = 8)",
    )
}
pub fn layout_row_contract() -> StandardKindContract {
    contract(
        LAYOUT_ROW_KIND,
        "Layout row",
        "Distribute children horizontally with deterministic remainder placement.",
        vec![u16_field(GAP_KEY, 8)],
        true,
        "row: layout/row(gap = 8)",
    )
}
pub fn layout_column_contract() -> StandardKindContract {
    contract(
        LAYOUT_COLUMN_KIND,
        "Layout column",
        "Distribute children vertically with deterministic remainder placement.",
        vec![u16_field(GAP_KEY, 8)],
        true,
        "column: layout/column(gap = 8)",
    )
}
pub fn layout_stack_contract() -> StandardKindContract {
    contract(
        LAYOUT_STACK_KIND,
        "Layout stack",
        "Place every child in the same clipped viewport.",
        Vec::new(),
        true,
        "stack: layout/stack",
    )
}
pub fn layout_align_contract() -> StandardKindContract {
    contract(
        LAYOUT_ALIGN_KIND,
        "Layout align",
        "Align bounded child rectangles on two explicit axes.",
        vec![
            alignment_field(HORIZONTAL_KEY),
            alignment_field(VERTICAL_KEY),
        ],
        true,
        "face: layout/align(horizontal = \"center\", vertical = \"center\")",
    )
}

pub fn layout_viewport_offer() -> CapabilityOffer {
    offer(layout_viewport_contract())
}
pub fn layout_inset_offer() -> CapabilityOffer {
    offer(layout_inset_contract())
}
pub fn layout_row_offer() -> CapabilityOffer {
    offer(layout_row_contract())
}
pub fn layout_column_offer() -> CapabilityOffer {
    offer(layout_column_contract())
}
pub fn layout_stack_offer() -> CapabilityOffer {
    offer(layout_stack_contract())
}
pub fn layout_align_offer() -> CapabilityOffer {
    offer(layout_align_contract())
}

pub fn layout_offer_for(kind: &str) -> Option<CapabilityOffer> {
    Some(match kind {
        LAYOUT_VIEWPORT_KIND => layout_viewport_offer(),
        LAYOUT_INSET_KIND => layout_inset_offer(),
        LAYOUT_ROW_KIND => layout_row_offer(),
        LAYOUT_COLUMN_KIND => layout_column_offer(),
        LAYOUT_STACK_KIND => layout_stack_offer(),
        LAYOUT_ALIGN_KIND => layout_align_offer(),
        _ => return None,
    })
}

fn contract(
    kind: &str,
    name: &str,
    summary: &str,
    configuration: Vec<StandardConfigurationField>,
    input: bool,
    example: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: name.to_string(),
        summary: summary.to_string(),
        inputs: if input {
            vec![port(LAYOUT_INPUT_PORT, PortDirection::Input)]
        } else {
            Vec::new()
        },
        outputs: vec![port(LAYOUT_OUTPUT_PORT, PortDirection::Output)],
        configuration,
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: MAX_LAYOUT_FRAME_BYTES as u32,
        },
        terminal_behavior: if input {
            TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible
        } else {
            TerminalBehavior::EmitsOnce
        },
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: example.to_string(),
    }
}

fn offer(contract: StandardKindContract) -> CapabilityOffer {
    let kind = contract.kind_id.as_str();
    CapabilityOffer {
        startup_parameters: super::functional_face::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(format_id(kind, "capability").as_str()),
        kind_id: contract.kind_id.clone(),
        kind_contract_revision: KindContractRevision::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(implementation_for(kind)),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: if kind == LAYOUT_VIEWPORT_KIND {
            Vec::new()
        } else {
            vec![HostOperationRequirement {
                contract_id: HostOperationContractId::from(LAYOUT_HOST_OPERATION),
                target_kind: Some(contract.kind_id),
                maximum_in_flight: 1,
                maximum_input_bytes: MAX_LAYOUT_FRAME_BYTES as u32,
                maximum_output_bytes: MAX_LAYOUT_FRAME_BYTES as u32,
            }]
        },
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn format_id(kind: &str, suffix: &str) -> alloc::string::String {
    alloc::format!("std/{kind}-{suffix}@1")
}
fn implementation_for(kind: &str) -> &'static str {
    match kind {
        LAYOUT_VIEWPORT_KIND => LAYOUT_VIEWPORT_IMPLEMENTATION,
        LAYOUT_INSET_KIND => LAYOUT_INSET_IMPLEMENTATION,
        LAYOUT_ROW_KIND => LAYOUT_ROW_IMPLEMENTATION,
        LAYOUT_COLUMN_KIND => LAYOUT_COLUMN_IMPLEMENTATION,
        LAYOUT_STACK_KIND => LAYOUT_STACK_IMPLEMENTATION,
        LAYOUT_ALIGN_KIND => LAYOUT_ALIGN_IMPLEMENTATION,
        _ => unreachable!(),
    }
}
fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(LAYOUT_FRAME_KIND),
        direction,
        temporal: PortTemporal::Value,
    }
}
fn u16_field(key: &str, default: u64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::U64(default),
        rule: StandardConfigurationRule::U64Range {
            minimum: 0,
            maximum: u64::from(MAX_LAYOUT_EXTENT),
        },
    }
}
fn alignment_field(key: &str) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Text("start".into()),
        rule: StandardConfigurationRule::TextOneOf {
            values: vec!["start".to_string(), "center".to_string(), "end".to_string()],
        },
    }
}
fn viewport_fields() -> Vec<StandardConfigurationField> {
    vec![
        u16_field(WIDTH_KEY, 960),
        u16_field(HEIGHT_KEY, 540),
        StandardConfigurationField {
            key: CHILDREN_KEY.to_string(),
            default_value: ConfigurationValue::U64(1),
            rule: StandardConfigurationRule::U64Range {
                minimum: 0,
                maximum: MAX_LAYOUT_CHILDREN as u64,
            },
        },
        u16_field(CHILD_WIDTH_KEY, 120),
        u16_field(CHILD_HEIGHT_KEY, 80),
    ]
}

#[cfg(feature = "form-catalog")]
pub fn install_layout_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    for contract in [
        layout_viewport_contract(),
        layout_inset_contract(),
        layout_row_contract(),
        layout_column_contract(),
        layout_stack_contract(),
        layout_align_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: match field.default_value {
                        ConfigurationValue::Text(_) => "Text",
                        _ => "Count",
                    }
                    .to_string(),
                    default: Some(match &field.default_value {
                        ConfigurationValue::U64(value) => value.to_string(),
                        ConfigurationValue::Text(value) => alloc::format!("{value:?}"),
                        _ => unreachable!(),
                    }),
                })
                .collect(),
        })?;
        let configuration = contract
            .configuration
            .into_iter()
            .map(|field| ConfigurationField {
                key: field.key,
                default_value: field.default_value,
                validation: match field.rule {
                    StandardConfigurationRule::U64Range { minimum, maximum } => {
                        ConfigurationRule::U64Range { minimum, maximum }
                    }
                    StandardConfigurationRule::TextOneOf { values } => {
                        ConfigurationRule::TextOneOf { values }
                    }
                    _ => unreachable!(),
                },
            })
            .collect();
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(REVISION),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration,
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
