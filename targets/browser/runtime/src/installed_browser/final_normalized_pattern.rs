//! Browser installation of finite Flow-to-final normalized-pattern selection.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, ImplementationId, PlannedGear,
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
    BackOfferBuilder::new(
        conduit_semantic_catalog::final_normalized_pattern_semantic_contract(),
        Back {
            capability_id: CapabilityId::from(IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(
                "browser/final-normalized-pattern-kernel@1",
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/final-normalized-pattern@1"),
            host_operations: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
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
    fn offer_is_the_exact_portable_flow_to_value_front() {
        let contract = conduit_semantic_catalog::final_normalized_pattern_definition();
        let offer = offer();
        assert_eq!(offer.inputs, contract.inputs);
        assert_eq!(offer.outputs, contract.outputs);
        assert!(offer.host_operations.is_empty());
    }
}
