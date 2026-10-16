use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{ConfigurationValue, PlannedGear, PortDescriptor, PortDirection};
use conduit_human::{
    ChordInfo, KeyEvent, KeymapRefusal, PreparedKeymapChoices as PreparedKeymap, CHORD_ENCODED_LEN,
    CORE_CHORD_MAP, KEYMAP_MAXIMUM_TEXT_BYTES, KEY_EVENT_ENCODED_LEN,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
};

pub(super) static KEY_EVENT_TEE_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::KEY_EVENT_TEE_IMPLEMENTATION,
    budget: key_event_tee_budget,
    prepare: prepare_key_event_tee,
};

pub(super) static KEYMAP_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::KEYMAP_IMPLEMENTATION,
    budget: keymap_budget,
    prepare: prepare_keymap,
};

pub(super) static CHORDS_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::CHORDS_IMPLEMENTATION,
    budget: chords_budget,
    prepare: prepare_chords,
};

pub(super) struct KeyEventTeeBack {
    pending: Option<conduit_kernel::ValueRef>,
    phase: u8,
}

pub(super) struct InputSemanticBack {
    pending: Option<RequestId>,
    next: u32,
    selections: bool,
    selection: Option<conduit_kernel::ValueRef>,
}

impl<const PORTS: usize> StepBack<PORTS> for KeyEventTeeBack {
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

impl<const PORTS: usize> StepBack<PORTS> for InputSemanticBack {
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
        if self.pending.is_some() {
            return StepOutcome::Await;
        }
        if self.selections {
            if let Some(value) = io
                .input(PortId(1))
                .filter(|value| Some(*value) != self.selection)
            {
                let Ok(input) = BoundedValueRef::new(value, 32) else {
                    return step_fail(42);
                };
                let request = RequestId(self.next);
                io.consume(PortId(1)).expect("present keymap selection");
                io.request_host_call(request, HostCallId(0), input)
                    .expect("keymap selection call");
                self.pending = Some(request);
                self.selection = Some(value);
                return StepOutcome::Progress;
            }
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
            io.request_host_call(request, HostCallId(u16::from(self.selections)), input)
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
        self.selection = None;
    }
}

const fn step_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}

impl KeyEventTeeBack {}

impl InputSemanticBack {}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct EncodedOutput {
    bytes: [u8; KEYMAP_MAXIMUM_TEXT_BYTES],
    len: usize,
}

impl EncodedOutput {
    pub(super) fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.len]
    }
}

pub(super) fn execute_host(
    keymap: bool,
    state: &mut Option<PreparedKeymap>,
    input: &[u8],
) -> Result<Option<EncodedOutput>, Failure> {
    let event = KeyEvent::decode(input).map_err(|_| invalid_input(1))?;
    if keymap {
        let state = state.as_mut().ok_or_else(|| invalid_input(6))?;
        match state.apply(event) {
            Ok(Some(fragment)) => {
                let value = fragment.as_slice();
                let mut bytes = [0; KEYMAP_MAXIMUM_TEXT_BYTES];
                bytes[..value.len()].copy_from_slice(value);
                Ok(Some(EncodedOutput {
                    bytes,
                    len: value.len(),
                }))
            }
            Ok(None) => Ok(None),
            Err(reason) => Err(invalid_input(match reason {
                KeymapRefusal::UnknownComposeSequence => 2,
                KeymapRefusal::EmptyUnicodeEntry => 3,
                KeymapRefusal::UnicodeEntryOverflow => 4,
                KeymapRefusal::InvalidUnicodeScalar => 5,
            })),
        }
    } else {
        Ok(ChordInfo::from_key_event(event).map(|chord| {
            let mut bytes = [0; KEYMAP_MAXIMUM_TEXT_BYTES];
            bytes[..CHORD_ENCODED_LEN].copy_from_slice(&chord.encode());
            EncodedOutput {
                bytes,
                len: CHORD_ENCODED_LEN,
            }
        }))
    }
}

