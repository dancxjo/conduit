//! Pure exact browser realization of portable Distance-to-Frequency mapping.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{ConfigurationValue, PlannedGear, Quantity, Unit};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{Failure, FailureCode, PortId};

const IMPLEMENTATION: &str = "browser/kernel-map-distance-frequency@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};

fn offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::distance_frequency_map_contract(),
        conduit_semantic_catalog::DISTANCE_FREQUENCY_MAP_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: IMPLEMENTATION,
            execution_profile: IMPLEMENTATION,
            implementation: IMPLEMENTATION,
            artifact: "conduit-browser-runtime/distance-frequency@1",
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

struct DistanceFrequencyBack {
    source_minimum_um: i64,
    source_maximum_um: i64,
    target_minimum_mhz: i64,
    target_maximum_mhz: i64,
    closed: bool,
    output: Box<[u8]>,
    output_length: usize,
}

impl DistanceFrequencyBack {
    fn map_coordinate(&self, distance: conduit_core::QuantityRef<'_>) -> Result<i64, u16> {
        let distance = i64::try_from(
            distance
                .project_u64_in_unit(Unit::Micrometer.as_ref())
                .map_err(|_| 3_u16)?,
        )
        .map_err(|_| 3_u16)?;
        if !(self.source_minimum_um..=self.source_maximum_um).contains(&distance) {
            return Err(4);
        }
        let source_span = i128::from(self.source_maximum_um - self.source_minimum_um);
        let source_offset = i128::from(distance - self.source_minimum_um);
        let target_span = i128::from(self.target_maximum_mhz - self.target_minimum_mhz);
        let numerator = source_offset * target_span;
        let offset = if numerator >= 0 {
            (numerator + source_span / 2) / source_span
        } else {
            (numerator - source_span / 2) / source_span
        };
        let value =
            i64::try_from(i128::from(self.target_minimum_mhz) + offset).map_err(|_| 5_u16)?;
        Ok(value)
    }
}

impl<const PORTS: usize> StepBack<PORTS> for DistanceFrequencyBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if io.input(PortId(0)).is_some() {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(1);
            };
            let Ok(distance) = conduit_core::QuantityRef::validate(bytes) else {
                return fail(2);
            };
            let frequency = match self.map_coordinate(distance) {
                Ok(value) => value,
                Err(detail) => return fail(detail),
            };
            io.consume(PortId(0)).expect("present Distance");
            self.output_length = match Quantity::encode_integer_into(
                frequency,
                Unit::Millihertz.as_ref(),
                conduit_core::QuantityRole::Linear,
                conduit_core::PortableNumericProfile {
                    maximum_encoded_bytes: 128,
                    ..conduit_core::PortableNumericProfile::MAXIMAL
                },
                &mut self.output,
            ) {
                Ok(length) => length,
                Err(_) => return fail(5),
            };
            io.send_prepared(PortId(0), self.output_length as u32)
                .expect("ready Frequency output");
            return StepOutcome::Progress;
        }
        if !self.closed && io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed Distance close");
            self.closed = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0)).then_some(&self.output[..self.output_length])
    }

    fn cancel(&mut self) {
        self.closed = true;
    }
}

fn configured(placement: &PlannedGear, key: &str, unit: Unit) -> Result<i64, String> {
    let value = placement
        .configuration
        .iter()
        .find_map(|entry| (entry.key == key).then_some(&entry.value))
        .ok_or_else(|| format!("distance-frequency mapping requires '{key}'"))?;
    let ConfigurationValue::Quantity(value) = value else {
        return Err(format!(
            "distance-frequency mapping '{key}' is not a Quantity"
        ));
    };
    value
        .value()
        .to_i64(&unit)
        .map_err(|error| format!("distance-frequency mapping '{key}': {error:?}"))
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<BrowserBack, String> {
    validate_placement(placement, &offer())?;
    if placement.configuration.len() != 4 {
        return Err("distance-frequency mapping requires exactly four configuration fields".into());
    }
    let back = DistanceFrequencyBack {
        source_minimum_um: configured(placement, "source-minimum", Unit::Micrometer)?,
        source_maximum_um: configured(placement, "source-maximum", Unit::Micrometer)?,
        target_minimum_mhz: configured(placement, "target-minimum", Unit::Millihertz)?,
        target_maximum_mhz: configured(placement, "target-maximum", Unit::Millihertz)?,
        closed: false,
        output: vec![0; conduit_core::MAX_QUANTITY_ENCODED_BYTES].into_boxed_slice(),
        output_length: 0,
    };
    if back.source_minimum_um >= back.source_maximum_um {
        return Err("distance-frequency mapping bounds are reversed".into());
    }
    Ok(BrowserBack::installed_step(back))
}

const fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descending_theremin_range_reaches_both_exact_endpoints() {
        let mapping = DistanceFrequencyBack {
            source_minimum_um: 0,
            source_maximum_um: 300_000,
            target_minimum_mhz: 1_760_000,
            target_maximum_mhz: 110_000,
            closed: false,
            output: vec![0; conduit_core::MAX_QUANTITY_ENCODED_BYTES].into_boxed_slice(),
            output_length: 0,
        };
        assert_eq!(
            mapping.map_coordinate(Quantity::new(0, Unit::Centimeter).as_ref()),
            Ok(1_760_000)
        );
        assert_eq!(
            mapping.map_coordinate(Quantity::new(30, Unit::Centimeter).as_ref()),
            Ok(110_000)
        );
    }
}
