//! Hosted/std text realization offers and catalog descriptions.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityOffer, ExecutionProfileId,
    FaceStartupParameter, ImplementationId,
};

pub const TEXT_LITERAL_EXECUTION_PROFILE: &str = "conduit.std/text-literal-kernel-hosted@1";
pub const TEXT_LITERAL_IMPLEMENTATION: &str = "std/kernel-text-literal@1";
pub const TEXT_LITERAL_ARTIFACT: &str = "conduit-std-host/text-literal@1";
pub const TEXT_LITERAL_CAPABILITY: &str = "text-literal-v1";
pub const TEXT_UPPER_EXECUTION_PROFILE: &str = "conduit.std/text-upper-kernel-hosted@1";
pub const TEXT_UPPER_IMPLEMENTATION: &str = "std/kernel-text-upper@1";
pub const TEXT_UPPER_ARTIFACT: &str = "conduit-std-host/text-upper@1";
pub const TEXT_UPPER_CAPABILITY: &str = "text-upper-v1";
pub const TEXT_UPPER_HOST_OPERATION_CONTRACT: &str = "conduit.host/text-upper@1";
pub const TEXT_UPPER_HOST_OPERATION_TARGET: &str = "text/uppercase-utf8";
pub const TEXT_JOIN_EXECUTION_PROFILE: &str = "conduit.std/text-join-kernel-hosted@1";
pub const TEXT_JOIN_IMPLEMENTATION: &str = "std/kernel-text-join@1";
pub const TEXT_JOIN_ARTIFACT: &str = "conduit-std-host/text-join@1";
pub const TEXT_JOIN_CAPABILITY: &str = "text-join-v1";
pub const TEXT_JOIN_HOST_OPERATION_CONTRACT: &str = "conduit.host/text-join@1";
pub const TEXT_JOIN_HOST_OPERATION_TARGET: &str = "text/prefix-concat-utf8";
pub const CONDUITOS_BOUNDED_HOST_OP_PROFILE: &str = "conduitos/bounded-host-operations@1";
pub const CONDUITOS_BOUNDED_HOST_OP_ARTIFACT: &str = "conduitos/bounded-host-operations@1";
pub const CONDUITOS_TEXT_JOIN_CAPABILITY: &str = "conduitos-text-join-v1";
pub const CONDUITOS_TEXT_JOIN_IMPLEMENTATION: &str = "conduitos/kernel-text-join@1";

pub(crate) fn text_literal_contract() -> StandardKindContract {
    describe(
        conduit_text::text_literal_semantics(),
        "Text literal",
        "Emit one bounded immutable UTF-8 startup value.",
        TerminalBehavior::EmitsOnce,
        "\"Hello\" > presentation/text",
    )
}

pub(crate) fn text_upper_contract() -> StandardKindContract {
    describe(
        conduit_text::text_upper_semantics(),
        "Uppercase text",
        "Uppercase one bounded stream of UTF-8 text values.",
        TerminalBehavior::MirrorsInputTerminal,
        "upper: text/upper",
    )
}

pub(crate) fn text_join_contract() -> StandardKindContract {
    describe(
        conduit_text::text_join_semantics(),
        "Prefix text",
        "Prepend one immutable bounded UTF-8 prefix without an implicit separator.",
        TerminalBehavior::MirrorsInputTerminal,
        "join: text/join(\"Hello\")",
    )
}

pub fn text_literal_offer() -> CapabilityOffer {
    offer(
        conduit_text::text_literal_semantics(),
        TEXT_LITERAL_CAPABILITY,
        TEXT_LITERAL_EXECUTION_PROFILE,
        TEXT_LITERAL_IMPLEMENTATION,
        TEXT_LITERAL_ARTIFACT,
        vec![FaceStartupParameter {
            name: "value".to_string(),
            value_type: "Text".to_string(),
            has_default: false,
        }],
        None,
    )
}

