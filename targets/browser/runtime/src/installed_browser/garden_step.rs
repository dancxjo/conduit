//! Browser production realization of the deterministic minimal Garden reducer.

use super::factory::{validate_placement, BrowserInstallation};
use super::{BrowserOperation, MAXIMUM_BROWSER_VALUE_BYTES};
use conduit_core::{
    ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ExecutionProfileId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, KindContractRevision,
    PlannedGear,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RequestId,
};

pub(crate) const OPERATIONS: [&str; 2] = [
    "conduit.host/browser-garden-step-0-prior@1",
    "conduit.host/browser-garden-step-1-clock@1",
];
pub(crate) const CONTACT_OPERATIONS: [&str; 3] = [
    "conduit.host/browser-garden-step-contact-0-prior@1",
    "conduit.host/browser-garden-step-contact-1-clock@1",
    "conduit.host/browser-garden-step-contact-2-contact@1",
];
const IMPLEMENTATION: &str = "browser/kernel-garden-step@1";
const CONTACT_IMPLEMENTATION: &str = "browser/kernel-garden-step-contact@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer,
    prepare,
    perform: None,
};
pub(super) static CONTACT_INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: CONTACT_IMPLEMENTATION,
    offer: contact_offer,
    prepare: prepare_contact,
    perform: None,
};

pub(crate) struct PreparedGardenStep {
    prior: Option<conduit_semantic_catalog::GardenState>,
    clock: Option<conduit_semantic_catalog::GardenClockObservation>,
    enriched: bool,
}

impl PreparedGardenStep {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        let enriched = match placement.implementation_id.as_str() {
            IMPLEMENTATION => false,
            CONTACT_IMPLEMENTATION => true,
            _ => return Ok(None),
        };
        let expected = if enriched { contact_offer() } else { offer() };
        validate_placement(placement, &expected)?;
        Ok(Some(Self {
            prior: None,
            clock: None,
            enriched,
        }))
    }

    pub(crate) fn execute(
        &mut self,
        contract: &str,
        input: &[u8],
    ) -> Result<Option<Vec<u8>>, Failure> {
        match contract {
            value if value == OPERATIONS[0] => {
                if self.prior.is_some() {
                    return Err(failure(1));
                }
                self.prior = Some(
                    conduit_semantic_catalog::decode_garden_state(input).map_err(|_| failure(2))?,
                );
                Ok(None)
            }
            value if value == OPERATIONS[1] => {
                let prior = self.prior.take().ok_or(failure(3))?;
                let clock = conduit_semantic_catalog::decode_garden_clock_observation(input)
                    .map_err(|_| failure(4))?;
                let next = conduit_semantic_catalog::evolve_garden_minimal(prior, clock)
                    .map_err(|error| failure(evolution_detail(error)))?;
                let bytes = conduit_semantic_catalog::garden_state_value(next)
                    .and_then(|value| value.canonical_bytes())
                    .map_err(|_| failure(10))?;
                if bytes.len() > MAXIMUM_BROWSER_VALUE_BYTES {
                    return Err(Failure {
                        code: FailureCode::StorageExhausted,
                        detail: 11,
                    });
                }
                Ok(Some(bytes))
            }
            value if value == CONTACT_OPERATIONS[0] && self.enriched => {
                if self.prior.is_some() {
                    return Err(failure(1));
                }
                self.prior = Some(
                    conduit_semantic_catalog::decode_garden_state(input).map_err(|_| failure(2))?,
                );
                Ok(None)
            }
            value if value == CONTACT_OPERATIONS[1] && self.enriched => {
                if self.clock.is_some() {
                    return Err(failure(13));
                }
                self.clock = Some(
                    conduit_semantic_catalog::decode_garden_clock_observation(input)
                        .map_err(|_| failure(4))?,
                );
                Ok(None)
            }
            value if value == CONTACT_OPERATIONS[2] && self.enriched => {
                let prior = self.prior.take().ok_or(failure(3))?;
                let clock = self.clock.take().ok_or(failure(14))?;
                let contact = conduit_semantic_catalog::decode_garden_contact_observation(input)
                    .map_err(|_| failure(6))?;
                let next = conduit_semantic_catalog::evolve_garden_enriched(prior, clock, contact)
                    .map_err(|error| failure(evolution_detail(error)))?;
                canonical_state(next).map(Some)
            }
            _ => Err(failure(12)),
        }
    }
}

fn canonical_state(state: conduit_semantic_catalog::GardenState) -> Result<Vec<u8>, Failure> {
    let bytes = conduit_semantic_catalog::garden_state_value(state)
        .and_then(|value| value.canonical_bytes())
        .map_err(|_| failure(10))?;
    if bytes.len() > MAXIMUM_BROWSER_VALUE_BYTES {
        return Err(Failure {
            code: FailureCode::StorageExhausted,
            detail: 11,
        });
    }
    Ok(bytes)
}

