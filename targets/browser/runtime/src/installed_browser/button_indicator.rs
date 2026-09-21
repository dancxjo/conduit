//! Ordinary browser implementations for the semantic button/indicator chain.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{
    kind_id, HostCallContractId, HostCallRequirement, InfoBool, PlannedGear, BOOL_ENCODED_LEN,
    PRESENTATION_RESOURCE_CLASS,
};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    CanonicalValue, Failure, FailureCode, HostedValueStore, PortId,
};

const ARTIFACT: &str = "conduit-browser-runtime/button-indicator@1";
const MAPPER_IMPLEMENTATION: &str = "browser/kernel-button-indicator-state@1";
const INDICATOR_IMPLEMENTATION: &str = "browser/presentation-indicator-state@1";
const INDICATOR_OPERATION: &str = "conduit.host/browser-present-indicator-state@1";

pub(super) static MAPPER: BrowserInstallation = BrowserInstallation {
    implementation_id: MAPPER_IMPLEMENTATION,
    offer: mapper_offer,
    prepare: prepare_mapper,
    perform: None,
};

pub(super) static INDICATOR: BrowserInstallation = BrowserInstallation {
    implementation_id: INDICATOR_IMPLEMENTATION,
    offer: indicator_offer,
    prepare: prepare_indicator,
    perform: Some(perform_indicator),
};

fn mapper_offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::button_indicator_state_contract(),
        conduit_semantic_catalog::BUTTON_INDICATOR_STATE_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: MAPPER_IMPLEMENTATION,
            execution_profile: MAPPER_IMPLEMENTATION,
            implementation: MAPPER_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
}

fn indicator_offer() -> conduit_core::CapabilityOffer {
    conduit_semantic_catalog::realization_offer(
        conduit_semantic_catalog::indicator_state_presentation_contract(),
        conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_REVISION,
        conduit_semantic_catalog::RealizationOfferIdentity {
            capability: INDICATOR_IMPLEMENTATION,
            execution_profile: INDICATOR_IMPLEMENTATION,
            implementation: INDICATOR_IMPLEMENTATION,
            artifact: ARTIFACT,
        },
        vec![HostCallRequirement {
            contract_id: HostCallContractId::from(INDICATOR_OPERATION),
            target_kind: Some(kind_id(
                conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_KIND,
            )),
            maximum_in_flight: 1,
            maximum_input_bytes: BOOL_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        vec![conduit_core::resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        Vec::new(),
    )
}

fn prepare_mapper(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &mapper_offer())?;
    Ok(BrowserOperation::installed_step(ButtonIndicatorOperation {
        mapper: conduit_semantic_catalog::PreparedButtonIndicatorMapper::new().map_err(debug)?,
        emitted: 0,
    }))
}

fn prepare_indicator(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &indicator_offer())?;
    Ok(BrowserOperation::presentation(BOOL_ENCODED_LEN as u32, 8))
}

fn perform_indicator(_placement: &PlannedGear, input: &[u8]) -> Result<BrowserHostResult, String> {
    InfoBool::decode(input).map_err(debug)?;
    Ok(BrowserHostResult {
        output: None,
        manifestation: Some(BrowserManifestation {
            kind_id: conduit_semantic_catalog::INDICATOR_STATE_PRESENTATION_KIND,
            canonical_value: input.to_vec(),
        }),
    })
}

struct ButtonIndicatorOperation {
    mapper: conduit_semantic_catalog::PreparedButtonIndicatorMapper,
    emitted: u32,
}

impl<const PORTS: usize> StepOperation<PORTS> for ButtonIndicatorOperation {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if let Some(input) = io.input(PortId(0)) {
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(canonical) = input_bytes.input(PortId(0)) else {
                return fail(61);
            };
            if canonical.len() != input.byte_len as usize {
                return fail(61);
            }
            let value = match self.mapper.map(canonical) {
                Ok(value) => value,
                Err(_) => return fail(62),
            };
            let Some(next) = self.emitted.checked_add(1) else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::IdentityCapacityExhausted,
                    detail: 63,
                });
            };
            let output = CanonicalValue::new(&value.encode())
                .expect("indicator state has a fixed canonical encoding");
            io.consume(PortId(0)).expect("present button transition");
            io.send_canonical(PortId(0), output)
                .expect("ready indicator state output");
            self.emitted = next;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed button transition closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