pub fn text_upper_offer() -> CapabilityOffer {
    let mut offer = offer(
        conduit_text::text_upper_semantics(),
        TEXT_UPPER_CAPABILITY,
        TEXT_UPPER_EXECUTION_PROFILE,
        TEXT_UPPER_IMPLEMENTATION,
        TEXT_UPPER_ARTIFACT,
        Vec::new(),
        Some((port_id("text"), port_id("text"))),
    );
    offer
        .host_operations
        .push(conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(
                TEXT_UPPER_HOST_OPERATION_CONTRACT,
            ),
            target_kind: Some(kind_id(TEXT_UPPER_HOST_OPERATION_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
            maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
        });
    offer
}

pub fn text_join_offer() -> CapabilityOffer {
    let mut offer = offer(
        conduit_text::text_join_semantics(),
        TEXT_JOIN_CAPABILITY,
        TEXT_JOIN_EXECUTION_PROFILE,
        TEXT_JOIN_IMPLEMENTATION,
        TEXT_JOIN_ARTIFACT,
        vec![FaceStartupParameter {
            name: "prefix".to_string(),
            value_type: "Text".to_string(),
            has_default: false,
        }],
        Some((port_id("text"), port_id("text"))),
    );
    offer
        .host_operations
        .push(conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(
                TEXT_JOIN_HOST_OPERATION_CONTRACT,
            ),
            target_kind: Some(kind_id(TEXT_JOIN_HOST_OPERATION_TARGET)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_text::MAX_TEXT_BYTES,
            maximum_output_bytes: conduit_text::MAX_TEXT_BYTES,
        });
    offer
}

pub fn conduitos_text_join_offer() -> CapabilityOffer {
    conduitos_bounded_host_operation_offer(
        text_join_offer(),
        CONDUITOS_TEXT_JOIN_CAPABILITY,
        CONDUITOS_TEXT_JOIN_IMPLEMENTATION,
    )
}

pub(crate) fn conduitos_bounded_host_operation_offer(
    mut offer: CapabilityOffer,
    capability: &str,
    implementation: &str,
) -> CapabilityOffer {
    offer.capability_id = CapabilityId::from(capability);
    offer.implementation.execution_profile_id =
        ExecutionProfileId::from(CONDUITOS_BOUNDED_HOST_OP_PROFILE);
    offer.implementation.implementation_id = ImplementationId::from(implementation);
    offer.implementation.artifact_id = ArtifactId::from(CONDUITOS_BOUNDED_HOST_OP_ARTIFACT);
    offer
}

#[cfg(feature = "form-catalog")]
pub fn install_text_pipeline_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{
        ConfigurationField, ConfigurationRule, KindDefinition, KindSignature,
        StartupParameterSignature,
    };
    conduit_text::install_text_catalogs(startup, profile)?;
    startup.insert(KindSignature {
        kind: super::TEXT_PRESENTATION_KIND.to_string(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-values".to_string(),
            value_type: "Count".to_string(),
            default: Some(super::MAX_TEXT_VALUES.to_string()),
        }],
    })?;
    let presentation = super::text_presentation_contract();
    profile
        .insert(KindDefinition {
            kind_id: presentation.kind_id,
            kind_contract_revision: conduit_core::KindContractRevision::from(
                super::TEXT_PRESENTATION_CONTRACT_REVISION,
            ),
            inputs: presentation.inputs,
            outputs: presentation.outputs,
            configuration: vec![ConfigurationField {
                key: "maximum-values".to_string(),
                default_value: conduit_core::ConfigurationValue::U64(super::MAX_TEXT_VALUES),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: super::MAX_TEXT_VALUES,
                },
            }],
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn describe(
    contract: conduit_text::TextKindContract,
    plain_name: &str,
    summary: &str,
    terminal_behavior: TerminalBehavior,
    example: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: contract.kind_id,
        plain_name: plain_name.to_string(),
        summary: summary.to_string(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract
            .configuration
            .into_iter()
            .map(|field| StandardConfigurationField {
                key: field.key.to_string(),
                default_value: field.default_value,
                rule: StandardConfigurationRule::TextBytes {
                    maximum: field.maximum_text_bytes,
                },
            })
            .collect(),
        limits: contract.limits,
        terminal_behavior,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: example.to_string(),
    }
}

#[allow(clippy::too_many_arguments)]
fn offer(
    contract: conduit_text::TextKindContract,
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    startup_parameters: Vec<FaceStartupParameter>,
    shorthand: Option<(conduit_core::PortId, conduit_core::PortId)>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters,
        shorthand,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn std_offers_consume_the_exact_portable_text_faces() {
        let semantic = conduit_text::text_upper_semantics();
        let offer = text_upper_offer();
        assert_eq!(offer.kind_id, semantic.kind_id);
        assert_eq!(
            offer.kind_contract_revision,
            semantic.kind_contract_revision
        );
        assert_eq!(offer.inputs, semantic.inputs);
        assert_eq!(offer.outputs, semantic.outputs);
        assert_eq!(offer.limits, semantic.limits);
    }
}
