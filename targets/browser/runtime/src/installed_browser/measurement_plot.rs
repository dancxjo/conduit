//! Browser production realization of finite measurement plot projection.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostOperationRequirement, ImplementationId, ImplementationOffer,
    KindContractRevision, PlannedGear,
};
use conduit_data::{MeasurementPlotOverflowPolicy, MeasurementPlotProfile, MeasurementPlotSeries};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const HOST_OPERATION: &str = "conduit.host/browser-measurement-plot@1";
const IMPLEMENTATION: &str = "browser/kernel-measurement-plot@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> CapabilityOffer {
    let contract = conduit_data::measurement_plot_kind_definition();
    let kind = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: [("points", "Count", true), ("when-full", "Text", true)]
            .map(
                |(name, value_type, has_default)| conduit_core::FaceStartupParameter {
                    name: name.into(),
                    value_type: value_type.into(),
                    has_default,
                },
            )
            .into(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_data::MEASUREMENT_PLOT_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/measurement-plot@1"),
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
    profile(placement).map_err(|_| "invalid browser measurement plot configuration".to_string())?;
    Ok(BrowserOperation::unary(
        MAXIMUM_BROWSER_VALUE_BYTES as u32,
        1,
    ))
}

pub(crate) fn execute(placement: &PlannedGear, input: &[u8]) -> Result<Vec<u8>, Failure> {
    let payload = exact_leaf(input, &conduit_data::measurement_window_type()).ok_or(failure(1))?;
    let window = conduit_data::decode_measurement_window(payload).map_err(|_| failure(2))?;
    let series = MeasurementPlotSeries::project(&window, profile(placement)?).map_err(|error| {
        use conduit_data::MeasurementPlotRefusal::*;
        failure(match error {
            InvalidPointCapacity => 3,
            EmptyWindow => 4,
            Full => 5,
            DegenerateValueRange => 6,
            ArithmeticOverflow => 7,
            InvalidProjection => 11,
        })
    })?;
    let payload = conduit_data::encode_measurement_plot_series(&series).map_err(|_| failure(8))?;
    let value = conduit_core::StructuredInfoValue::leaf(
        conduit_data::measurement_plot_series_type(),
        payload,
    )
    .map_err(|_| failure(8))?;
    let bytes = value.canonical_bytes().map_err(|_| failure(8))?;
    if bytes.len() > MAXIMUM_BROWSER_VALUE_BYTES {
        return Err(Failure {
            code: FailureCode::StorageExhausted,
            detail: 9,
        });
    }
    Ok(bytes)
}

fn profile(placement: &PlannedGear) -> Result<MeasurementPlotProfile, Failure> {
    let points = placement
        .configuration
        .iter()
        .find_map(|entry| match entry {
            conduit_core::ConfigurationEntry {
                key,
                value: ConfigurationValue::U64(value),
            } if key == "points" => usize::try_from(*value).ok(),
            _ => None,
        })
        .ok_or(failure(10))?;
    let policy = placement
        .configuration
        .iter()
        .find_map(|entry| match entry {
            conduit_core::ConfigurationEntry {
                key,
                value: ConfigurationValue::Text(value),
            } if key == "when-full" => Some(value.as_str()),
            _ => None,
        })
        .ok_or(failure(10))?;
    Ok(MeasurementPlotProfile {
        point_capacity: points,
        overflow_policy: match policy {
            "reject" => MeasurementPlotOverflowPolicy::Reject,
            "evenly-spaced" => MeasurementPlotOverflowPolicy::EvenlySpaced,
            _ => return Err(failure(10)),
        },
    })
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
        ConfigurationEntry, OfferGeneration, Quantity, QuantityUnit, TemporalInstant, TemporalScale,
    };
    use conduit_data::{
        BoundedMeasurementWindow, FullWindowPolicy, MeasurementRange, MeasurementSample,
        MeasurementWindowProfile,
    };

    fn placement() -> PlannedGear {
        let offered = offer();
        PlannedGear {
            placement_id: "plot-placement".into(),
            gear_id: "plot".into(),
            kind_id: offered.kind_id,
            kind_contract_revision: offered.kind_contract_revision,
            execution_profile_id: offered.implementation.execution_profile_id,
            configuration: vec![
                ConfigurationEntry {
                    key: "points".into(),
                    value: ConfigurationValue::U64(2),
                },
                ConfigurationEntry {
                    key: "when-full".into(),
                    value: ConfigurationValue::Text("evenly-spaced".into()),
                },
            ],
            host_id: "browser/plot".into(),
            boot_id: "browser-boot/plot".into(),
            offer_generation: OfferGeneration(1),
            capability_id: offered.capability_id,
            implementation_id: offered.implementation.implementation_id,
            artifact_id: offered.implementation.artifact_id,
            realization_characteristics: Vec::new(),
            limits: offered.limits,
            inputs: offered.inputs,
            outputs: offered.outputs,
            host_operations: offered.host_operations,
            resources: Vec::new(),
            authority: Vec::new(),
            pool_references: Vec::new(),
        }
    }

    #[test]
    fn installed_browser_plot_projects_an_exact_typed_window() {
        let mut window = BoundedMeasurementWindow::new(MeasurementWindowProfile {
            capacity: 3,
            unit: QuantityUnit::Millivolt,
            range: MeasurementRange {
                minimum: Quantity::new(0, QuantityUnit::Millivolt),
                maximum: Quantity::new(100, QuantityUnit::Millivolt),
            },
            clock_basis: "browser-source-clock".into(),
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
                        clock_basis: "browser-source-clock".into(),
                        resolution_ticks: 1,
                        uncertainty_ticks: 0,
                    },
                    uncertainty: None,
                })
                .unwrap();
        }
        let input = conduit_core::StructuredInfoValue::leaf(
            conduit_data::measurement_window_type(),
            conduit_data::encode_measurement_window(&window).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let output = execute(&placement(), &input).unwrap();
        let payload = exact_leaf(&output, &conduit_data::measurement_plot_series_type()).unwrap();
        let series = conduit_data::decode_measurement_plot_series(payload).unwrap();
        assert_eq!((series.source_samples(), series.omitted_samples()), (3, 1));
        assert_eq!(
            series
                .points()
                .iter()
                .map(|point| point.source_index)
                .collect::<Vec<_>>(),
            [0, 2]
        );
    }

    #[test]
    fn installed_browser_plot_keeps_type_and_payload_failures_distinct() {
        assert_eq!(execute(&placement(), b"not a leaf"), Err(failure(1)));
        let malformed = conduit_core::StructuredInfoValue::leaf(
            conduit_data::measurement_window_type(),
            vec![1, 0],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        assert_eq!(execute(&placement(), &malformed), Err(failure(2)));
    }
}
