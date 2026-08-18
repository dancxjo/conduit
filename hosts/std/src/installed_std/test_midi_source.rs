use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, Gate, ImplementationId, KindContractRevision, MusicalControl,
    MusicalControlEvent, MusicalNoteEvent, MusicalPitch, NoteOccurrenceId, PlannedGear,
    PortDescriptor, PortDirection, PortTemporal, MUSIC_CONTROL_INFO_ID, MUSIC_NOTE_INFO_ID,
};
use conduit_form::{KindDefinition, ProfileCatalog};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) const KIND: &str = "conduit-proof/midi-performance-source";
const REVISION: &str = "conduit-proof/midi-performance-source@1";
const PROFILE: &str = "conduit-proof/midi-performance-source-kernel@1";
pub(super) const IMPLEMENTATION: &str = "conduit-proof/midi-performance-source-kernel@1";
const ARTIFACT: &str = "conduit-std-host/proof-midi-performance-source@1";
const EVENT_COUNT: usize = 3;
const YIELD_COUNT: usize = EVENT_COUNT - 1;

pub(super) static FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) struct TestMidiSourceOperation {
    values: [ValueRef; EVENT_COUNT],
    ports: [PortId; EVENT_COUNT],
    yield_markers: [ValueRef; YIELD_COUNT],
    next: usize,
    pending: Option<RequestId>,
}

impl TestMidiSourceOperation {
    pub(super) fn emit_or_complete(&self) -> OperationAction {
        self.values
            .get(self.next)
            .copied()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: self.ports[self.next],
                value,
            })
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next += 1;
        if self.next >= self.values.len() {
            return OperationAction::Complete;
        }
        let request = RequestId(self.next as u32);
        self.pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.yield_markers[self.next - 1], 1)
                .expect("test MIDI yield marker is one byte"),
        }
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending == Some(request)
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.pending = None;
                self.emit_or_complete()
            }
            _ => InstalledOperation::fail(90),
        }
    }

    pub(super) fn cancel(&mut self) {
        self.next = self.values.len();
        self.pending = None;
    }
}

pub(super) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-midi-performance-source"),
        kind_id: kind_id(KIND),
        kind_contract_revision: KindContractRevision::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: outputs(),
        host_operations: vec![conduit_core::HostOperationRequirement {
            contract_id: conduit_core::HostOperationContractId::from(
                super::test_audio_source::YIELD_OPERATION,
            ),
            target_kind: None,
            maximum_in_flight: 1,
            maximum_input_bytes: 1,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: EVENT_COUNT as u16,
            max_queue_bytes: (2 * conduit_core::NOTE_EVENT_ENCODED_LEN
                + conduit_core::CONTROL_EVENT_ENCODED_LEN) as u32,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(KIND),
            kind_contract_revision: KindContractRevision::from(REVISION),
            inputs: Vec::new(),
            outputs: outputs(),
            configuration: Vec::new(),
        })
        .expect("test MIDI source kind is unique");
}

fn outputs() -> Vec<PortDescriptor> {
    vec![
        PortDescriptor {
            port_id: port_id("notes"),
            value_kind: kind_id(MUSIC_NOTE_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        },
        PortDescriptor {
            port_id: port_id("controls"),
            value_kind: kind_id(MUSIC_CONTROL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        },
    ]
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != KIND
        || placement.kind_contract_revision.as_str() != REVISION
        || placement.execution_profile_id.as_str() != PROFILE
        || placement.implementation_id.as_str() != IMPLEMENTATION
        || placement.artifact_id.as_str() != ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs != outputs()
        || placement.host_operations != offer().host_operations
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.configuration.is_empty()
    {
        return Err("planned test MIDI source does not match its fixture".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: (EVENT_COUNT + YIELD_COUNT) as u16,
        value_bytes: (2 * conduit_core::NOTE_EVENT_ENCODED_LEN
            + conduit_core::CONTROL_EVENT_ENCODED_LEN
            + YIELD_COUNT) as u32,
        host_requests: YIELD_COUNT,
        sign_items: 16,
        maximum_value_bytes: conduit_core::NOTE_EVENT_ENCODED_LEN
            .max(conduit_core::CONTROL_EVENT_ENCODED_LEN) as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let pitch =
        MusicalPitch::from_equal_tempered(0, crate::hosted_midi::A4_REFERENCE_MILLIHERTZ, 0)
            .map_err(|error| format!("prepare test MIDI pitch: {error:?}"))?;
    let on = MusicalNoteEvent::new(NoteOccurrenceId(41), pitch, Gate::On, u16::MAX, 10, 0)
        .map_err(|error| format!("prepare test MIDI note-on: {error:?}"))?;
    let sustain = MusicalControlEvent::new(MusicalControl::Sustain { down: true }, 11, 1)
        .map_err(|error| format!("prepare test MIDI sustain: {error:?}"))?;
    let off = MusicalNoteEvent::new(NoteOccurrenceId(41), pitch, Gate::Off, 0, 12, 2)
        .map_err(|error| format!("prepare test MIDI note-off: {error:?}"))?;
    let encoded = [
        on.encode().to_vec(),
        sustain.encode().to_vec(),
        off.encode().to_vec(),
    ];
    let stored: [Result<ValueRef, String>; EVENT_COUNT] = core::array::from_fn(|index| {
        values
            .store(&encoded[index])
            .map_err(|error| format!("store test MIDI event: {error:?}"))
    });
    let yield_markers: [Result<ValueRef, String>; YIELD_COUNT] = core::array::from_fn(|_| {
        values
            .store(&[0])
            .map_err(|error| format!("store test MIDI yield marker: {error:?}"))
    });
    Ok(InstalledOperation::TestMidiSource(
        TestMidiSourceOperation {
            values: stored
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| "test MIDI event count changed")?,
            ports: [PortId(0), PortId(1), PortId(0)],
            yield_markers: yield_markers
                .into_iter()
                .collect::<Result<Vec<_>, _>>()?
                .try_into()
                .map_err(|_| "test MIDI yield count changed")?,
            next: 0,
            pending: None,
        },
    ))
}
