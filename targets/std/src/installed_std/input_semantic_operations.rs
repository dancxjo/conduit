use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    ChordInfo, ConduitIntlKeymap, ConfigurationValue, KeyEvent, KeymapDisposition, KeymapRefusal,
    PlannedGear, PortDescriptor, PortDirection, CHORD_ENCODED_LEN, CONDUIT_INTL_LAYOUT,
    CORE_CHORD_MAP, KEY_EVENT_ENCODED_LEN,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, OperationAction,
    OperationInput, PortId, RequestId,
};

pub(super) static KEY_EVENT_TEE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::KEY_EVENT_TEE_IMPLEMENTATION,
    budget: key_event_tee_budget,
    prepare: prepare_key_event_tee,
};

pub(super) static KEYMAP_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::KEYMAP_IMPLEMENTATION,
    budget: keymap_budget,
    prepare: prepare_keymap,
};

pub(super) static CHORDS_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: conduit_std_offers::CHORDS_IMPLEMENTATION,
    budget: chords_budget,
    prepare: prepare_chords,
};

pub(super) struct KeyEventTeeOperation {
    pending: Option<conduit_kernel::ValueRef>,
    phase: u8,
}

pub(super) struct InputSemanticOperation {
    pending: Option<RequestId>,
    next: u32,
}

impl KeyEventTeeOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if conduit_semantic_catalog::key_event_tee_accepts_encoded_len(value.byte_len)
                && self.pending.is_none() =>
            {
                self.pending = Some(value);
                self.phase = 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(41),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        match (self.pending, self.phase) {
            (Some(value), 1) => {
                self.phase = 2;
                OperationAction::Emit {
                    port: PortId(1),
                    value,
                }
            }
            (Some(_), 2) => {
                self.pending = None;
                self.phase = 0;
                OperationAction::Await
            }
            _ => InstalledOperation::fail(41),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.phase = 0;
    }
}

impl InputSemanticOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none()
                && self.next < conduit_semantic_catalog::INPUT_SEMANTIC_MAXIMUM_VALUES.into() =>
            {
                let request = RequestId(self.next);
                self.pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: conduit_kernel::HostOperationId(0),
                    input: match BoundedValueRef::new(value, KEY_EVENT_ENCODED_LEN as u32) {
                        Ok(value) => value,
                        Err(_) => return InstalledOperation::fail(42),
                    },
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request) =>
            {
                self.pending = None;
                self.next += 1;
                if let Some(failure) = outcome.failure {
                    return OperationAction::Fail(failure);
                }
                if outcome.disposition != HostOperationDisposition::Completed {
                    return InstalledOperation::fail(42);
                }
                match outcome.output {
                    Some(output) => OperationAction::Emit {
                        port: PortId(0),
                        value: output.value,
                    },
                    None => OperationAction::Await,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(42),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct EncodedOutput {
    bytes: [u8; CHORD_ENCODED_LEN],
    len: usize,
}

impl EncodedOutput {
    pub(super) fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub(super) fn execute_host(
    keymap: bool,
    state: &mut ConduitIntlKeymap,
    input: &[u8],
) -> Result<Option<EncodedOutput>, Failure> {
    let event = KeyEvent::decode(input).map_err(|_| invalid_input(1))?;
    if keymap {
        match state.apply(event) {
            KeymapDisposition::Text(fragment) => {
                let mut bytes = [0; CHORD_ENCODED_LEN];
                let value = fragment.as_bytes();
                bytes[..value.len()].copy_from_slice(value);
                Ok(Some(EncodedOutput {
                    bytes,
                    len: value.len(),
                }))
            }
            KeymapDisposition::NoText | KeymapDisposition::Cancelled => Ok(None),
            KeymapDisposition::Refused(reason) => Err(invalid_input(match reason {
                KeymapRefusal::UnknownComposeSequence => 2,
                KeymapRefusal::EmptyUnicodeEntry => 3,
                KeymapRefusal::UnicodeEntryOverflow => 4,
                KeymapRefusal::InvalidUnicodeScalar => 5,
            })),
        }
    } else {
        Ok(ChordInfo::from_key_event(event).map(|chord| EncodedOutput {
            bytes: chord.encode(),
            len: CHORD_ENCODED_LEN,
        }))
    }
}

const fn invalid_input(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

fn key_event_tee_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_identity(
        placement,
        Identity {
            kind: conduit_semantic_catalog::KEY_EVENT_TEE_KIND,
            revision: conduit_semantic_catalog::KEY_EVENT_TEE_REVISION,
            profile: conduit_std_offers::KEY_EVENT_TEE_PROFILE,
            implementation: conduit_std_offers::KEY_EVENT_TEE_IMPLEMENTATION,
            artifact: conduit_std_offers::KEY_EVENT_TEE_ARTIFACT,
        },
        &conduit_semantic_catalog::key_event_tee_contract().inputs,
        &conduit_semantic_catalog::key_event_tee_contract().outputs,
        None,
    )?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: KEY_EVENT_ENCODED_LEN as u32,
    })
}

fn keymap_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_keymap(placement)?;
    semantic_budget(4)
}

fn chords_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_chords(placement)?;
    semantic_budget(CHORD_ENCODED_LEN as u32)
}