fn debug(error: impl core::fmt::Debug) -> String {
    format!("{error:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueRef;

    fn value(byte_len: usize) -> ValueRef {
        ValueRef {
            slot: 3,
            generation: 1,
            byte_len: byte_len as u32,
        }
    }

    fn step<const PORTS: usize>(
        operation: &mut ButtonIndicatorOperation,
        inputs: [Option<ValueRef>; PORTS],
        closed: [bool; PORTS],
        outputs: [Option<u32>; PORTS],
        bytes: [Option<&[u8]>; PORTS],
    ) -> (StepOutcome, StepIo<PORTS>) {
        let mut io = StepIo::test_frame(inputs, closed, outputs, None, 4);
        let outcome = operation.step(&mut io, &StepInputBytes::test_frame(bytes, None));
        (outcome, io)
    }

    #[test]
    fn pressed_and_released_emit_transaction_local_current_states() {
        let mut operation = ButtonIndicatorOperation {
            mapper: conduit_semantic_catalog::PreparedButtonIndicatorMapper::new().unwrap(),
            emitted: 0,
        };
        for pressed in [true, false, true, false] {
            let encoded = conduit_semantic_catalog::button_transition_value(
                "button/primary",
                pressed,
                u64::from(operation.emitted),
            )
            .unwrap()
            .canonical_bytes()
            .unwrap();
            let (outcome, io) = step(
                &mut operation,
                [Some(value(encoded.len()))],
                [false],
                [Some(BOOL_ENCODED_LEN as u32)],
                [Some(&encoded)],
            );
            assert_eq!(outcome, StepOutcome::Progress);
            let (port, output) = io.test_canonical_output().unwrap();
            assert_eq!(*port, PortId(0));
            assert_eq!(
                output.as_slice(),
                &if pressed {
                    InfoBool::TRUE
                } else {
                    InfoBool::FALSE
                }
                .encode()
            );
        }
    }

    #[test]
    fn mapping_preserves_pressure_state_and_closure() {
        let mut operation = ButtonIndicatorOperation {
            mapper: conduit_semantic_catalog::PreparedButtonIndicatorMapper::new().unwrap(),
            emitted: 0,
        };
        let encoded = conduit_semantic_catalog::button_transition_value("button/primary", true, 0)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert_eq!(
            step(
                &mut operation,
                [Some(value(7))],
                [false],
                [Some(BOOL_ENCODED_LEN as u32)],
                [Some(b"pressed")],
            )
            .0,
            fail(62)
        );
        assert_eq!(operation.emitted, 0);
        assert_eq!(
            step(
                &mut operation,
                [Some(value(encoded.len()))],
                [false],
                [Some(BOOL_ENCODED_LEN as u32)],
                [Some(b"pressed")],
            )
            .0,
            fail(61)
        );
        assert_eq!(operation.emitted, 0);
        let (outcome, io) = step(
            &mut operation,
            [Some(value(encoded.len()))],
            [false],
            [None],
            [Some(&encoded)],
        );
        assert_eq!(outcome, StepOutcome::Await);
        assert!(!io.test_consumed(PortId(0)));
        assert_eq!(operation.emitted, 0);
        assert_eq!(
            step(
                &mut operation,
                [Some(value(encoded.len()))],
                [false],
                [Some(BOOL_ENCODED_LEN as u32)],
                [Some(&encoded)],
            )
            .0,
            StepOutcome::Progress
        );
        assert_eq!(operation.emitted, 1);
        assert_eq!(
            step(&mut operation, [None], [true], [None], [None]).0,
            StepOutcome::Complete
        );
    }
}
