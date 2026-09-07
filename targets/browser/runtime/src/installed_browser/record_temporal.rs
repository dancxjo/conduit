//! Pure-kernel browser realizations of explicit framed-record temporal adapters.

use super::factory::{validate_placement, BrowserInstallation};
use super::BrowserOperation;
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    ImplementationId, ImplementationOffer, KindContractRevision, PlannedGear,
};
use conduit_kernel::{
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, ValueRef,
};

const SINGLETON_IMPLEMENTATION: &str = "browser/record-singleton-stream@1";
const EXACTLY_ONE_IMPLEMENTATION: &str = "browser/record-exactly-one@1";
const MAXIMUM: u32 = super::MAXIMUM_BROWSER_VALUE_BYTES as u32;

pub(super) static SINGLETON: BrowserInstallation = BrowserInstallation {
    implementation_id: SINGLETON_IMPLEMENTATION,
    offer: singleton_offer,
    prepare: prepare_singleton,
    perform: None,
};
pub(super) static EXACTLY_ONE: BrowserInstallation = BrowserInstallation {
    implementation_id: EXACTLY_ONE_IMPLEMENTATION,
    offer: exactly_one_offer,
    prepare: prepare_exactly_one,
    perform: None,
};

fn singleton_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_singleton_stream_definition(),
        SINGLETON_IMPLEMENTATION,
    )
}

fn exactly_one_offer() -> CapabilityOffer {
    offer(
        conduit_net::record_exactly_one_definition(),
        EXACTLY_ONE_IMPLEMENTATION,
    )
}

fn offer(definition: conduit_form::KindDefinition, implementation: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: Some((
            definition.inputs[0].port_id.clone(),
            definition.outputs[0].port_id.clone(),
        )),
        capability_id: CapabilityId::from(implementation),
        kind_id: definition.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_net::RECORD_TEMPORAL_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("browser/record-temporal@1"),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("conduit-net/record-temporal@1"),
        },
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 4,
            max_queue_bytes: MAXIMUM * 4,
        },
    }
}

fn prepare_singleton(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &singleton_offer())?;
    Ok(BrowserOperation::installed(SingletonStreamOperation {
        maximum_bytes: MAXIMUM,
        emitted: false,
    }))
}

fn prepare_exactly_one(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    validate_placement(placement, &exactly_one_offer())?;
    Ok(BrowserOperation::installed(ExactlyOneOperation {
        maximum_bytes: MAXIMUM,
        held: None,
        released: None,
        emitted: false,
        retain_resumed: false,
    }))
}

struct SingletonStreamOperation {
    maximum_bytes: u32,
    emitted: bool,
}

impl Operation for SingletonStreamOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.emitted && value.byte_len <= self.maximum_bytes => {
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            _ => fail(40),
        }
    }

    fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }
}

struct ExactlyOneOperation {
    maximum_bytes: u32,
    held: Option<ValueRef>,
    released: Option<ValueRef>,
    emitted: bool,
    retain_resumed: bool,
}

impl Operation for ExactlyOneOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        self.retain_resumed = false;
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.held.is_none() && value.byte_len <= self.maximum_bytes => {
                self.held = Some(value);
                self.retain_resumed = true;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if !self.emitted => {
                let Some(value) = self.held.take() else {
                    return fail(41);
                };
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value,
                }
            }
            _ => fail(41),
        }
    }

    fn retains_resumed_value(&self) -> bool {
        self.retain_resumed
    }

    fn advance(&mut self) -> OperationAction {
        OperationAction::Complete
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.take()
    }

    fn cancel(&mut self) {
        self.released = self.held.take();
        self.retain_resumed = false;
    }
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::OperationFailed,
        detail,
    })
}
