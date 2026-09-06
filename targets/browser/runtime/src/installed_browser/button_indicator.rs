//! Ordinary browser implementations for the semantic button/indicator chain.

use super::factory::{
    validate_placement, BrowserHostResult, BrowserInstallation, BrowserManifestation,
};
use super::BrowserOperation;
use conduit_core::{
    kind_id, HostOperationContractId, HostOperationRequirement, InfoBool, PlannedGear,
    BOOL_ENCODED_LEN, PRESENTATION_RESOURCE_CLASS,
};
use conduit_kernel::{
    Failure, FailureCode, HostedValueStore, Operation, OperationAction, OperationInput, PortId,
    ValueRef, ValueStorage,
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
        vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(INDICATOR_OPERATION),
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
    values: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &mapper_offer())?;
    let admitted_states = |values: &mut HostedValueStore, level: InfoBool| {
        (0..conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_VALUES)
            .map(|_| values.store(&level.encode()).map_err(debug))
            .collect::<Result<Vec<_>, _>>()
    };
    let off = admitted_states(values, InfoBool::FALSE)?;
    let on = admitted_states(values, InfoBool::TRUE)?;
    Ok(BrowserOperation::installed(ButtonIndicatorOperation {
        mapper: conduit_semantic_catalog::PreparedButtonIndicatorMapper::new().map_err(debug)?,
        off,
        on,
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
    off: Vec<ValueRef>,
    on: Vec<ValueRef>,
    emitted: usize,
}

impl Operation for ButtonIndicatorOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port: PortId(0) } => OperationAction::Complete,
            _ => fail(60),
        }
    }

    fn resume_value(
        &mut self,
        port: PortId,
        _value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        if port != PortId(0) {
            return fail(61);
        }
        match self.mapper.map(canonical) {
            Ok(value) if self.emitted < self.on.len() => {
                let emitted = self.emitted;
                self.emitted += 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: if value.get() {
                        self.on[emitted]
                    } else {
                        self.off[emitted]
                    },
                }
            }
            Ok(_) => fail(63),
            Err(_) => fail(62),
        }
    }

    fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
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

    #[test]
    fn pressed_and_released_emit_pre_admitted_current_states() {
        let mut operation = ButtonIndicatorOperation {
            mapper: conduit_semantic_catalog::PreparedButtonIndicatorMapper::new().unwrap(),
            off: vec![value(1), value(3)],
            on: vec![value(2), value(4)],
            emitted: 0,
        };
        for (pressed, slot) in [(true, 2), (false, 3)] {
            let encoded = conduit_semantic_catalog::button_transition_value(
                "button/primary",
                pressed,
                u64::from(slot),
            )
            .unwrap()
            .canonical_bytes()
            .unwrap();
            assert_eq!(
                operation.resume_value(
                    PortId(0),
                    ValueRef {
                        slot: 3,
                        generation: 1,
                        byte_len: encoded.len() as u32
                    },
                    &encoded,
                ),
                OperationAction::Emit {
                    port: PortId(0),
                    value: ValueRef {
                        slot,
                        generation: 1,
                        byte_len: 1
                    },
                }
            );
            assert_eq!(operation.advance(), OperationAction::Await);
        }
    }

    fn value(slot: u16) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: 1,
        }
    }

    #[test]
    fn mapping_refuses_bad_input_and_exhaustion_and_preserves_closure() {
        let mut operation = ButtonIndicatorOperation {
            mapper: conduit_semantic_catalog::PreparedButtonIndicatorMapper::new().unwrap(),
            off: vec![value(1)],
            on: vec![value(2)],
            emitted: 0,
        };
        let encoded = conduit_semantic_catalog::button_transition_value("button/primary", true, 0)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        assert_eq!(operation.start(), OperationAction::Await);
        assert_eq!(
            operation.resume_value(PortId(1), value(0), &encoded),
            fail(61)
        );
        assert_eq!(
            operation.resume_value(PortId(0), value(0), b"pressed"),
            fail(62)
        );
        assert_eq!(operation.emitted, 0);
        assert_eq!(
            operation.resume_value(PortId(0), value(0), &encoded),
            OperationAction::Emit {
                port: PortId(0),
                value: value(2)
            }
        );
        assert_eq!(operation.advance(), OperationAction::Await);
        assert_eq!(
            operation.resume_value(PortId(0), value(0), &encoded),
            fail(63)
        );
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );
    }
}
