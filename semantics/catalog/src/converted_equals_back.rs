//! Shared bounded conversion-receipt Boolean projection.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId, ValueRef,
};
use conduit_plot::quantity_conversion::converted_equals::PreparedConvertedEquals;

/// Prepared receipt validator and two admitted Boolean values. Play allocates
/// no values and computes from typed quantities; authored spelling is checked
/// only as evidence.
pub struct ConvertedEqualsBack {
    comparison: PreparedConvertedEquals,
    decisions: [Option<ValueRef>; 2],
    maximum_input_bytes: u32,
}

impl ConvertedEqualsBack {
    pub fn new(
        comparison: PreparedConvertedEquals,
        decisions: [ValueRef; 2],
        maximum_input_bytes: u32,
    ) -> Self {
        Self {
            comparison,
            decisions: decisions.map(Some),
            maximum_input_bytes,
        }
    }
}

impl<const PORTS: usize> StepBack<PORTS> for ConvertedEqualsBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len > self.maximum_input_bytes {
                return failure(1);
            }
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return failure(2);
            };
            if canonical.len() != value.byte_len as usize {
                return failure(2);
            }
            // Leave input and prepared decisions intact under output pressure.
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let result = match self.comparison.compare_canonical_receipt(canonical) {
                Ok(result) => result,
                Err(_) => return failure(3),
            };
            let selected = usize::from(result);
            let Some(output) = self.decisions[selected].take() else {
                return failure(4);
            };
            let Some(unused) = self.decisions[1 - selected].take() else {
                return failure(4);
            };
            io.consume(PortId(0)).expect("present conversion receipt");
            io.send(PortId(0), output).expect("ready comparator result");
            io.discard(unused).expect("unused comparator decision");
            return StepOutcome::Complete;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed comparator closure");
            for value in &mut self.decisions {
                if let Some(value) = value.take() {
                    io.discard(value).expect("unused comparator decision");
                }
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.decisions = [None; 2];
    }
}

fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    use conduit_core::{
        ConfigurationEntry, ConfigurationValue, Quantity, QuantityConfigurationValue,
        UnitConfigurationValue,
    };
    use conduit_plot::quantity_conversion as conversion;

    #[test]
    fn portable_step_projects_success_difference_refusal_and_dimension_mismatch() {
        for (source, target, expected, result) in [
            ("1kHz", "Hz", "1000Hz", true),
            ("1kHz", "Hz", "999Hz", false),
            ("1Hz", "m", "1m", false),
            ("1kHz", "Hz", "1m", false),
        ] {
            let configuration = vec![
                ConfigurationEntry {
                    key: "source".into(),
                    value: ConfigurationValue::Quantity(
                        QuantityConfigurationValue::parse(source).unwrap(),
                    ),
                },
                ConfigurationEntry {
                    key: "to".into(),
                    value: ConfigurationValue::Unit(UnitConfigurationValue::parse(target).unwrap()),
                },
            ];
            let receipt = conversion::prepare_configuration(&configuration)
                .unwrap()
                .canonical_bytes()
                .unwrap();
            let no = ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 1,
            };
            let yes = ValueRef {
                slot: 1,
                generation: 1,
                byte_len: 1,
            };
            let input = ValueRef {
                slot: 2,
                generation: 1,
                byte_len: receipt.len() as u32,
            };
            let comparison =
                PreparedConvertedEquals::new(Quantity::parse_plot_literal(expected).unwrap())
                    .unwrap();
            let mut back =
                ConvertedEqualsBack::new(comparison, [no, yes], conversion::MAXIMUM_RECEIPT_BYTES);
            let mut blocked = StepIo::test_frame([Some(input)], [false], [None], None, 8);
            let bytes = StepInputBytes::test_frame([Some(receipt.as_slice())], None);
            assert_eq!(back.step(&mut blocked, &bytes), StepOutcome::Await);
            let mut ready = StepIo::test_frame([Some(input)], [false], [Some(1)], None, 8);
            assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Complete);
            assert_eq!(
                ready.test_output(PortId(0)).unwrap(),
                if result { yes } else { no }
            );
        }
    }
}
