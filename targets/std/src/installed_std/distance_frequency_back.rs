//! Pure exact realization of the reviewed Distance-to-Frequency mapping.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{ConfigurationValue, PlannedGear, Quantity, QuantityUnit};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{Failure, FailureCode, PortId};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::DISTANCE_FREQUENCY_MAP_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct DistanceFrequencyBack {
    source_minimum_um: i64,
    source_maximum_um: i64,
    target_minimum_mhz: i64,
    target_maximum_mhz: i64,
    closed: bool,
}

impl DistanceFrequencyBack {
    fn map(&self, distance: Quantity) -> Result<Quantity, u16> {
        let distance = distance
            .convert(QuantityUnit::Micrometer)
            .map_err(|_| 3_u16)?;
        if !(self.source_minimum_um..=self.source_maximum_um).contains(&distance.value()) {
            return Err(4);
        }
        let source_span = i128::from(self.source_maximum_um - self.source_minimum_um);
        let source_offset = i128::from(distance.value() - self.source_minimum_um);
        let target_span = i128::from(self.target_maximum_mhz - self.target_minimum_mhz);
        let numerator = source_offset * target_span;
        // Millihertz is the canonical exact Frequency resolution. Choose the
        // nearest representable value, with ties away from the lower bound.
        let offset = if numerator >= 0 {
            (numerator + source_span / 2) / source_span
        } else {
            (numerator - source_span / 2) / source_span
        };
        let value =
            i64::try_from(i128::from(self.target_minimum_mhz) + offset).map_err(|_| 5_u16)?;
        Ok(Quantity::new(value, QuantityUnit::Millihertz))
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
                conduit_kernel::CanonicalValue::new(&frequency.encode())
                    .expect("Frequency is fixed and bounded"),
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

fn configuration(placement: &PlannedGear, key: &str, unit: QuantityUnit) -> Result<i64, String> {
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
        .convert(unit)
        .map(Quantity::value)
        .map_err(|error| format!("distance-frequency mapping '{key}': {error:?}"))
}

fn prepared(placement: &PlannedGear) -> Result<DistanceFrequencyBack, String> {
    let offer = conduit_std_offers::distance_frequency_map_offer();
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || placement.configuration.len() != 4
    {
        return Err("planned distance-frequency mapping does not match its installation".into());
    }
    let value = DistanceFrequencyBack {
        source_minimum_um: configuration(placement, "source-minimum", QuantityUnit::Micrometer)?,
        source_maximum_um: configuration(placement, "source-maximum", QuantityUnit::Micrometer)?,
        target_minimum_mhz: configuration(placement, "target-minimum", QuantityUnit::Millihertz)?,
        target_maximum_mhz: configuration(placement, "target-maximum", QuantityUnit::Millihertz)?,
        closed: false,
    };
    if value.source_minimum_um >= value.source_maximum_um {
        return Err("distance-frequency mapping bounds are reversed".into());
    }
    Ok(value)
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    prepared(placement)?;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
        host_requests: 0,
        sign_items: 8,
        maximum_value_bytes: conduit_core::QUANTITY_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    prepared(placement).map(InstalledBack::DistanceFrequency)
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

    fn mapping() -> DistanceFrequencyBack {
        DistanceFrequencyBack {
            source_minimum_um: 0,
            source_maximum_um: 300_000,
            target_minimum_mhz: 220_000,
            target_maximum_mhz: 880_000,
            closed: false,
        }
    }

    #[test]
    fn exact_bounds_map_to_exact_bounded_frequency_payloads() {
        for (distance, expected) in [(0, 220_000), (150_000, 550_000), (300_000, 880_000)] {
            let frequency = mapping()
                .map(Quantity::new(distance, QuantityUnit::Micrometer))
                .unwrap();
            assert_eq!(frequency, Quantity::new(expected, QuantityUnit::Millihertz));
            assert_eq!(frequency.encode().len(), conduit_core::QUANTITY_ENCODED_LEN);
        }
        assert_eq!(
            mapping().map(Quantity::new(300_001, QuantityUnit::Micrometer)),
            Err(4)
        );
    }

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
            mapping.map(Quantity::new(0, QuantityUnit::Centimeter)),
            Ok(Quantity::new(1_760_000, QuantityUnit::Millihertz))
        );
        assert_eq!(
            mapping.map(Quantity::new(30, QuantityUnit::Centimeter)),
            Ok(Quantity::new(110_000, QuantityUnit::Millihertz))
        );
    }
}
