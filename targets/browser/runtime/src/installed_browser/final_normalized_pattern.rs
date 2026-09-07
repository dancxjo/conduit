//! Browser installation of finite Flow-to-final normalized-pattern selection.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityOffer, ConfigurationValue, ExecutionProfileId,
    FaceStartupParameter, ImplementationId, ImplementationOffer, PlannedGear,
};
use conduit_kernel::HostedValueStore;

const IMPLEMENTATION: &str = "browser/kernel-final-normalized-pattern@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::final_normalized_pattern_definition();
    CapabilityOffer {
        startup_parameters: vec![FaceStartupParameter {
            name: "maximum-values".into(),
            value_type: "Count".into(),
            has_default: true,
        }],
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                "browser/final-normalized-pattern-kernel@1",
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/final-normalized-pattern@1"),
        },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: conduit_semantic_catalog::final_normalized_pattern_limits(),
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    let maximum = placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("maximum-values", ConfigurationValue::U64(value)) => Some(*value),
            _ => None,
        })
        .ok_or("final normalized-pattern value bound is absent")?;
    if !(1..=conduit_semantic_catalog::MAXIMUM_FINAL_PATTERN_VALUES).contains(&maximum) {
        return Err("final normalized-pattern value bound is outside browser limits".into());
    }
    Ok(BrowserOperation::installed(
        conduit_semantic_catalog::FinalNormalizedPatternOperation::new(maximum),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offer_is_the_exact_portable_flow_to_value_face() {
        let contract = conduit_semantic_catalog::final_normalized_pattern_definition();
        let offer = offer();
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert!(offer.host_operations.is_empty());
    }
}
