//! Text-family browser capability installations.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, FaceStartupParameter, HostOperationContractId, HostOperationRequirement,
    ImplementationId, PlannedGear, PRESENTATION_RESOURCE_CLASS,
};
use conduit_kernel::{HostedValueStore, ValueStorage};

const ARTIFACT: &str = "conduit-browser-runtime/installed-text@1";
const LITERAL_IMPLEMENTATION: &str = "browser/kernel-text-literal@1";
const UPPER_IMPLEMENTATION: &str = "browser/kernel-text-upper@1";
const JOIN_IMPLEMENTATION: &str = "browser/kernel-text-join@1";
const PRESENTATION_IMPLEMENTATION: &str = "browser/presentation-text@1";
const UPPER_OPERATION: &str = "conduit.host/browser-text-upper@1";
const JOIN_OPERATION: &str = "conduit.host/browser-text-join@1";
const PRESENT_OPERATION: &str = "conduit.host/browser-present-text@1";

pub(super) static LITERAL: BrowserInstallation = BrowserInstallation {
    implementation_id: LITERAL_IMPLEMENTATION,
    offer: literal_offer,
    prepare: prepare_literal,
    perform: None,
};

pub(super) static UPPER: BrowserInstallation = BrowserInstallation {
    implementation_id: UPPER_IMPLEMENTATION,
    offer: upper_offer,
    prepare: prepare_unary,
    perform: Some(perform_upper),
};

pub(super) static JOIN: BrowserInstallation = BrowserInstallation {
    implementation_id: JOIN_IMPLEMENTATION,
    offer: join_offer,
    prepare: prepare_unary,
    perform: Some(perform_join),
};

pub(super) static PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: PRESENTATION_IMPLEMENTATION,
    offer: presentation_offer,
    prepare: prepare_presentation,
    perform: Some(perform_presentation),
};

fn literal_offer() -> CapabilityOffer {
    let contract = conduit_text::text_literal_semantics();
    offer(
        contract,
        "browser/text-literal@1",
        "browser/kernel-text-literal@1",
        LITERAL_IMPLEMENTATION,
        vec![FaceStartupParameter {
            name: "value".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        None,
    )
}

fn upper_offer() -> CapabilityOffer {
    let contract = conduit_text::text_upper_semantics();
    let mut offer = offer(
        contract,
        "browser/text-upper@1",
        "browser/kernel-text-upper@1",
        UPPER_IMPLEMENTATION,
        Vec::new(),
        Some((port_id("text"), port_id("text"))),
    );
    offer.host_operations.push(operation(
        UPPER_OPERATION,
        "text/uppercase-utf8",
        conduit_text::MAX_TEXT_BYTES,
        conduit_text::MAX_TEXT_BYTES,
    ));
    offer
}

fn join_offer() -> CapabilityOffer {
    let contract = conduit_text::text_join_semantics();
    let mut offer = offer(
        contract,
        "browser/text-join@1",
        "browser/kernel-text-join@1",
        JOIN_IMPLEMENTATION,
        vec![FaceStartupParameter {
            name: "prefix".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        Some((port_id("text"), port_id("text"))),
    );
    offer.host_operations.push(operation(
        JOIN_OPERATION,
        "text/prefix-concat-utf8",
        conduit_text::MAX_TEXT_BYTES,
        conduit_text::MAX_TEXT_BYTES,
    ));
    offer
}

fn presentation_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::text_presentation_contract();
    let mut offer = CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "maximum-values".into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from("browser/text-presentation@1"),
        kind_id: contract.kind_id,
        kind_contract_revision: conduit_core::KindContractRevision::from(
            conduit_semantic_catalog::TEXT_PRESENTATION_CONTRACT_REVISION,
        ),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/presentation-text@1"),
            implementation_id: ImplementationId::from(PRESENTATION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![operation(
            PRESENT_OPERATION,
            "presentation/browser-text",
            conduit_text::MAX_TEXT_BYTES,
            0,
        )],
        resource_requirements: vec![conduit_core::resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    };
    offer.limits.max_queue_bytes = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32;
    offer
}

fn offer(
    contract: conduit_text::TextKindContract,
    capability: &str,
    profile: &str,
    implementation: &str,
    startup_parameters: Vec<FaceStartupParameter>,
    shorthand: Option<(conduit_core::PortId, conduit_core::PortId)>,
) -> CapabilityOffer {
    let mut offer = CapabilityOffer {
        startup_parameters,
        shorthand,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    };
    offer.limits.max_queue_bytes = conduit_text::MAXIMUM_MORSE_PATTERN_BYTES as u32;
    offer
}

fn operation(
    contract: &str,
    target: &str,
    maximum_input_bytes: u32,
    maximum_output_bytes: u32,
) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: Some(kind_id(target)),
        maximum_in_flight: 1,
        maximum_input_bytes,
        maximum_output_bytes,
    }
}

fn prepare_literal(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &literal_offer())?;
    let value = text_configuration(placement, "value")?;
    let stored = values.store(value.as_bytes()).map_err(debug_error)?;
    Ok(BrowserOperation::source(stored))
}

fn prepare_unary(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    let offer = match placement.implementation_id.as_str() {
        UPPER_IMPLEMENTATION => upper_offer(),
        JOIN_IMPLEMENTATION => join_offer(),
        _ => return Err("unknown text transform installation".into()),
    };
    validate_placement(placement, &offer)?;
    Ok(BrowserOperation::unary(
        placement.host_operations[0].maximum_input_bytes,
        1,
    ))
}

fn prepare_presentation(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &presentation_offer())?;
    Ok(BrowserOperation::presentation(
        placement.host_operations[0].maximum_input_bytes,
        maximum_values(placement)?,
    ))
}

