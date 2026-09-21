use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{ConfigurationValue, PlannedGear, PortDescriptor, PortDirection};
use conduit_human::{
    ChordInfo, ConduitIntlKeymap, KeyEvent, KeymapDisposition, KeymapRefusal, CHORD_ENCODED_LEN,
    CONDUIT_INTL_LAYOUT, CORE_CHORD_MAP, KEY_EVENT_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
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

impl<const PORTS: usize> StepBack<PORTS> for KeyEventTeeOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if !conduit_semantic_catalog::key_event_tee_accepts_encoded_len(value.byte_len) {
                return step_fail(41);
            }
            if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(0)).expect("present key event for tee");
            io.send(PortId(0), value)
                .expect("ready first key-event tee output");
            io.send(PortId(1), value)
                .expect("ready second key-event tee output");
            self.pending = None;
            self.phase = 0;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0))
                .expect("observed key-event tee closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.phase = 0;
    }
}

impl<const PORTS: usize> StepBack<PORTS> for InputSemanticOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request) {
                return step_fail(42);
            }
            if let Some(failure) = outcome.failure {
                return StepOutcome::Fail(failure);
            }
            if outcome.disposition != HostCallDisposition::Completed {
                return step_fail(42);
            }
            if outcome.output.is_some() && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed input semantic completion");
            if let Some(output) = outcome.output {
                io.send(PortId(0), output.value)
                    .expect("ready input semantic output");
            }
            self.pending = None;
            self.next = self.next.saturating_add(1);
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some()
                || self.next >= u32::from(conduit_semantic_catalog::INPUT_SEMANTIC_MAXIMUM_VALUES)
            {
                return step_fail(42);
            }
            let Ok(input) = BoundedValueRef::new(value, KEY_EVENT_ENCODED_LEN as u32) else {
                return step_fail(42);
            };
            let request = RequestId(self.next);
            io.consume(PortId(0))
                .expect("present input semantic key event");
            io.request_host_call(request, HostCallId(0), input)
                .expect("input semantic Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed input semantic closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl KeyEventTeeOperation {}

impl InputSemanticOperation {}

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
    use conduit_human::{KeyModifiers, KeyTransition};

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
            conduit_human::CoreChordId::CancelOrEscape
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
        let mut io = StepIo::test_frame([Some(value)], [false], [Some(4)], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Progress
        );
        assert_eq!(
            io.test_host_request().map(|request| request.0),
            Some(RequestId(0))
        );
        StepBack::<1>::cancel(&mut operation);
        let mut io = StepIo::test_frame([None], [true], [None], None, 8);
        assert_eq!(
            operation.step(&mut io, &StepInputBytes::test_frame([None], None)),
            StepOutcome::Complete
        );
        assert!(io.test_consumed_closed(PortId(0)));
    }
}
