use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, resource_requirement, ArtifactId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ExecutionProfileId, ImplementationId, InfoBool, KindIdentity, PlannedGear,
    PortDescriptor, PortDirection, PortTemporal, Scalar, BOOL_INFO_ID, SCALAR_ENCODED_LEN,
    SCALAR_INFO_ID, TIMER_RESOURCE_CLASS,
};
use conduit_form::{KindProjection, ProfileCatalog};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, OperationAction, OperationInput, PortId,
    RequestId, ValueRef, ValueStorage,
};

const SOURCE_KIND: &str = "conduit-test/gate-script";
const SOURCE_REVISION: &str = "conduit-test/gate-script@1";
const SOURCE_PROFILE: &str = "conduit-test/gate-script-kernel@1";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/gate-script-kernel@1";
const SOURCE_ARTIFACT: &str = "conduit-std-host/test-gate-script@1";

const SLOW_SINK_KIND: &str = "conduit-test/slow-scalar-sink";
const SLOW_SINK_REVISION: &str = "conduit-test/slow-scalar-sink@1";
const SLOW_SINK_PROFILE: &str = "conduit-test/slow-scalar-sink-kernel@1";
const SLOW_SINK_IMPLEMENTATION: &str = "conduit-test/slow-scalar-sink-kernel@1";
const SLOW_SINK_ARTIFACT: &str = "conduit-std-host/test-slow-scalar-sink@1";

const SCRIPT_ITEMS: usize = 6;
const EXPECTED_SCALARS: usize = 3;

pub(super) static TEST_GATE_SCRIPT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};

pub(super) static TEST_SLOW_SCALAR_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SLOW_SINK_IMPLEMENTATION,
    budget: slow_sink_budget,
    prepare: prepare_slow_sink,
};

pub(super) struct TestGateScriptOperation {
    pub(super) items: Vec<(PortId, ValueRef)>,
    pub(super) waits: Vec<ValueRef>,
    pub(super) next: usize,
    pending: Option<RequestId>,
}

pub(super) struct TestSlowScalarSinkOperation {
    pub(super) waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl<const PORTS: usize> StepOperation<PORTS> for TestGateScriptOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return gate_fixture_fail(17);
            }
            let Some((port, value)) = self.items.get(self.next).copied() else {
                return gate_fixture_fail(17);
            };
            if !io.output_ready(port) {
                return StepOutcome::Await;
            }
            io.consume_host_completion()
                .expect("observed gate fixture wait");
            io.send(port, value).expect("ready gate fixture output");
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
                BoundedValueRef::new(wait, 8).expect("gate fixture wait is bounded"),
            )
            .expect("gate fixture wait Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl<const PORTS: usize> StepOperation<PORTS> for TestSlowScalarSinkOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return gate_fixture_fail(18);
            }
            io.consume_host_completion()
                .expect("observed slow sink wait");
            self.pending = None;
            self.next += 1;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len != SCALAR_ENCODED_LEN as u32
                || self.pending.is_some()
                || self.next >= self.waits.len()
            {
                return gate_fixture_fail(18);
            }
            let request = RequestId(u32::try_from(self.next).unwrap_or(u32::MAX));
            io.consume(PortId(0)).expect("present slow scalar input");
            io.request_host_call(
                request,
                HostCallId(0),
                BoundedValueRef::new(self.waits[self.next], 8)
                    .expect("slow sink wait is exactly eight bytes"),
            )
            .expect("slow sink wait Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() && self.next == self.waits.len() {
            io.consume_closed(PortId(0))
                .expect("observed slow sink closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

const fn gate_fixture_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

impl TestGateScriptOperation {}

impl TestSlowScalarSinkOperation {}

pub(super) fn source_offer() -> CapabilityOffer {
    offer(
        SOURCE_KIND,
        SOURCE_REVISION,
        SOURCE_PROFILE,
        SOURCE_IMPLEMENTATION,
        SOURCE_ARTIFACT,
        Vec::new(),
        vec![
            port(
                "scalar",
                SCALAR_INFO_ID,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            ),
            port(
                "enable",
                BOOL_INFO_ID,
                PortDirection::Output,
                PortTemporal::Current,
            ),
        ],
    )
}

pub(super) fn slow_sink_offer() -> CapabilityOffer {
    offer(
        SLOW_SINK_KIND,
        SLOW_SINK_REVISION,
        SLOW_SINK_PROFILE,
        SLOW_SINK_IMPLEMENTATION,
        SLOW_SINK_ARTIFACT,
        vec![port(
            "in",
            SCALAR_INFO_ID,
            PortDirection::Input,
            PortTemporal::Current,
        )],
        Vec::new(),
    )
}

fn offer(
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
        capability_id: CapabilityId::from(implementation),
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs,
        outputs,
        host_calls: vec![conduit_core::wait_host_call_requirement()],
        resource_requirements: vec![resource_requirement(TIMER_RESOURCE_CLASS, 1)],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: SCALAR_ENCODED_LEN as u32,
        },
    }
}