fn semantic_budget(maximum_value_bytes: u32) -> Result<OperationBudget, String> {
    Ok(OperationBudget {
        value_items: conduit_semantic_catalog::INPUT_SEMANTIC_MAXIMUM_VALUES,
        value_bytes: u32::from(conduit_semantic_catalog::INPUT_SEMANTIC_MAXIMUM_VALUES)
            * maximum_value_bytes,
        host_requests: conduit_semantic_catalog::INPUT_SEMANTIC_MAXIMUM_VALUES.into(),
        sign_items: 128,
        maximum_value_bytes,
    })
}

fn prepare_key_event_tee(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    key_event_tee_budget(placement)?;
    Ok(InstalledOperation::KeyEventTee(KeyEventTeeOperation {
        pending: None,
        phase: 0,
    }))
}

fn prepare_keymap(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_keymap(placement)?;
    Ok(InstalledOperation::InputKeymap(InputSemanticOperation {
        pending: None,
        next: 0,
    }))
}

fn prepare_chords(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_chords(placement)?;
    Ok(InstalledOperation::InputChords(InputSemanticOperation {
        pending: None,
        next: 0,
    }))
}

fn validate_keymap(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        Identity {
            kind: conduit_semantic_catalog::KEYMAP_KIND,
            revision: conduit_semantic_catalog::KEYMAP_REVISION,
            profile: conduit_std_offers::KEYMAP_PROFILE,
            implementation: conduit_std_offers::KEYMAP_IMPLEMENTATION,
            artifact: conduit_std_offers::KEYMAP_ARTIFACT,
        },
        &conduit_semantic_catalog::keymap_contract().inputs,
        &conduit_semantic_catalog::keymap_contract().outputs,
        Some(("layout", CONDUIT_INTL_LAYOUT)),
    )
}

fn validate_chords(placement: &PlannedGear) -> Result<(), String> {
    validate_identity(
        placement,
        Identity {
            kind: conduit_semantic_catalog::CHORDS_KIND,
            revision: conduit_semantic_catalog::CHORDS_REVISION,
            profile: conduit_std_offers::CHORDS_PROFILE,
            implementation: conduit_std_offers::CHORDS_IMPLEMENTATION,
            artifact: conduit_std_offers::CHORDS_ARTIFACT,
        },
        &conduit_semantic_catalog::chords_contract().inputs,
        &conduit_semantic_catalog::chords_contract().outputs,
        Some(("map", CORE_CHORD_MAP)),
    )
}

struct Identity {
    kind: &'static str,
    revision: &'static str,
    profile: &'static str,
    implementation: &'static str,
    artifact: &'static str,
}

fn validate_identity(
    placement: &PlannedGear,
    identity: Identity,
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
    configuration: Option<(&str, &str)>,
) -> Result<(), String> {
    let configuration_matches = match configuration {
        None => placement.configuration.is_empty(),
        Some((key, expected)) => {
            placement.configuration.len() == 1
                && placement.configuration[0].key == key
                && placement.configuration[0].value == ConfigurationValue::Text(expected.into())
        }
    };
    if placement.kind_id.as_str() != identity.kind
        || placement.kind_contract_revision.as_str() != identity.revision
        || placement.execution_profile_id.as_str() != identity.profile
        || placement.implementation_id.as_str() != identity.implementation
        || placement.artifact_id.as_str() != identity.artifact
        || placement.inputs != inputs
        || placement.outputs != outputs
        || !configuration_matches
        || inputs
            .iter()
            .any(|port| port.direction != PortDirection::Input)
        || outputs
            .iter()
            .any(|port| port.direction != PortDirection::Output)
    {
        return Err("planned input semantic identity does not match its installation".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{KeyModifiers, KeyTransition};

    #[test]
    fn host_semantics_share_the_core_state_machine_and_refuse_malformed_input() {
        let mut map = ConduitIntlKeymap::new();
        let event = KeyEvent::new(0x08, KeyTransition::Pressed, KeyModifiers::RIGHT_ALT).unwrap();
        let output = execute_host(true, &mut map, &event.encode())
            .unwrap()
            .unwrap();
        assert_eq!(output.as_slice(), "€".as_bytes());
        assert_eq!(execute_host(true, &mut map, &[0, 1]), Err(invalid_input(1)));

        let ctrl_g =
            KeyEvent::new(0x0a, KeyTransition::Pressed, KeyModifiers::LEFT_CONTROL).unwrap();
        let chord = execute_host(false, &mut map, &ctrl_g.encode())
            .unwrap()
            .unwrap();
        assert_eq!(
            ChordInfo::decode(chord.as_slice()).unwrap().chord_id(),
            conduit_core::CoreChordId::CancelOrEscape
        );
    }

    #[test]
    fn cancellation_clears_one_pending_semantic_request_without_a_duplicate() {
        let mut operation = InputSemanticOperation {
            pending: None,
            next: 0,
        };
        let value = conduit_kernel::ValueRef {
            slot: 1,
            generation: 1,
            byte_len: KEY_EVENT_ENCODED_LEN as u32,
        };
        assert!(matches!(
            operation.resume(OperationInput::Value {
                port: PortId(0),
                value,
            }),
            OperationAction::RequestHostOperation {
                request: RequestId(0),
                ..
            }
        ));
        operation.cancel();
        assert_eq!(
            operation.resume(OperationInput::Closed { port: PortId(0) }),
            OperationAction::Complete
        );
    }
}
