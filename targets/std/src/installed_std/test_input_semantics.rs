use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindIdentity, PlannedGear, PortDescriptor, PortDirection,
    PortTemporal, TIMER_RESOURCE_CLASS,
};
use conduit_form::{KindProjection, ProfileCatalog};
use conduit_human::{
    ChordInfo, CoreChordId, KeyEvent, KeyModifiers, KeyTransition, CHORD_ENCODED_LEN,
    CHORD_INFO_ID, KEY_EVENT_ENCODED_LEN, KEY_EVENT_INFO_ID,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

const SOURCE_KIND: &str = "conduit-test/key-event-source";
const SOURCE_REVISION: &str = "conduit-test/key-event-source@1";
const SOURCE_PROFILE: &str = "conduit-test/key-event-source-kernel@1";
pub(super) const SOURCE_IMPLEMENTATION: &str = "conduit-test/key-event-source-kernel@1";
const SOURCE_ARTIFACT: &str = "conduit-std-host/test-key-event-source@1";

const SINK_KIND: &str = "conduit-test/chord-sink";
const SINK_REVISION: &str = "conduit-test/chord-sink@1";
const SINK_PROFILE: &str = "conduit-test/chord-sink-kernel@1";
pub(super) const SINK_IMPLEMENTATION: &str = "conduit-test/chord-sink-kernel@1";
const SINK_ARTIFACT: &str = "conduit-std-host/test-chord-sink@1";

pub(super) static TEST_KEY_EVENT_SOURCE_FACTORY: BackFactory = BackFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};

pub(super) static TEST_CHORD_SINK_FACTORY: BackFactory = BackFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct TestKeyEventSourceBack {
    pub(super) values: Vec<ValueRef>,
    pub(super) waits: Vec<ValueRef>,
    pub(super) next: usize,
    pending: Option<RequestId>,
}

pub(super) struct TestChordSinkBack {
    observed: u8,
}

impl<const PORTS: usize> StepBack<PORTS> for TestKeyEventSourceBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return key_fixture_fail(60);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(value) = self.values.get(self.next).copied() else {
                return key_fixture_fail(60);
            };
            io.consume_host_completion()
                .expect("observed key fixture wait");
            io.send(PortId(0), value).expect("ready key fixture output");
            self.pending = None;
            self.next += 1;
            return StepOutcome::Progress;
        }
        let Some(wait) = self.waits.get(self.next).copied() else {
            return StepOutcome::Complete;
        };
        if self.pending.is_none() {
            let request = RequestId(u32::try_from(self.next).unwrap_or(u32::MAX));
            io.request_host_call(
                request,
                HostCallId(0),
                BoundedValueRef::new(wait, 8).expect("key fixture wait is bounded"),
            )
            .expect("key fixture wait Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl<const PORTS: usize> StepBack<PORTS> for TestChordSinkBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        let Some(_) = io.input(PortId(0)) else {
            return StepOutcome::Await;
        };
        let valid = self.observed == 0
            && input_bytes.input(PortId(0)).is_some_and(|bytes| {
                ChordInfo::decode(bytes)
                    .is_ok_and(|chord| chord.chord_id() == CoreChordId::CancelOrEscape)
            });
        if !valid {
            return key_fixture_fail(63);
        }
        io.consume(PortId(0)).expect("present chord fixture input");
        self.observed = 1;
        StepOutcome::Complete
    }
}

const fn key_fixture_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

impl TestKeyEventSourceBack {}

impl TestChordSinkBack {}

pub(super) fn source_offer() -> CapabilityOffer {
    offer(
        "test-key-event-source",
        SOURCE_KIND,
        SOURCE_REVISION,
        SOURCE_PROFILE,
        SOURCE_IMPLEMENTATION,
        SOURCE_ARTIFACT,
        Vec::new(),
        vec![source_port()],
    )
}

