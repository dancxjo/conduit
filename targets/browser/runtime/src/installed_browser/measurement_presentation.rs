//! Browser realization of typed measurement decision and plot presentations.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    HostCallContractId, HostCallRequirement, ImplementationId, Kind, PlannedGear,
    StructuredInfoValue, StructuredInfoValueShape, PRESENTATION_RESOURCE_CLASS,
};
use conduit_kernel::HostedValueStore;

const PLOT_IMPLEMENTATION: &str = "browser/presentation-measurement-plot@1";
const PLOT_OPERATION: &str = "conduit.host/browser-present-measurement-plot@1";
const THRESHOLD_IMPLEMENTATION: &str = "browser/presentation-measurement-threshold@1";
const THRESHOLD_OPERATION: &str = "conduit.host/browser-present-measurement-threshold@1";
const ARTIFACT: &str = "conduit-browser-runtime/measurement-presentation@1";

pub(super) static PLOT: BrowserInstallation = BrowserInstallation {
    implementation_id: PLOT_IMPLEMENTATION,
    offer: plot_offer,
    prepare: prepare_plot,
    perform: Some(perform_plot),
};
pub(super) static THRESHOLD: BrowserInstallation = BrowserInstallation {
    implementation_id: THRESHOLD_IMPLEMENTATION,
    offer: threshold_offer,
    prepare: prepare_threshold,
    perform: Some(perform_threshold),
};

fn plot_offer() -> CapabilityOffer {
    offer(
        conduit_data::measurement_plot_presentation_semantic_contract(),
        PLOT_IMPLEMENTATION,
        PLOT_OPERATION,
    )
}
fn threshold_offer() -> CapabilityOffer {
    offer(
        conduit_data::measurement_threshold_presentation_semantic_contract(),
        THRESHOLD_IMPLEMENTATION,
        THRESHOLD_OPERATION,
    )
}

fn offer(contract: Kind, implementation: &str, operation: &str) -> CapabilityOffer {
    let target_kind = contract.kind_id.clone();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(implementation),
            execution_profile_id: ExecutionProfileId::from(implementation),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(operation),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: 0,
            }],
            resource_requirements: vec![conduit_core::resource_requirement(
                PRESENTATION_RESOURCE_CLASS,
                1,
            )],
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn prepare_plot(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    prepare(placement, plot_offer())
}
fn prepare_threshold(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    prepare(placement, threshold_offer())
}

fn prepare(placement: &PlannedGear, offered: CapabilityOffer) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offered)?;
    Ok(BrowserOperation::presentation(
        MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}

fn perform_plot(_placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    decode_leaf(input, conduit_data::measurement_plot_series_type()).and_then(|payload| {
        conduit_data::decode_measurement_plot_series(&payload)
            .map_err(|error| format!("decode measurement plot presentation: {error:?}"))
    })?;
    manifestation(conduit_data::MEASUREMENT_PLOT_PRESENTATION_KIND, input)
}
fn perform_threshold(_placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    decode_leaf(input, conduit_data::measurement_threshold_decision_type()).and_then(
        |payload| {
            conduit_data::decode_measurement_threshold_decision(&payload)
                .map_err(|error| format!("decode measurement threshold presentation: {error:?}"))
        },
    )?;
    manifestation(conduit_data::MEASUREMENT_THRESHOLD_PRESENTATION_KIND, input)
}

fn decode_leaf(
    input: &[u8],
    expected: conduit_core::StructuredInfoType,
) -> Result<Vec<u8>, String> {
    let value = StructuredInfoValue::from_canonical_bytes(input)
        .map_err(|error| format!("decode measurement presentation value: {error:?}"))?;
    if value.value_type() != &expected {
        return Err("measurement presentation has the wrong exact type".into());
    }
    let StructuredInfoValueShape::Leaf(payload) = value.shape() else {
        return Err("measurement presentation is not an exact leaf".into());
    };
    Ok(payload.to_vec())
}

fn manifestation(kind_id: &'static str, input: &[u8]) -> Result<BrowserHostResult, String> {
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id,
            canonical_value: input.to_vec(),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measurement_presenters_refuse_a_different_exact_leaf_type() {
        for (offer, semantic) in [
            (
                plot_offer(),
                conduit_data::measurement_plot_presentation_semantic_contract(),
            ),
            (
                threshold_offer(),
                conduit_data::measurement_threshold_presentation_semantic_contract(),
            ),
        ] {
            assert_eq!(offer.startup_parameters, semantic.startup_parameters);
            assert_eq!(offer.shorthand, semantic.shorthand);
            assert_eq!(offer.kind_id, semantic.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                semantic.kind_contract_revision
            );
            assert_eq!(offer.inputs, semantic.inputs);
            assert_eq!(offer.outputs, semantic.outputs);
            assert_eq!(offer.limits, semantic.limits);
            assert_eq!(offer.host_calls[0].target_kind, Some(offer.kind_id));
        }
        let wrong = StructuredInfoValue::leaf(conduit_data::measurement_summary_type(), vec![])
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert!(decode_leaf(&wrong, conduit_data::measurement_plot_series_type()).is_err());
        assert!(decode_leaf(&wrong, conduit_data::measurement_threshold_decision_type()).is_err());
    }
}
