use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, ImplementationId, KindIdentity, PlannedGear,
    PortDescriptor, PortDirection, PortTemporal, Scalar, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
    TIMER_RESOURCE_CLASS,
};
use conduit_form::{KindConfigurationField, KindConfigurationRule, KindProjection, ProfileCatalog};
use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId, ValueRef, ValueStorage,
};

const SOURCE_KIND: &str = "conduit-test/scalar-source";
const SOURCE_REVISION: &str = "conduit-test/scalar-source@1";
const SOURCE_PROFILE: &str = "conduit-test/scalar-source-kernel@1";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/scalar-source-kernel@1";
const SOURCE_ARTIFACT: &str = "conduit-std-host/test-scalar-source@1";

const LITERAL_KIND: &str = "conduit-test/scalar-literal";
const LITERAL_REVISION: &str = "conduit-test/scalar-literal@1";
const LITERAL_PROFILE: &str = "conduit-test/scalar-literal-kernel@1";
const LITERAL_IMPLEMENTATION: &str = "conduit-test/scalar-literal-kernel@1";
const LITERAL_ARTIFACT: &str = "conduit-std-host/test-scalar-literal@1";

const SINK_KIND: &str = "conduit-test/scalar-sink";
const SINK_REVISION: &str = "conduit-test/scalar-sink@1";
const SINK_PROFILE: &str = "conduit-test/scalar-sink-kernel@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/scalar-sink-kernel@1";
const SINK_ARTIFACT: &str = "conduit-std-host/test-scalar-sink@1";
const EXPECTED_VALUES: u64 = 3;

pub(super) static TEST_SCALAR_SOURCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};

pub(super) static TEST_SCALAR_LITERAL_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: LITERAL_IMPLEMENTATION,
    budget: literal_budget,
    prepare: prepare_literal,
};

pub(super) static TEST_SCALAR_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct TestScalarSourceOperation {
    pub(super) values: Vec<ValueRef>,
    pub(super) waits: Vec<ValueRef>,
    pub(super) next: usize,
    pending: Option<RequestId>,
}

pub(super) struct TestScalarLiteralOperation {
    pub(super) value: ValueRef,
    pub(super) emitted: bool,
}

impl<const PORTS: usize> StepOperation<PORTS> for TestScalarLiteralOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        io.send(PortId(0), self.value)
            .expect("ready scalar literal output");
        self.emitted = true;
        StepOutcome::Complete
    }
}

impl TestScalarLiteralOperation {}

pub(super) struct TestScalarSinkOperation {
    seen: u64,
    expected: u64,
}

