//! Canonical deterministic inputs for the Little Seismograph specimen.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, KindContractRevision, PortDescriptor, PortDirection, PortTemporal, Quantity,
    QuantityUnit, TemporalInstant, TemporalScale,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};

use crate::{
    measurement_hysteresis_profile_type, measurement_sample_type, measurement_window_profile_type,
    FullWindowPolicy, MeasurementHysteresisProfile, MeasurementRange, MeasurementSample,
    MeasurementThresholdPolicy, MeasurementThresholdState, MeasurementWindowProfile,
};

pub const LITTLE_SEISMOGRAPH_FIXTURE_KIND: &str = "measurement/deterministic-seismograph-inputs";
pub const LITTLE_SEISMOGRAPH_FIXTURE_REVISION: &str =
    "conduit.data/deterministic-seismograph-inputs@1";

pub fn install_little_seismograph_fixture_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(KindSignature {
        kind: LITTLE_SEISMOGRAPH_FIXTURE_KIND.to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(little_seismograph_fixture_definition())
        .map_err(|error| error.to_string())
}

pub fn little_seismograph_fixture_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(LITTLE_SEISMOGRAPH_FIXTURE_KIND),
        kind_contract_revision: KindContractRevision::from(LITTLE_SEISMOGRAPH_FIXTURE_REVISION),
        inputs: vec![],
        outputs: vec![
            output(
                "profile",
                measurement_window_profile_type(),
                PortTemporal::Value,
            ),
            output(
                "measurement",
                measurement_sample_type(),
                PortTemporal::Flow { closes: true },
            ),
            output(
                "threshold-profile",
                measurement_hysteresis_profile_type(),
                PortTemporal::Value,
            ),
        ],
        configuration: vec![],
    }
}

pub fn deterministic_little_seismograph_inputs() -> (
    MeasurementWindowProfile,
    Vec<MeasurementSample>,
    MeasurementHysteresisProfile,
) {
    let profile = MeasurementWindowProfile {
        capacity: 2,
        unit: QuantityUnit::Millivolt,
        range: MeasurementRange {
            minimum: Quantity::new(0, QuantityUnit::Millivolt),
            maximum: Quantity::new(100, QuantityUnit::Millivolt),
        },
        clock_basis: "fixture-clock".into(),
        full_policy: FullWindowPolicy::DropOldest,
    };
    let samples = [100]
        .into_iter()
        .enumerate()
        .map(|(index, value)| MeasurementSample {
            value: Quantity::new(value, QuantityUnit::Millivolt),
            observed_at: TemporalInstant {
                ticks: index as u64 + 1,
                scale: TemporalScale::Milliseconds,
                clock_basis: "fixture-clock".into(),
                resolution_ticks: 1,
                uncertainty_ticks: 0,
            },
            uncertainty: None,
        })
        .collect();
    let threshold = MeasurementHysteresisProfile {
        policy: MeasurementThresholdPolicy {
            lower: Quantity::new(40, QuantityUnit::Millivolt),
            upper: Quantity::new(60, QuantityUnit::Millivolt),
        },
        initial_state: MeasurementThresholdState::Below,
    };
    (profile, samples, threshold)
}

fn output(
    name: &str,
    value_type: conduit_core::StructuredInfoType,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction: PortDirection::Output,
        temporal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_inputs_preserve_exact_profile_clock_and_transition() {
        let (profile, samples, threshold) = deterministic_little_seismograph_inputs();
        assert_eq!(profile.capacity, 2);
        assert_eq!(profile.full_policy, FullWindowPolicy::DropOldest);
        assert_eq!(samples.len(), 1);
        assert!(samples
            .iter()
            .all(|sample| sample.observed_at.clock_basis.as_str() == profile.clock_basis));

        let mut window = crate::BoundedMeasurementWindow::new(profile).unwrap();
        for sample in samples {
            window.push(sample).unwrap();
        }
        assert_eq!(window.discarded_samples(), 0);
        let summary = crate::summarize_measurement_window(&window).unwrap();
        let decision = crate::MeasurementHysteresis::new(threshold.policy, threshold.initial_state)
            .unwrap()
            .evaluate(&summary)
            .unwrap();
        assert_eq!(decision.state, MeasurementThresholdState::Above);
        assert_eq!(
            decision.transition,
            Some(crate::MeasurementThresholdTransition::RoseAbove)
        );
    }
}