fn perform_upper(_placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let text = core::str::from_utf8(input).map_err(|_| "text/upper input is not UTF-8")?;
    let output = text.to_uppercase().into_bytes();
    bounded_text_result(output)
}

fn perform_join(placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let input = core::str::from_utf8(input).map_err(|_| "text/join input is not UTF-8")?;
    let prefix = text_configuration(placement, "prefix")?;
    let mut output = String::with_capacity(prefix.len().saturating_add(input.len()));
    output.push_str(prefix);
    output.push_str(input);
    bounded_text_result(output.into_bytes())
}

fn perform_presentation(
    _placement: &PlannedGear,
    input: &[u8],
) -> Result<BrowserHostResult, String> {
    core::str::from_utf8(input).map_err(|_| "presentation/text input is not UTF-8")?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::TEXT_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn bounded_text_result(output: Vec<u8>) -> Result<BrowserHostResult, String> {
    if output.len() > conduit_text::MAX_TEXT_BYTES as usize {
        return Err("browser text transform exceeded its semantic byte bound".into());
    }
    Ok(BrowserHostResult {
        output: Some(output),
        manifestation: None,
    })
}

fn text_configuration<'a>(placement: &'a PlannedGear, key: &str) -> Result<&'a str, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (entry_key, ConfigurationValue::Text(value)) if entry_key == key => {
                Some(value.as_str())
            }
            _ => None,
        })
        .filter(|value| value.len() <= conduit_text::MAX_TEXT_BYTES as usize)
        .ok_or_else(|| format!("browser text Gear is missing bounded '{key}' configuration"))
}

fn maximum_values(placement: &PlannedGear) -> Result<u32, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-values", ConfigurationValue::U64(value)) => u32::try_from(*value).ok(),
            _ => None,
        })
        .filter(|value| {
            *value > 0 && u64::from(*value) <= conduit_semantic_catalog::MAX_TEXT_VALUES
        })
        .ok_or_else(|| "presentation/text maximum-values is invalid".into())
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