pub(super) fn select_host(
    state: &mut Option<PreparedKeymap>,
    input: &[u8],
) -> Result<Option<EncodedOutput>, Failure> {
    state
        .as_mut()
        .ok_or_else(|| invalid_input(6))?
        .select(input)
        .map_err(|_| invalid_input(7))?;
    Ok(None)
}

/// Prepare each layout against the exact lowered node identity before play.
/// Body partitions use a shared node space; placement order is not a node ID.
pub(super) fn prepare_keymaps<'a, const N: usize>(
    placements: impl IntoIterator<Item = (Option<usize>, &'a PlannedGear)>,
) -> Result<[Option<PreparedKeymap>; N], String> {
    let mut keymaps = core::array::from_fn(|_| None);
    for (node, placement) in placements {
        if placement.kind_id.as_str() != conduit_semantic_catalog::KEYMAP_KIND {
            continue;
        }
        validate_keymap(placement)?;
        let node = node.ok_or("keymap placement has no lowered node")?;
        let state = keymaps
            .get_mut(node)
            .ok_or("keymap node exceeds prepared capacity")?;
        if state.is_some() {
            return Err("keymap node was prepared twice".into());
        }
        let layout = structured_configuration(placement, "layout")
            .ok_or("keymap layout must be a structured Conduitese value")?;
        let choices = placement
            .configuration
            .iter()
            .find_map(|entry| match (&*entry.key, &entry.value) {
                ("choices", ConfigurationValue::Structured(value)) => Some(value),
                _ => None,
            })
            .ok_or("keymap choices are missing")?;
        *state = Some(PreparedKeymap::new(layout, choices)?);
    }
    Ok(keymaps)
}

const fn invalid_input(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

fn key_event_tee_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
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
        ConfigurationMatch::Empty,
    )?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 96,
        maximum_value_bytes: KEY_EVENT_ENCODED_LEN as u32,
    })
}

fn keymap_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_keymap(placement)?;
    semantic_budget(KEYMAP_MAXIMUM_TEXT_BYTES as u32)
}

fn chords_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_chords(placement)?;
    semantic_budget(CHORD_ENCODED_LEN as u32)
}

fn semantic_budget(maximum_value_bytes: u32) -> Result<BackBudget, String> {
    Ok(BackBudget {
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
) -> Result<InstalledBack, String> {
    key_event_tee_budget(placement)?;
    Ok(InstalledBack::KeyEventTee(KeyEventTeeBack {
        pending: None,
        phase: 0,
    }))
}

fn prepare_keymap(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate_keymap(placement)?;
    Ok(InstalledBack::InputKeymap(InputSemanticBack {
        pending: None,
        next: 0,
        selections: true,
        selection: None,
    }))
}

fn prepare_chords(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate_chords(placement)?;
    Ok(InstalledBack::InputChords(InputSemanticBack {
        pending: None,
        next: 0,
        selections: false,
        selection: None,
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
        ConfigurationMatch::Layout,
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
        ConfigurationMatch::Text("map", CORE_CHORD_MAP),
    )
}

enum ConfigurationMatch<'a> {
    Empty,
    Text(&'a str, &'a str),
    Layout,
}

struct Identity {
    kind: &'static str,
    revision: &'static str,
    profile: &'static str,
    implementation: &'static str,
    artifact: &'static str,
}

fn structured_configuration<'a>(
    placement: &'a PlannedGear,
    key: &str,
) -> Option<&'a conduit_core::StructuredConfigurationValue> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (name, ConfigurationValue::Structured(value)) if name == key => Some(value),
            _ => None,
        })
}

fn validate_identity(
    placement: &PlannedGear,
    identity: Identity,
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
    configuration: ConfigurationMatch<'_>,
) -> Result<(), String> {
    let configuration_matches = match configuration {
        ConfigurationMatch::Empty => placement.configuration.is_empty(),
        ConfigurationMatch::Layout => {
            placement.configuration.len() == 2
                && structured_configuration(placement, "layout")
                    .zip(structured_configuration(placement, "choices"))
                    .is_some_and(|(layout, choices)| PreparedKeymap::new(layout, choices).is_ok())
        }
        ConfigurationMatch::Text(key, expected) => {
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
#[path = "input_semantic_backs_tests.rs"]
mod tests;