pub(super) fn sink_offer() -> CapabilityOffer {
    offer(
        "test-chord-sink",
        SINK_KIND,
        SINK_REVISION,
        SINK_PROFILE,
        SINK_IMPLEMENTATION,
        SINK_ARTIFACT,
        vec![sink_port()],
        Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
fn offer(
    capability: &str,
    kind: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs,
        outputs,
        host_calls: if kind == SOURCE_KIND {
            vec![conduit_core::wait_host_call_requirement()]
        } else {
            Vec::new()
        },
        resource_requirements: if kind == SOURCE_KIND {
            vec![conduit_core::resource_requirement(TIMER_RESOURCE_CLASS, 1)]
        } else {
            Vec::new()
        },
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 8,
            max_queue_bytes: 32,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    for (kind, revision, inputs, outputs) in [
        (
            SOURCE_KIND,
            SOURCE_REVISION,
            Vec::new(),
            vec![source_port()],
        ),
        (SINK_KIND, SINK_REVISION, vec![sink_port()], Vec::new()),
    ] {
        catalog
            .insert(KindProjection {
                kind_id: kind_id(kind),
                kind_contract_revision: KindIdentity::from(revision),
                inputs,
                outputs,
                configuration: Default::default(),
            })
            .expect("test input semantic kind is unique");
    }
}

fn source_port() -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("key"),
        value_kind: kind_id(KEY_EVENT_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Flow { closes: true },
    }
}

fn sink_port() -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("chord"),
        value_kind: kind_id(CHORD_INFO_ID),
        direction: PortDirection::Input,
        temporal: PortTemporal::Flow { closes: false },
    }
}

fn source_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_source(placement)?;
    Ok(BackBudget {
        value_items: 16,
        value_bytes: 8 * KEY_EVENT_ENCODED_LEN as u32 + 64,
        host_requests: 8,
        sign_items: 64,
        maximum_value_bytes: KEY_EVENT_ENCODED_LEN as u32,
    })
}

fn sink_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate_sink(placement)?;
    Ok(BackBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: CHORD_ENCODED_LEN as u32,
    })
}

fn prepare_source(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate_source(placement)?;
    let specs = [
        (0x04, KeyTransition::Pressed, 0),
        (0x04, KeyTransition::Released, 0),
        (
            0xe0,
            KeyTransition::Pressed,
            KeyModifiers::LEFT_CONTROL.bits(),
        ),
        (
            0x0a,
            KeyTransition::Pressed,
            KeyModifiers::LEFT_CONTROL.bits(),
        ),
        (
            0x0a,
            KeyTransition::Released,
            KeyModifiers::LEFT_CONTROL.bits(),
        ),
        (0xe0, KeyTransition::Released, 0),
        (0x05, KeyTransition::Pressed, 0),
        (0x05, KeyTransition::Released, 0),
    ];
    let event_values = specs
        .into_iter()
        .map(|(usage, transition, modifiers)| {
            let event = KeyEvent::new(usage, transition, KeyModifiers::from_bits(modifiers))
                .map_err(|error| format!("build test key event: {error:?}"))?;
            values
                .store(&event.encode())
                .map_err(|error| format!("store test key event: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let waits = (0..specs.len())
        .map(|_| {
            values
                .store(&conduit_time::encode_tick(0))
                .map_err(|error| format!("store test key wait: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstalledBack::TestKeyEventSource(TestKeyEventSourceBack {
        values: event_values,
        waits,
        next: 0,
        pending: None,
    }))
}

fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate_sink(placement)?;
    Ok(InstalledBack::TestChordSink(TestChordSinkBack {
        observed: 0,
    }))
}

fn validate_source(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        SOURCE_KIND,
        SOURCE_REVISION,
        SOURCE_PROFILE,
        SOURCE_IMPLEMENTATION,
        SOURCE_ARTIFACT,
        &[],
        &[source_port()],
    )
}

fn validate_sink(placement: &PlannedGear) -> Result<(), String> {
    validate(
        placement,
        SINK_KIND,
        SINK_REVISION,
        SINK_PROFILE,
        SINK_IMPLEMENTATION,
        SINK_ARTIFACT,
        &[sink_port()],
        &[],
    )
}

#[allow(clippy::too_many_arguments)]
fn validate(
    placement: &PlannedGear,
    kind: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    inputs: &[PortDescriptor],
    outputs: &[PortDescriptor],
) -> Result<(), String> {
    if placement.kind_id.as_str() != kind
        || placement.kind_contract_revision.as_str() != revision
        || placement.execution_profile_id.as_str() != profile
        || placement.implementation_id.as_str() != implementation
        || placement.artifact_id.as_str() != artifact
        || placement.inputs != inputs
        || placement.outputs != outputs
        || !placement.configuration.is_empty()
    {
        return Err("planned test input semantic identity does not match its fixture".into());
    }
    Ok(())
}
