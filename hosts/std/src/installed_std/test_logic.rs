use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindContractRevision, PlannedGear, PortDescriptor,
    PortDirection, PortTemporal, Scalar, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};
use conduit_form::{KindDefinition, ProfileCatalog};
use conduit_kernel::{OperationAction, PortId, ValueRef, ValueStorage};

const KIND: &str = "conduit-test/logic-script";
const REVISION: &str = "conduit-test/logic-script@1";
const PROFILE: &str = "conduit-test/logic-script-kernel@1";
const IMPLEMENTATION: &str = "conduit-test/logic-script-kernel@1";
const ARTIFACT: &str = "conduit-std-host/test-logic-script@1";
const SINK_KIND: &str = "conduit-test/logic-sink";
const SINK_REVISION: &str = "conduit-test/logic-sink@1";
const SINK_PROFILE: &str = "conduit-test/logic-sink-kernel@1";
const SINK_IMPLEMENTATION: &str = "conduit-test/logic-sink-kernel@1";
const SINK_ARTIFACT: &str = "conduit-std-host/test-logic-sink@1";

pub(super) static TEST_LOGIC_SCRIPT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) static TEST_LOGIC_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct TestLogicScriptOperation {
    pub(super) values: [ValueRef; 4],
    pub(super) next: usize,
}

pub(super) struct TestLogicSinkOperation;

impl TestLogicScriptOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.emit_or_complete()
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next += 1;
        self.emit_or_complete()
    }

    fn emit_or_complete(&self) -> OperationAction {
        self.values
            .get(self.next)
            .copied()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(u16::try_from(self.next).unwrap_or(u16::MAX)),
                value,
            })
    }
}

impl TestLogicSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        if port == PortId(0)
            && value.byte_len == SCALAR_ENCODED_LEN as u32
            && Scalar::decode(canonical) == Ok(Scalar::from_raw_microunits(-1))
        {
            OperationAction::Complete
        } else {
            InstalledOperation::fail(24)
        }
    }
}

pub(super) fn offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-logic-script"),
        kind_id: kind_id(KIND),
        kind_contract_revision: KindContractRevision::from(REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: ["compare-left", "compare-right", "when-false", "when-true"]
            .into_iter()
            .map(scalar_output)
            .collect(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: SCALAR_ENCODED_LEN as u32,
        },
    }
}

pub(super) fn sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("test-logic-sink"),
        kind_id: kind_id(SINK_KIND),
        kind_contract_revision: KindContractRevision::from(SINK_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(SINK_PROFILE),
            implementation_id: ImplementationId::from(SINK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(SINK_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("in"),
            value_kind: kind_id(SCALAR_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: SCALAR_ENCODED_LEN as u32,
        },
    }
}

pub(super) fn install_catalog(catalog: &mut ProfileCatalog) {
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(KIND),
            kind_contract_revision: KindContractRevision::from(REVISION),
            inputs: Vec::new(),
            outputs: offer().outputs,
            configuration: Vec::new(),
        })
        .expect("logic script fixture kind is unique");
    catalog
        .insert(KindDefinition {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindContractRevision::from(SINK_REVISION),
            inputs: sink_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .expect("logic sink fixture kind is unique");
}

fn scalar_output(name: &str) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_INFO_ID),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
    }
}

fn budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement)?;
    Ok(OperationBudget {
        value_items: 4,
        value_bytes: (SCALAR_ENCODED_LEN * 4) as u32,
        host_requests: 0,
        sign_items: 64,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    store: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement)?;
    let values = [
        Scalar::MIN,
        Scalar::MIN,
        Scalar::from_raw_microunits(-1),
        Scalar::MAX,
    ]
    .map(|scalar| {
        store
            .store(&scalar.encode())
            .map_err(|error| format!("store logic fixture scalar: {error:?}"))
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?
    .try_into()
    .map_err(|_| "logic fixture value count changed".to_string())?;
    Ok(InstalledOperation::TestLogicScript(
        TestLogicScriptOperation { values, next: 0 },
    ))
}

fn sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate_sink(placement)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 32,
        maximum_value_bytes: SCALAR_ENCODED_LEN as u32,
    })
}

fn prepare_sink(
    placement: &PlannedGear,
    _store: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate_sink(placement)?;
    Ok(InstalledOperation::TestLogicSink(TestLogicSinkOperation))
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    if placement.kind_id.as_str() != KIND
        || placement.kind_contract_revision.as_str() != REVISION
        || placement.execution_profile_id.as_str() != PROFILE
        || placement.implementation_id.as_str() != IMPLEMENTATION
        || placement.artifact_id.as_str() != ARTIFACT
        || !placement.inputs.is_empty()
        || placement.outputs != offer().outputs
        || !placement.configuration.is_empty()
    {
        return Err("planned logic script does not match its fixture".into());
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
        || !placement.configuration.is_empty()
    {
        return Err("planned logic sink does not match its fixture".into());
    }
    Ok(())
}