impl<const PORTS: usize> StepOperation<PORTS> for TestScalarSourceOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
            {
                return scalar_fixture_fail(14);
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            let Some(value) = self.values.get(self.next).copied() else {
                return scalar_fixture_fail(14);
            };
            io.consume_host_completion()
                .expect("observed scalar fixture wait");
            io.send(PortId(0), value)
                .expect("ready scalar fixture output");
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
                BoundedValueRef::new(wait, 8).expect("scalar fixture wait is bounded"),
            )
            .expect("scalar fixture wait Host Call");
            self.pending = Some(request);
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl<const PORTS: usize> StepOperation<PORTS> for TestScalarSinkOperation {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some(value) = io.input(PortId(0)) {
            if value.byte_len != SCALAR_ENCODED_LEN as u32 || self.seen >= self.expected {
                return scalar_fixture_fail(15);
            }
            io.consume(PortId(0)).expect("present scalar fixture input");
            self.seen += 1;
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.seen == self.expected {
            io.consume_closed(PortId(0))
                .expect("observed scalar fixture closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

const fn scalar_fixture_fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

impl TestScalarSourceOperation {}

impl TestScalarSinkOperation {}

pub(super) fn source_offer() -> CapabilityOffer {
    let mut offer = offer(
        TestIdentity {
            kind: SOURCE_KIND,
            revision: SOURCE_REVISION,
            profile: SOURCE_PROFILE,
            implementation: SOURCE_IMPLEMENTATION,
            artifact: SOURCE_ARTIFACT,
            max_active_instances: 1,
        },
        Vec::new(),
        vec![scalar_port(
            "value",
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )],
    );
    offer.host_calls = vec![conduit_core::wait_host_call_requirement()];
    offer.resource_requirements = vec![conduit_core::resource_requirement(TIMER_RESOURCE_CLASS, 1)];
    offer
}

pub(super) fn literal_offer() -> CapabilityOffer {
    offer(
        TestIdentity {
            kind: LITERAL_KIND,
            revision: LITERAL_REVISION,
            profile: LITERAL_PROFILE,
            implementation: LITERAL_IMPLEMENTATION,
            artifact: LITERAL_ARTIFACT,
            max_active_instances: 1,
        },
        Vec::new(),
        vec![scalar_port(
            "value",
            PortDirection::Output,
            PortTemporal::Value,
        )],
    )
}

pub(super) fn sink_offer() -> CapabilityOffer {
    offer(
        TestIdentity {
            kind: SINK_KIND,
            revision: SINK_REVISION,
            profile: SINK_PROFILE,
            implementation: SINK_IMPLEMENTATION,
            artifact: SINK_ARTIFACT,
            max_active_instances: 2,
        },
        vec![scalar_port(
            "in",
            PortDirection::Input,
            PortTemporal::Current,
        )],
        Vec::new(),
    )
}

struct TestIdentity {
    kind: &'static str,
    revision: &'static str,
    profile: &'static str,
    implementation: &'static str,
    artifact: &'static str,
    max_active_instances: u16,
}

fn offer(
    identity: TestIdentity,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: if identity.kind == SINK_KIND {
            vec![conduit_core::FrontStartupParameter {
                name: "expected".into(),
                value_type: conduit_core::kind_id("value/count"),
                has_default: true,
            }]
        } else {
            Vec::new()
        },
        shorthand: None,
        capability_id: CapabilityId::from(identity.implementation),
        kind_id: kind_id(identity.kind),
        kind_contract_revision: KindIdentity::from(identity.revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(identity.profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
        },
        inputs,
        outputs,
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: identity.max_active_instances,
            max_queue_items: 4,
            max_queue_bytes: 32,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    for definition in [
        KindProjection {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindIdentity::from(SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: source_offer().outputs,
            configuration: Default::default(),
        },
        KindProjection {
            kind_id: kind_id(LITERAL_KIND),
            kind_contract_revision: KindIdentity::from(LITERAL_REVISION),
            inputs: Vec::new(),
            outputs: literal_offer().outputs,
            configuration: Default::default(),
        },
        KindProjection {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindIdentity::from(SINK_REVISION),
            inputs: sink_offer().inputs,
            outputs: Vec::new(),
            configuration: vec![KindConfigurationField {
                key: "expected".into(),
                default_value: ConfigurationValue::U64(EXPECTED_VALUES),
                rule: KindConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: EXPECTED_VALUES,
                },
            }],
        },
        KindProjection {
            kind_id: kind_id(conduit_semantic_catalog::LATEST_KIND),
            kind_contract_revision: KindIdentity::from(
                conduit_semantic_catalog::STATE_LATEST_SCALAR_CONTRACT_REVISION,
            ),
            inputs: conduit_semantic_catalog::state_latest_scalar_contract().inputs,
            outputs: conduit_semantic_catalog::state_latest_scalar_contract().outputs,
            configuration: Default::default(),
        },
        KindProjection {
            kind_id: kind_id(conduit_semantic_catalog::TEE_KIND),
            kind_contract_revision: KindIdentity::from(
                conduit_semantic_catalog::FLOW_TEE_SCALAR_CONTRACT_REVISION,
            ),
            inputs: conduit_semantic_catalog::flow_tee_scalar_contract().inputs,
            outputs: conduit_semantic_catalog::flow_tee_scalar_contract().outputs,
            configuration: Default::default(),
        },
    ] {
        catalog
            .insert(definition)
            .expect("scalar flow fixture kind is exact and unique");
    }
}

fn scalar_port(name: &str, direction: PortDirection, temporal: PortTemporal) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_INFO_ID),
        direction,
        temporal,
    }
}

fn source_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_source(placement)?;
    Ok(OperationBudget {
        value_items: (EXPECTED_VALUES * 2) as u16,
        value_bytes: (SCALAR_ENCODED_LEN as u64 * EXPECTED_VALUES * 2) as u32,
        host_requests: EXPECTED_VALUES as usize,
        sign_items: 64,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn literal_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_literal(placement)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: SCALAR_ENCODED_LEN as u32,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare_literal(
    placement: &PlannedGear,
    store: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_literal(placement)?;
    let value = store
        .store(&Scalar::from_raw_microunits(-1).encode())
        .map_err(|error| format!("store scalar literal fixture: {error:?}"))?;
    Ok(InstalledOperation::TestScalarLiteral(
        TestScalarLiteralOperation {
            value,
            emitted: false,
        },
    ))
}

fn prepare_source(
    placement: &PlannedGear,
    store: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_source(placement)?;
    let values = [-1_000_000_i64, 0, 1_000_000]
        .into_iter()
        .map(|raw| {
            store
                .store(&Scalar::from_raw_microunits(raw).encode())
                .map_err(|error| format!("store scalar fixture: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let waits = (0..EXPECTED_VALUES)
        .map(|_| {
            store
                .store(&0_u64.to_le_bytes())
                .map_err(|error| format!("store scalar fixture wait: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstalledOperation::TestScalarSource(
        TestScalarSourceOperation {
            values,
            waits,
            next: 0,
            pending: None,
        },
    ))
}

fn sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_sink(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_sink(placement)?;
    Ok(InstalledOperation::TestScalarSink(
        TestScalarSinkOperation {
            seen: 0,
            expected: expected(placement)?,
        },
    ))
}

fn expected(placement: &PlannedGear) -> Result<u64, String> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            ("expected", ConfigurationValue::U64(value)) if *value <= EXPECTED_VALUES => {
                Some(*value)
            }
            _ => None,
        })
        .ok_or_else(|| "scalar sink expected count is missing or invalid".into())
}

fn validate_source(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != SOURCE_KIND
        || placement.kind_contract_revision.as_str() != SOURCE_REVISION
        || placement.execution_profile_id.as_str() != SOURCE_PROFILE
        || placement.implementation_id.as_str() != SOURCE_IMPLEMENTATION
        || placement.artifact_id.as_str() != SOURCE_ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs != source_offer().outputs
        || !placement.configuration.is_empty()
    {
        return Err("planned scalar source does not match its fixture".into());
    }
    Ok(())
}

fn validate_literal(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != LITERAL_KIND
        || placement.kind_contract_revision.as_str() != LITERAL_REVISION
        || placement.execution_profile_id.as_str() != LITERAL_PROFILE
        || placement.implementation_id.as_str() != LITERAL_IMPLEMENTATION
        || placement.artifact_id.as_str() != LITERAL_ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs != literal_offer().outputs
        || !placement.configuration.is_empty()
    {
        return Err("planned scalar literal does not match its fixture".into());
    }
    Ok(())
}

fn validate_sink(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != SINK_KIND
        || placement.kind_contract_revision.as_str() != SINK_REVISION
        || placement.execution_profile_id.as_str() != SINK_PROFILE
        || placement.implementation_id.as_str() != SINK_IMPLEMENTATION
        || placement.artifact_id.as_str() != SINK_ARTIFACT
        || placement.inputs != sink_offer().inputs
        || !placement.outputs.is_empty()
        || placement.configuration.len() != 1
    {
        return Err("planned scalar sink does not match its fixture".into());
    }
    expected(placement).map(|_| ())
}