fn port(
    name: &str,
    info: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(info),
        direction,
        temporal,
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    for (offer, configuration) in [
        (source_offer(), Vec::new()),
        (slow_sink_offer(), Vec::new()),
        (
            conduit_std_offers::flow_gate_scalar_offer(),
            vec![conduit_form::KindConfigurationField {
                key: "maximum-enable-updates".into(),
                default_value: conduit_core::ConfigurationValue::U64(
                    conduit_semantic_catalog::FLOW_STATE_MAXIMUM_VALUES.into(),
                ),
                rule: conduit_form::KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: conduit_semantic_catalog::FLOW_STATE_MAXIMUM_VALUES.into(),
                },
            }],
        ),
    ] {
        catalog
            .insert(KindProjection {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration,
            })
            .expect("gate fixture Kind is exact and unique");
    }
}

fn source_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &source_offer())?;
    Ok(OperationBudget {
        value_items: (SCRIPT_ITEMS * 2) as u16,
        value_bytes: 75,
        host_requests: SCRIPT_ITEMS,
        sign_items: 128,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare_source(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, &source_offer())?;
    let encoded = [
        (PortId(1), InfoBool::FALSE.encode().to_vec()),
        (
            PortId(0),
            Scalar::from_raw_microunits(-1_000_000).encode().to_vec(),
        ),
        (PortId(1), InfoBool::TRUE.encode().to_vec()),
        (PortId(0), Scalar::ZERO.encode().to_vec()),
        (PortId(1), InfoBool::FALSE.encode().to_vec()),
        (PortId(0), Scalar::ONE.encode().to_vec()),
    ];
    let items = encoded
        .iter()
        .map(|(port, bytes)| {
            values
                .store(bytes)
                .map(|value| (*port, value))
                .map_err(|error| format!("store gate script value: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let waits = (0..SCRIPT_ITEMS)
        .map(|_| {
            values
                .store(&0_u64.to_le_bytes())
                .map_err(|error| format!("store gate script wait: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstalledOperation::TestGateScript(
        TestGateScriptOperation {
            items,
            waits,
            next: 0,
            pending: None,
        },
    ))
}

fn slow_sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &slow_sink_offer())?;
    Ok(OperationBudget {
        value_items: EXPECTED_SCALARS as u16,
        value_bytes: (EXPECTED_SCALARS * SCALAR_ENCODED_LEN) as u32,
        host_requests: EXPECTED_SCALARS,
        sign_items: 96,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare_slow_sink(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, &slow_sink_offer())?;
    let waits = (0..EXPECTED_SCALARS)
        .map(|_| {
            values
                .store(&0_u64.to_le_bytes())
                .map_err(|error| format!("store slow sink wait: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstalledOperation::TestSlowScalarSink(
        TestSlowScalarSinkOperation {
            waits,
            next: 0,
            pending: None,
        },
    ))
}

fn validate(placement: &PlannedGear, offer: &CapabilityOffer) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || !placement.configuration.is_empty()
    {
        return Err("planned gate fixture identity does not match its installation".into());
    }
    Ok(())
}
