//! Dynamic browser realization of exact structured field and variant selectors.

use super::factory::{BrowserHostResult, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    HostOperationContractId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    PlannedGear, PortTemporal, StructuredCanonicalSelection, StructuredSelector,
    StructuredSelectorRefusal, UnmatchedVariantDisposition, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-structured-selector@1";
pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-structured-selector@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: Some(unreachable_perform),
};

pub(crate) struct PreparedSelector {
    selector: StructuredSelector,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
    output: Vec<u8>,
}

impl PreparedSelector {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if placement.implementation_id.as_str() != IMPLEMENTATION {
            return Ok(None);
        }
        let selector = selector_from_placement(placement)?;
        validate(placement, &selector)?;
        Ok(Some(Self {
            input_type: selector
                .input_type()
                .canonical_bytes()
                .map_err(|error| format!("selector input type: {error:?}"))?,
            output_type: selector
                .output_type()
                .canonical_bytes()
                .map_err(|error| format!("selector output type: {error:?}"))?,
            selector,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }))
    }

    pub(crate) fn execute(&mut self, input: &[u8]) -> Result<Option<&[u8]>, Failure> {
        match self
            .selector
            .select_canonical_into(input, &self.input_type, &self.output_type, &mut self.output)
            .map_err(failure)?
        {
            StructuredCanonicalSelection::Matched => Ok(Some(&self.output)),
            StructuredCanonicalSelection::Unmatched(UnmatchedVariantDisposition::Drop) => Ok(None),
            StructuredCanonicalSelection::Unmatched(UnmatchedVariantDisposition::Refuse) => {
                Err(failure(StructuredSelectorRefusal::UnmatchedVariant))
            }
        }
    }
}

pub(crate) fn offer(selector: &StructuredSelector, temporal: PortTemporal) -> CapabilityOffer {
    let contract = conduit_semantic_catalog::structured_selector_contract(selector, temporal);
    let target = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: contract.startup_parameters,
        shorthand: contract.shorthand,
        capability_id: CapabilityId::from(format!("browser/{}", target.as_str())),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                "browser/structured-selector-kernel-hosted@1",
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-core/structured-selector@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(HOST_OPERATION),
            target_kind: Some(target),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: contract.limits,
    }
}

/// Reconstruct and validate the exact contract supported by this installed generic implementation.
#[cfg(feature = "form-runner")]
pub(crate) fn offer_for_placement(
    placement: &PlannedGear,
) -> Result<Option<CapabilityOffer>, String> {
    if placement.implementation_id.as_str() != IMPLEMENTATION {
        return Ok(None);
    }
    let selector = selector_from_placement(placement)?;
    validate(placement, &selector)?;
    let exact = offer(&selector, placement.inputs[0].temporal);
    if placement.capability_id != exact.capability_id {
        return Err("planned selector capability differs from its exact contract".into());
    }
    Ok(Some(exact))
}

fn selector_from_placement(placement: &PlannedGear) -> Result<StructuredSelector, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("structured selector requires one exact configuration".into());
    };
    let ("selector", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err("structured selector configuration is malformed".into());
    };
    StructuredSelector::from_canonical_hex(encoded)
        .map_err(|error| format!("structured selector configuration: {error:?}"))
}

fn validate(placement: &PlannedGear, selector: &StructuredSelector) -> Result<(), String> {
    let temporal = placement
        .inputs
        .first()
        .map(|port| port.temporal)
        .ok_or("structured selector input is absent")?;
    let exact = offer(selector, temporal);
    if placement.kind_id != exact.kind_id
        || placement.kind_contract_revision != exact.kind_contract_revision
        || placement.execution_profile_id != exact.implementation.execution_profile_id
        || placement.implementation_id != exact.implementation.implementation_id
        || placement.artifact_id != exact.implementation.artifact_id
        || placement.inputs != exact.inputs
        || placement.outputs != exact.outputs
        || placement.host_operations != exact.host_operations
    {
        return Err("planned structured selector differs from browser realization".into());
    }
    Ok(())
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    let selector = selector_from_placement(placement)?;
    validate(placement, &selector)?;
    Ok(BrowserOperation::installed(
        conduit_semantic_catalog::StructuredSelectorOperation::new(
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        ),
    ))
}

fn failure(refusal: StructuredSelectorRefusal) -> Failure {
    let detail = match refusal {
        StructuredSelectorRefusal::WrongInputType => 1,
        StructuredSelectorRefusal::MalformedCheckedValue => 2,
        StructuredSelectorRefusal::UnmatchedVariant => 3,
        StructuredSelectorRefusal::CanonicalEncodingTooLarge => 4,
        _ => 5,
    };
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

fn unreachable_offer() -> CapabilityOffer {
    panic!("dynamic structured-selector offers must be derived from checked source")
}

fn unreachable_perform(_: &PlannedGear, _: &[u8]) -> Result<BrowserHostResult, String> {
    Err("structured selector execution requires prepared exact selector state".into())
}
