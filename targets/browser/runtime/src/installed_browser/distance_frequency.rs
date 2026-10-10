//! Pure exact browser realization of portable Distance-to-Frequency mapping.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{ConfigurationValue, PlannedGear, Quantity, Unit};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{CanonicalValue, Failure, FailureCode, PortId};

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
}

impl DistanceFrequencyBack {
    fn map(&self, distance: Quantity) -> Result<Quantity, u16> {
        let distance = distance.to_i64(Unit::Micrometer).map_err(|_| 3_u16)?;
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
        Ok(Quantity::new(value, Unit::Millihertz))
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
            let Ok(distance) = Quantity::decode(bytes) else {
                return fail(2);
            };
            let frequency = match self.map(distance) {
                Ok(value) => value,
                Err(detail) => return fail(detail),
            };
            io.consume(PortId(0)).expect("present Distance");
            io.send_canonical(
                PortId(0),
                CanonicalValue::new(&frequency.encode()).expect("Frequency is fixed and bounded"),
            )
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
        .to_i64(unit)
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
        };
        assert_eq!(
            mapping.map(Quantity::new(0, Unit::Centimeter)),
            Ok(Quantity::new(1_760_000, Unit::Millihertz))
        );
        assert_eq!(
            mapping.map(Quantity::new(30, Unit::Centimeter)),
            Ok(Quantity::new(110_000, Unit::Millihertz))
        );
    }
}
