use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, ImplementationId, KindContractRevision, PlannedGear,
    PortDescriptor, PortDirection, PortTemporal, Scalar, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
    TIMER_RESOURCE_CLASS,
};
use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};

const SOURCE_KIND: &str = "conduit.test/scalar-source";
const SOURCE_REVISION: &str = "conduit.test/scalar-source@1";
const SOURCE_PROFILE: &str = "conduit.test/scalar-source-kernel@1";
const SOURCE_IMPLEMENTATION: &str = "conduit.test/scalar-source-kernel@1";
const SOURCE_ARTIFACT: &str = "conduit-std-host/test-scalar-source@1";

const SINK_KIND: &str = "conduit.test/scalar-sink";
const SINK_REVISION: &str = "conduit.test/scalar-sink@1";
const SINK_PROFILE: &str = "conduit.test/scalar-sink-kernel@1";
const SINK_IMPLEMENTATION: &str = "conduit.test/scalar-sink-kernel@1";
const SINK_ARTIFACT: &str = "conduit-std-host/test-scalar-sink@1";
const EXPECTED_VALUES: u64 = 3;

pub(super) static TEST_SCALAR_SOURCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
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

pub(super) struct TestScalarSinkOperation {
    seen: u64,
    expected: u64,
}

impl TestScalarSourceOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.request_wait().unwrap_or(OperationAction::Complete)
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
                self.values.get(self.next).copied().map_or_else(
                    || InstalledOperation::fail(14),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            _ => InstalledOperation::fail(14),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next += 1;
        self.request_wait().unwrap_or(OperationAction::Complete)
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    fn request_wait(&mut self) -> Option<OperationAction> {
        let input = self.waits.get(self.next).copied()?;
        let request = RequestId(u32::try_from(self.next).ok()?);
        self.pending = Some(request);
        Some(OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(input, 8).ok()?,
        })
    }
}

impl TestScalarSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if value.byte_len == SCALAR_ENCODED_LEN as u32 && self.seen < self.expected => {
                self.seen += 1;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.seen == self.expected => {
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(15),
        }
    }
}

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
    offer.host_operations = vec![conduit_core::wait_host_operation_requirement()];
    offer.resource_requirements = vec![conduit_core::resource_requirement(TIMER_RESOURCE_CLASS, 1)];
    offer
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
            vec![conduit_core::FaceStartupParameter {
                name: "expected".into(),
                value_type: "Count".into(),
                has_default: true,
            }]
        } else {
            Vec::new()
        },
        shorthand: None,
        capability_id: CapabilityId::from(identity.implementation),
        kind_id: kind_id(identity.kind),
        kind_contract_revision: KindContractRevision::from(identity.revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(identity.profile),
            implementation_id: ImplementationId::from(identity.implementation),
            artifact_id: ArtifactId::from(identity.artifact),
        },
        inputs,
        outputs,
        host_operations: Vec::new(),
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
        KindDefinition {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: source_offer().outputs,
            configuration: Vec::new(),
        },
        KindDefinition {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindContractRevision::from(SINK_REVISION),
            inputs: sink_offer().inputs,
            outputs: Vec::new(),
            configuration: vec![ConfigurationField {
                key: "expected".into(),
                default_value: ConfigurationValue::U64(EXPECTED_VALUES),
                validation: ConfigurationRule::U64Range {
                    minimum: 0,
                    maximum: EXPECTED_VALUES,
                },
            }],
        },
        KindDefinition {
            kind_id: kind_id(conduit_std_catalog::LATEST_KIND),
            kind_contract_revision: KindContractRevision::from(
                conduit_std_catalog::STATE_LATEST_SCALAR_CONTRACT_REVISION,
            ),
            inputs: conduit_std_catalog::state_latest_scalar_contract().inputs,
            outputs: conduit_std_catalog::state_latest_scalar_contract().outputs,
            configuration: Vec::new(),
        },
        KindDefinition {
            kind_id: kind_id(conduit_std_catalog::TEE_KIND),
            kind_contract_revision: KindContractRevision::from(
                conduit_std_catalog::FLOW_TEE_SCALAR_CONTRACT_REVISION,
            ),
            inputs: conduit_std_catalog::flow_tee_scalar_contract().inputs,
            outputs: conduit_std_catalog::flow_tee_scalar_contract().outputs,
            configuration: Vec::new(),
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
