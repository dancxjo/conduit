use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, present_host_operation_requirement, ArtifactId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, ImplementationId, KindContractRevision,
    PlannedGear, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, ProfileCatalog};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};

const SOURCE_KIND: &str = "conduit-test/json-text-source";
const SOURCE_REVISION: &str = "conduit-test/json-text-source@1";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/json-text-source-kernel@1";
const SINK_KIND: &str = "conduit-test/json-text-sink";
const SINK_REVISION: &str = "conduit-test/json-text-sink@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/json-text-sink-kernel@1";
const PROFILE: &str = "conduit-test/json-codec-kernel@1";
const ARTIFACT: &str = "conduit-std-host/test-json-codec@1";

thread_local! {
    static SOURCE_TEXT: std::cell::RefCell<Option<Vec<u8>>> = const { std::cell::RefCell::new(None) };
}

/// Test-only alternate input, captured in kernel value storage before Play.
pub(super) fn with_source_text<T>(input: &[u8], run: impl FnOnce() -> T) -> T {
    assert!(input.len() <= conduit_web::JSON_MAXIMUM_ENCODED_BYTES);
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            SOURCE_TEXT.with(|text| *text.borrow_mut() = None);
        }
    }
    SOURCE_TEXT.with(|text| {
        assert!(text.borrow().is_none(), "nested JSON fixture input");
        *text.borrow_mut() = Some(input.to_vec());
    });
    let _reset = Reset;
    run()
}

pub(super) static TEST_JSON_SOURCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};
pub(super) static TEST_JSON_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct TestJsonSourceOperation {
    value: ValueRef,
    emitted: bool,
}
pub(super) struct TestJsonSinkOperation {
    pending: bool,
}

impl TestJsonSourceOperation {
    pub(super) fn emit_or_complete(&self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Emit {
                port: PortId(0),
                value: self.value,
            }
        }
    }
    pub(super) fn advance(&mut self) -> OperationAction {
        self.emitted = true;
        OperationAction::Complete
    }
}

impl TestJsonSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(
                        value,
                        conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
                    )
                    .unwrap(),
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                self.pending = false;
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(105),
        }
    }
    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

pub(crate) fn source_offer() -> CapabilityOffer {
    offer(
        SOURCE_KIND,
        SOURCE_REVISION,
        SOURCE_IMPLEMENTATION,
        PortDirection::Output,
        Vec::new(),
    )
}
pub(crate) fn sink_offer() -> CapabilityOffer {
    offer(
        SINK_KIND,
        SINK_REVISION,
        SINK_IMPLEMENTATION,
        PortDirection::Input,
        vec![present_host_operation_requirement(
            kind_id("presentation/stdout-text"),
            conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
        )],
    )
}

fn offer(
    kind: &str,
    revision: &str,
    implementation: &str,
    direction: PortDirection,
    host_operations: Vec<conduit_core::HostOperationRequirement>,
) -> CapabilityOffer {
    let descriptor = PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(conduit_web::JSON_TEXT_INFO_ID),
        direction,
        temporal: PortTemporal::Value,
    };
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(kind),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: if direction == PortDirection::Input {
            vec![descriptor.clone()]
        } else {
            Vec::new()
        },
        outputs: if direction == PortDirection::Output {
            vec![descriptor]
        } else {
            Vec::new()
        },
        host_operations,
        resource_requirements: if direction == PortDirection::Input {
            vec![conduit_core::resource_requirement(
                conduit_core::PRESENTATION_RESOURCE_CLASS,
                1,
            )]
        } else {
            Vec::new()
        },
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    for offer in [source_offer(), sink_offer()] {
        catalog
            .insert(KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: Vec::new(),
            })
            .unwrap();
    }
}

fn validate(placement: &PlannedGear, offer: CapabilityOffer) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
    {
        Err("planned JSON fixture differs".into())
    } else {
        Ok(())
    }
}
fn source_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, source_offer())?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
    })
}
fn sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, sink_offer())?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
    })
}
fn prepare_source(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    source_budget(placement)?;
    let value = SOURCE_TEXT
        .with(|text| {
            let text = text.borrow();
            values.store(text.as_deref().unwrap_or(b" {\"z\":1.2300,\"a\":\"ok\"} "))
        })
        .map_err(|error| format!("store JSON fixture: {error:?}"))?;
    Ok(InstalledOperation::TestJsonSource(
        TestJsonSourceOperation {
            value,
            emitted: false,
        },
    ))
}
fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    sink_budget(placement)?;
    Ok(InstalledOperation::TestJsonSink(TestJsonSinkOperation {
        pending: false,
    }))
}
