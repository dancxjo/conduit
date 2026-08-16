use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, KindContractRevision, PlannedGear, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_kernel::{OperationAction, OperationInput, PortId, ValueRef, ValueStorage};

const SOURCE_KIND: &str = "conduit-test/local-model-request";
const SOURCE_REVISION: &str = "conduit.test/local-model-request@1";
const SOURCE_IMPLEMENTATION: &str = "conduit.test/local-model-request-kernel@1";
const SINK_KIND: &str = "conduit-test/local-model-result";
const SINK_REVISION: &str = "conduit.test/local-model-result@1";
const SINK_IMPLEMENTATION: &str = "conduit.test/local-model-result-kernel@1";
const PROFILE: &str = "conduit.test/local-model-io-kernel@1";
const ARTIFACT: &str = "conduit-std-host/test-local-model-io@1";

pub(super) static TEST_LOCAL_MODEL_SOURCE_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SOURCE_IMPLEMENTATION,
    budget: source_budget,
    prepare: prepare_source,
};
pub(super) static TEST_LOCAL_MODEL_SINK_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SINK_IMPLEMENTATION,
    budget: sink_budget,
    prepare: prepare_sink,
};

pub(super) struct TestLocalModelSourceOperation {
    value: ValueRef,
    emitted: bool,
}

pub(super) struct TestLocalModelSinkOperation {
    complete: bool,
}

impl TestLocalModelSourceOperation {
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

impl TestLocalModelSinkOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0), ..
            } if !self.complete => {
                self.complete = true;
                OperationAction::Complete
            }
            _ => InstalledOperation::fail(142),
        }
    }
}

pub(crate) fn source_offer(value_kind: &str) -> CapabilityOffer {
    offer(
        SOURCE_KIND,
        SOURCE_REVISION,
        SOURCE_IMPLEMENTATION,
        value_kind,
        PortDirection::Output,
    )
}

pub(crate) fn sink_offer(value_kind: &str) -> CapabilityOffer {
    offer(
        SINK_KIND,
        SINK_REVISION,
        SINK_IMPLEMENTATION,
        value_kind,
        PortDirection::Input,
    )
}

fn offer(
    kind: &str,
    revision: &str,
    implementation: &str,
    value_kind: &str,
    direction: PortDirection,
) -> CapabilityOffer {
    let port = PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    };
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("{kind}/{value_kind}")),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: if direction == PortDirection::Input {
            vec![port.clone()]
        } else {
            Vec::new()
        },
        outputs: if direction == PortDirection::Output {
            vec![port]
        } else {
            Vec::new()
        },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: 16_384,
        },
    }
}

pub(crate) fn install_catalog(
    startup: &mut StartupCatalog,
    catalog: &mut ProfileCatalog,
    request_kind: &str,
    result_kind: &str,
) {
    for offer in [source_offer(request_kind), sink_offer(result_kind)] {
        startup
            .insert(KindSignature {
                kind: offer.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .expect("test local-model IO startup Kind is unique");
        catalog
            .insert(KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: Vec::new(),
            })
            .expect("test local-model IO Kind is unique");
    }
}

fn validate(placement: &PlannedGear, direction: PortDirection) -> Result<(), String> {
    let (kind, revision, implementation) = if direction == PortDirection::Output {
        (SOURCE_KIND, SOURCE_REVISION, SOURCE_IMPLEMENTATION)
    } else {
        (SINK_KIND, SINK_REVISION, SINK_IMPLEMENTATION)
    };
    if placement.kind_id.as_str() != kind
        || placement.kind_contract_revision.as_str() != revision
        || placement.execution_profile_id.as_str() != PROFILE
        || placement.implementation_id.as_str() != implementation
        || placement.artifact_id.as_str() != ARTIFACT
    {
        return Err("planned local-model test endpoint identity mismatch".to_string());
    }
    Ok(())
}

fn source_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, PortDirection::Output)?;
    Ok(OperationBudget {
        value_items: 1,
        value_bytes: 4_096,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: 4_096,
    })
}

fn sink_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, PortDirection::Input)?;
    Ok(OperationBudget {
        value_items: 0,
        value_bytes: 0,
        host_requests: 0,
        sign_items: 16,
        maximum_value_bytes: 4_096,
    })
}

fn prepare_source(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, PortDirection::Output)?;
    let request = if placement.outputs[0].value_kind.as_str() == "llm/interpretation-request@1" {
        serde_json::to_vec(&conduit_ai::InterpretationRequest {
            evidence: vec![
                conduit_ai::InterpretationEvidence {
                    sign_id: conduit_core::SignId::from("sign/line/carrier-lost/7"),
                    observation: "carrier lost".into(),
                },
                conduit_ai::InterpretationEvidence {
                    sign_id: conduit_core::SignId::from("sign/peer/unreachable/8"),
                    observation: "peer unreachable".into(),
                },
                conduit_ai::InterpretationEvidence {
                    sign_id: conduit_core::SignId::from("sign/host/offer-fresh/9"),
                    observation: "fresh Host offer remains available".into(),
                },
            ],
            context: "explain the likely operational boundary without taking action".into(),
        })
        .map_err(|error| format!("encode local-model interpretation request: {error}"))?
    } else {
        b"Conduit bounded local model request".to_vec()
    };
    let value = values
        .store(&request)
        .map_err(|error| format!("store local-model test request: {error:?}"))?;
    Ok(InstalledOperation::TestLocalModelSource(
        TestLocalModelSourceOperation {
            value,
            emitted: false,
        },
    ))
}

fn prepare_sink(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, PortDirection::Input)?;
    Ok(InstalledOperation::TestLocalModelSink(
        TestLocalModelSinkOperation { complete: false },
    ))
}
