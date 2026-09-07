//! Browser production realization of exact measurement summaries.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PlannedGear,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-measurement-summary@1";
const IMPLEMENTATION: &str = "browser/kernel-measurement-summary@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    let contract = conduit_data::measurement_summary_kind_definition();
    let kind = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_data::MEASUREMENT_SUMMARY_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/measurement-summary@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HOST_OPERATION.into(),
            target_kind: Some(kind),
            maximum_in_flight: 1,
            maximum_input_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
            maximum_output_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    validate_placement(placement, &offer())?;
    if !placement.configuration.is_empty() {
        return Err("measurement summary accepts no configuration".into());
    }
    Ok(BrowserOperation::unary(
        MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}

pub(crate) fn execute(input: &[u8]) -> Result<Vec<u8>, Failure> {
    let payload = exact_leaf(input, &conduit_data::measurement_window_type()).ok_or(failure(1))?;
    let window = conduit_data::decode_measurement_window(payload).map_err(|_| failure(2))?;
    let summary = conduit_data::summarize_measurement_window(&window).map_err(|error| {
        use conduit_data::MeasurementSummaryRefusal::*;
        failure(match error {
            EmptyWindow => 3,
            UnitMismatch => 4,
            ArithmeticOverflow => 5,
            InexactMean => 6,
        })
    })?;
    let payload = conduit_data::encode_measurement_summary(&summary).map_err(|_| failure(7))?;
    let value =
        conduit_core::StructuredInfoValue::leaf(conduit_data::measurement_summary_type(), payload)
            .map_err(|_| failure(7))?;
    let bytes = value.canonical_bytes().map_err(|_| failure(7))?;
    if bytes.len() > MAXIMUM_BROWSER_VALUE_BYTES {
        return Err(Failure {
            code: FailureCode::StorageExhausted,
            detail: 8,
        });
    }
    Ok(bytes)
}

fn exact_leaf<'a>(
    canonical: &'a [u8],
    value_type: &conduit_core::StructuredInfoType,
) -> Option<&'a [u8]> {
    let type_bytes = value_type.canonical_bytes().ok()?;
    let node = canonical.strip_prefix(type_bytes.as_slice())?;
    if node.first() != Some(&0) || node.len() < 5 {
        return None;
    }
    let length = usize::try_from(u32::from_le_bytes(node[1..5].try_into().ok()?)).ok()?;
    (node.len() == 5 + length).then_some(&node[5..])
}

fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        Quantity, QuantityUnit, StructuredInfoValue, TemporalInstant, TemporalScale,
    };
    use conduit_data::{
        BoundedMeasurementWindow, FullWindowPolicy, MeasurementRange, MeasurementSample,
        MeasurementWindowProfile,
    };

    #[test]
    fn browser_summary_preserves_exact_unit_window_and_mean() {
        let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
            capacity: 3,
            unit: QuantityUnit::Millivolt,
            range: MeasurementRange {
                minimum: Quantity::new(0, QuantityUnit::Millivolt),
                maximum: Quantity::new(100, QuantityUnit::Millivolt),
            },
            clock_basis: "browser-summary-clock".into(),
            full_policy: FullWindowPolicy::Reject,
        })
        .unwrap();
        for (value, ticks) in [(0, 1), (50, 2), (100, 3)] {
            window
                .push(MeasurementSample {
                    value: Quantity::new(value, QuantityUnit::Millivolt),
                    observed_at: TemporalInstant {
                        ticks,
                        scale: TemporalScale::Milliseconds,
                        clock_basis: "browser-summary-clock".into(),
                        resolution_ticks: 1,
                        uncertainty_ticks: 0,
                    },
                    uncertainty: None,
                })
                .unwrap();
        }
        let input = StructuredInfoValue::leaf(
            conduit_data::measurement_window_type(),
            conduit_data::encode_measurement_window(&window).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let output = execute(&input).unwrap();
        let payload = exact_leaf(&output, &conduit_data::measurement_summary_type()).unwrap();
        let summary = conduit_data::decode_measurement_summary(payload).unwrap();
        assert_eq!(summary.sample_count, 3);
        assert_eq!(summary.mean, Quantity::new(50, QuantityUnit::Millivolt));
        assert_eq!(summary.first_observed_at.ticks, 1);
        assert_eq!(summary.last_observed_at.ticks, 3);
    }

    #[test]
    fn browser_summary_distinguishes_wrong_type_malformed_window_and_empty_window() {
        assert_eq!(execute(b"wrong type"), Err(failure(1)));
        let malformed =
            StructuredInfoValue::leaf(conduit_data::measurement_window_type(), vec![1, 0])
                .unwrap()
                .canonical_bytes()
                .unwrap();
        assert_eq!(execute(&malformed), Err(failure(2)));
        let empty = BoundedMeasurementWindow::new(MeasurementWindowProfile {
            capacity: 1,
            unit: QuantityUnit::Millivolt,
            range: MeasurementRange {
                minimum: Quantity::new(0, QuantityUnit::Millivolt),
                maximum: Quantity::new(1, QuantityUnit::Millivolt),
            },
            clock_basis: "browser-summary-clock".into(),
            full_policy: FullWindowPolicy::Reject,
        })
        .unwrap();
        let empty = StructuredInfoValue::leaf(
            conduit_data::measurement_window_type(),
            conduit_data::encode_measurement_window(&empty).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert_eq!(execute(&empty), Err(failure(3)));
    }
}
