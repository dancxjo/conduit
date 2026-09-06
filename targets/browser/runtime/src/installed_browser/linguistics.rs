//! Browser installations for bounded linguistic structured Info.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, FaceStartupParameter, HostOperationContractId, HostOperationRequirement,
    ImplementationId, PlannedGear, StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    PRESENTATION_RESOURCE_CLASS,
};
use conduit_kernel::{HostedValueStore, ValueStorage};

const ARTIFACT: &str = "conduit-browser-runtime/installed-linguistics@1";
const TOKENIZE_IMPLEMENTATION: &str = "browser/kernel-language-tokenize-four@1";
const ANNOTATE_IMPLEMENTATION: &str = "browser/kernel-language-annotate-four@1";
const PRESENTATION_IMPLEMENTATION: &str = "browser/presentation-structured-info@1";
const HOST_OPERATION: &str = "conduit.host/browser-linguistics@1";

pub(super) static TOKENIZE: BrowserInstallation = BrowserInstallation {
    implementation_id: TOKENIZE_IMPLEMENTATION,
    offer: tokenize_offer,
    prepare: prepare_tokenize,
    perform: None,
};
pub(super) static ANNOTATE: BrowserInstallation = BrowserInstallation {
    implementation_id: ANNOTATE_IMPLEMENTATION,
    offer: annotate_offer,
    prepare: prepare_annotate,
    perform: Some(perform_annotate),
};
pub(super) static PRESENTATION: BrowserInstallation = BrowserInstallation {
    implementation_id: PRESENTATION_IMPLEMENTATION,
    offer: presentation_offer,
    prepare: prepare_presentation,
    perform: Some(perform_presentation),
};

pub(super) fn install_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{KindDefinition, KindSignature};
    conduit_language::install_linguistics_catalogs(startup, profile)?;
    let contract = presentation_contract();
    startup.insert(KindSignature {
        kind: conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())
}

fn tokenize_offer() -> CapabilityOffer {
    offer(
        conduit_language::tokenize_four_definition(),
        vec![FaceStartupParameter {
            name: "text".into(),
            value_type: "Text".into(),
            has_default: false,
        }],
        TOKENIZE_IMPLEMENTATION,
        Vec::new(),
    )
}

fn annotate_offer() -> CapabilityOffer {
    offer(
        conduit_language::annotate_four_definition(),
        Vec::new(),
        ANNOTATE_IMPLEMENTATION,
        vec![operation(
            ANNOTATE_IMPLEMENTATION,
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        )],
    )
}

fn presentation_offer() -> CapabilityOffer {
    let contract = presentation_contract();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(PRESENTATION_IMPLEMENTATION),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PRESENTATION_IMPLEMENTATION),
            implementation_id: ImplementationId::from(PRESENTATION_IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![operation(PRESENTATION_IMPLEMENTATION, 0)],
        resource_requirements: vec![conduit_core::resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

fn offer(
    definition: conduit_form::KindDefinition,
    startup_parameters: Vec<FaceStartupParameter>,
    implementation: &str,
    host_operations: Vec<HostOperationRequirement>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters,
        shorthand: None,
        capability_id: CapabilityId::from(implementation),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations,
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 4,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}

fn operation(target: &str, output: u32) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(HOST_OPERATION),
        target_kind: Some(kind_id(target)),
        maximum_in_flight: 1,
        maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        maximum_output_bytes: output,
    }
}

fn presentation_contract() -> conduit_semantic_catalog::StructuredValueContract {
    conduit_semantic_catalog::structured_presentation_contract(
        conduit_language::ANNOTATION_BUNDLE_FOUR_TYPE,
        &conduit_language::annotation_bundle_four_type(),
    )
}

fn prepare_tokenize(
    placement: &PlannedGear,
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &tokenize_offer())?;
    let value = conduit_language::tokenize_four("tour/gear-lab", configuration_text(placement)?)
        .map_err(|error| format!("tokenize four: {error:?}"))?;
    let canonical = value
        .canonical_bytes()
        .map_err(|error| format!("encode linguistic tokens: {error:?}"))?;
    let stored = values.store(&canonical).map_err(debug_error)?;
    Ok(BrowserOperation::source(stored))
}

fn prepare_annotate(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &annotate_offer())?;
    Ok(BrowserOperation::unary(
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        1,
    ))
}

fn prepare_presentation(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &presentation_offer())?;
    Ok(BrowserOperation::presentation(
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        1,
    ))
}

fn perform_annotate(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let tokens = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode linguistic tokens: {error:?}"))?;
    let annotated = conduit_language::annotate_with_unicode_library(&tokens)
        .map_err(|error| format!("annotate four: {error:?}"))?;
    Ok(BrowserHostResult {
        output: Some(
            annotated
                .canonical_bytes()
                .map_err(|error| format!("encode annotations: {error:?}"))?,
        ),
        manifestation: None,
    })
}

fn perform_presentation(_: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    let value = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode structured presentation: {error:?}"))?;
    if value.value_type() != &conduit_language::annotation_bundle_four_type() {
        return Err("structured presentation has the wrong exact linguistic type".into());
    }
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

fn configuration_text(placement: &PlannedGear) -> Result<&str, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("text", ConfigurationValue::Text(value)) => Some(value.as_str()),
            _ => None,
        })
        .ok_or_else(|| "language/tokenize-four is missing its bounded text".into())
}

fn debug_error(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}
