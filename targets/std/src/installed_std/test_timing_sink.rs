use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindContractRevision, PlannedGear, PortDescriptor,
    PortDirection, PortTemporal, BOOL_INFO_ID,
};
use conduit_form::{KindDefinition, ProfileCatalog};
use conduit_kernel::{
    BoundedValueRef, HostOperationDisposition, HostOperationId, OperationAction, OperationInput,
    PortId, RequestId, ValueRef, ValueStorage,
};

const KIND: &str = "test/timing-bool-sink";
const REVISION: &str = "conduit-test/timing-bool-sink@1";
const PROFILE: &str = "conduit-test/timing-bool-sink-kernel@1";
const IMPLEMENTATION: &str = "conduit-test/timing-bool-sink-kernel@1";
const ARTIFACT: &str = "conduit-std-host/test-timing-bool-sink@1";
const MAXIMUM_VALUES: usize = 16;
const SOURCE_KIND: &str = "test/timing-bool-source";
const SOURCE_REVISION: &str = "conduit-test/timing-bool-source@1";
const SOURCE_PROFILE: &str = "conduit-test/timing-bool-source-kernel@1";
const SOURCE_IMPLEMENTATION: &str = "conduit-test/timing-bool-source-kernel@1";
const SOURCE_ARTIFACT: &str = "conduit-std-host/test-timing-bool-source@1";

pub(super) static TEST_TIMING_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static TEST_TIMING_SOURCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};

pub(super) struct TestTimingSinkOperation {
    received: usize,
}

pub(super) struct TestTimingSourceOperation {
    pub(super) values: Vec<ValueRef>,
    pub(super) waits: Vec<ValueRef>,
    next: usize,
    pending: Option<RequestId>,
}

impl TestTimingSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if value.byte_len == 1 && self.received < MAXIMUM_VALUES => {
                self.received += 1;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } => OperationAction::Complete,
            _ => InstalledOperation::fail(789),
        }
    }
}

impl TestTimingSourceOperation {
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
                    || InstalledOperation::fail(790),
                    |value| OperationAction::Emit {
                        port: PortId(0),
                        value,
                    },
                )
            }
            _ => InstalledOperation::fail(791),
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

pub(super) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-timing-bool-sink-v1"),
        kind_id: kind_id(KIND),
        kind_contract_revision: KindContractRevision::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("in"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: Vec::new(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 2,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
    }
}

pub(super) fn source_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-timing-bool-source-v1"),
        kind_id: kind_id(SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(SOURCE_PROFILE),
            implementation_id: ImplementationId::from(SOURCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(SOURCE_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("out"),
            value_kind: kind_id(BOOL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
        }],
        host_operations: vec![conduit_core::wait_host_operation_requirement()],
        resource_requirements: vec![conduit_core::resource_requirement(
            conduit_core::TIMER_RESOURCE_CLASS,
            1,
        )],
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: 8,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    for offer in [offer(), source_offer()] {
        catalog
            .insert(KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: Vec::new(),
            })
            .expect("timing fixture is exact and unique");
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: 1,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    Ok(InstalledOperation::TestTimingSink(
        TestTimingSinkOperation { received: 0 },
    ))
}

fn source_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_exact(placement, &source_offer())?;
    Ok(OperationBudget {
        value_items: 6,
        value_bytes: 27,
        host_requests: 3,
        sign_items: 96,
        maximum_value_bytes: 8,
    })
}

fn prepare_source(
    placement: &PlannedGear,
    store: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_exact(placement, &source_offer())?;
    let values = [
        conduit_core::InfoBool::FALSE,
        conduit_core::InfoBool::TRUE,
        conduit_core::InfoBool::FALSE,
    ]
    .into_iter()
    .map(|value| {
        store
            .store(&value.encode())
            .map_err(|error| format!("store timing fixture bool: {error:?}"))
    })
    .collect::<Result<Vec<_>, _>>()?;
    let waits = (0..3)
        .map(|_| {
            store
                .store(&0_u64.to_le_bytes())
                .map_err(|error| format!("store timing fixture wait: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InstalledOperation::TestTimingSource(
        TestTimingSourceOperation {
            values,
            waits,
            next: 0,
            pending: None,
        },
    ))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let offer = offer();
    validate_exact(placement, &offer)
}

fn validate_exact(placement: &PlannedGear, offer: &CapabilityOffer) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.capability_id != offer.capability_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.limits != offer.limits
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
        || (offer.resource_requirements.is_empty() != placement.resources.is_empty())
        || placement.resources.iter().any(|binding| {
            binding.class_id.as_str() != conduit_core::TIMER_RESOURCE_CLASS || binding.units != 1
        })
        || !placement.configuration.is_empty()
    {
        return Err("planned timing sink fixture identity does not match installation".into());
    }
    Ok(())
}