fn offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::garden_minimal_step_definition();
    let kind = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(IMPLEMENTATION),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::GARDEN_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(IMPLEMENTATION),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/garden-step@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: OPERATIONS
            .iter()
            .enumerate()
            .map(|(index, contract_id)| HostOperationRequirement {
                contract_id: (*contract_id).into(),
                target_kind: Some(kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: if index == 0 {
                    0
                } else {
                    MAXIMUM_BROWSER_VALUE_BYTES as u32
                },
            })
            .collect(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn contact_offer() -> CapabilityOffer {
    let contract = conduit_semantic_catalog::garden_enriched_step_definition();
    let kind = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(CONTACT_IMPLEMENTATION),
        kind_id: kind.clone(),
        kind_contract_revision: KindContractRevision::from(
            conduit_semantic_catalog::GARDEN_CONTRACT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(CONTACT_IMPLEMENTATION),
            implementation_id: ImplementationId::from(CONTACT_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-browser-runtime/garden-step-contact@1"),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: CONTACT_OPERATIONS
            .iter()
            .enumerate()
            .map(|(index, contract_id)| HostOperationRequirement {
                contract_id: (*contract_id).into(),
                target_kind: Some(kind.clone()),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
                maximum_output_bytes: if index == 2 {
                    MAXIMUM_BROWSER_VALUE_BYTES as u32
                } else {
                    0
                },
            })
            .collect(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: MAXIMUM_BROWSER_VALUE_BYTES as u32,
        },
    }
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserOperation, String> {
    PreparedGardenStep::for_placement(placement)?
        .ok_or_else(|| "Garden step selected another implementation".to_string())?;
    Ok(BrowserOperation::installed(GardenStepOperation::new()))
}

fn prepare_contact(
    placement: &PlannedGear,
    _: &mut HostedValueStore,
) -> Result<BrowserOperation, String> {
    PreparedGardenStep::for_placement(placement)?
        .filter(|prepared| prepared.enriched)
        .ok_or_else(|| "contact Garden step selected another implementation".to_string())?;
    Ok(BrowserOperation::installed(
        ContactGardenStepOperation::new(),
    ))
}

struct GardenStepOperation {
    prior_ready: bool,
    pending: Option<(RequestId, HostOperationId)>,
    emitted: bool,
}

impl GardenStepOperation {
    const fn new() -> Self {
        Self {
            prior_ready: false,
            pending: None,
            emitted: false,
        }
    }
}

struct ContactGardenStepOperation {
    next_port: u16,
    pending: Option<(RequestId, HostOperationId)>,
    emitted: bool,
}

impl ContactGardenStepOperation {
    const fn new() -> Self {
        Self {
            next_port: 0,
            pending: None,
            emitted: false,
        }
    }
}

impl Operation for ContactGardenStepOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value { port, value }
                if port.0 == self.next_port && self.pending.is_none() && !self.emitted =>
            {
                let Ok(input) = BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32)
                else {
                    return OperationAction::Fail(failure(30));
                };
                let request = RequestId(u32::from(self.next_port));
                let operation = HostOperationId(self.next_port);
                self.pending = Some((request, operation));
                OperationAction::RequestHostOperation {
                    request,
                    operation,
                    input,
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending.map(|pending| pending.0) == Some(request) =>
            {
                let Some((_, operation)) = self.pending.take() else {
                    return OperationAction::Fail(failure(31));
                };
                match (
                    operation,
                    outcome.disposition,
                    outcome.output,
                    outcome.failure,
                ) {
                    (HostOperationId(0 | 1), HostOperationDisposition::Completed, None, None) => {
                        self.next_port += 1;
                        OperationAction::Await
                    }
                    (
                        HostOperationId(2),
                        HostOperationDisposition::Completed,
                        Some(output),
                        None,
                    ) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (_, HostOperationDisposition::Failed, None, Some(reason)) => {
                        OperationAction::Fail(reason)
                    }
                    _ => OperationAction::Fail(failure(31)),
                }
            }
            OperationInput::Closed { port } if port.0 < self.next_port => OperationAction::Await,
            OperationInput::Closed { port: PortId(2) } if self.emitted => OperationAction::Complete,
            _ => OperationAction::Fail(failure(32)),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl Operation for GardenStepOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.prior_ready && self.pending.is_none() => {
                self.request(value, RequestId(0), HostOperationId(0), 20)
            }
            OperationInput::Value {
                port: PortId(1),
                value,
            } if self.prior_ready && self.pending.is_none() && !self.emitted => {
                self.request(value, RequestId(1), HostOperationId(1), 21)
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending.map(|pending| pending.0) == Some(request) =>
            {
                let Some((_, operation)) = self.pending.take() else {
                    return OperationAction::Fail(failure(22));
                };
                match (
                    operation,
                    outcome.disposition,
                    outcome.output,
                    outcome.failure,
                ) {
                    (HostOperationId(0), HostOperationDisposition::Completed, None, None) => {
                        self.prior_ready = true;
                        OperationAction::Await
                    }
                    (
                        HostOperationId(1),
                        HostOperationDisposition::Completed,
                        Some(output),
                        None,
                    ) => {
                        self.emitted = true;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (_, HostOperationDisposition::Failed, None, Some(reason)) => {
                        OperationAction::Fail(reason)
                    }
                    _ => OperationAction::Fail(failure(22)),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.prior_ready => {
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(1) } if self.emitted => OperationAction::Complete,
            _ => OperationAction::Fail(failure(23)),
        }
    }

    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    fn cancel(&mut self) {
        self.pending = None;
    }
}

impl GardenStepOperation {
    fn request(
        &mut self,
        value: conduit_kernel::ValueRef,
        request: RequestId,
        operation: HostOperationId,
        detail: u16,
    ) -> OperationAction {
        let Ok(input) = BoundedValueRef::new(value, MAXIMUM_BROWSER_VALUE_BYTES as u32) else {
            return OperationAction::Fail(failure(detail));
        };
        self.pending = Some((request, operation));
        OperationAction::RequestHostOperation {
            request,
            operation,
            input,
        }
    }
}

fn evolution_detail(error: conduit_semantic_catalog::GardenEvolutionRefusal) -> u16 {
    use conduit_semantic_catalog::GardenEvolutionRefusal::*;
    match error {
        MalformedState => 5,
        MalformedClockObservation => 6,
        MalformedContactObservation => 7,
        StepCapacityExceeded => 8,
        ArithmeticOverflow => 9,
    }
}

fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

#[cfg(test)]
#[path = "garden_step_tests.rs"]
mod tests;
