use super::operation::{InstalledFactory, InstalledOperation, OperationBudget};
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement,
    CapabilityId, CapabilityOffer, ExecutionProfileId, HostOperationContractId,
    HostOperationRequirement, ImplementationId, ImplementationOffer, PlannedGear,
};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationId,
    OperationAction, OperationInput, PortId, RequestId, ValueRef, ValueStorage,
};

pub(super) const CLIENT_IMPLEMENTATION: &str = "std/kernel-http-client-http1@1";
pub(super) const CLIENT_ARTIFACT: &str = "conduit-std-host/http-client-http1@1";
pub(super) const CLIENT_PROFILE: &str = "std/http1-plain-bounded@1";
pub(super) const CLIENT_OPERATION: &str = "conduit.host/http-client-exchange@1";
pub(super) const CLIENT_RESOURCE: &str = "conduit.resource/network/http-client@1";
pub(super) const CLIENT_AUTHORITY: &str = "conduit.authority/http-outbound@1";

pub(super) const SERVER_IMPLEMENTATION: &str = "std/kernel-http-server-http1@1";
pub(super) const SERVER_ARTIFACT: &str = "conduit-std-host/http-server-http1@1";
pub(super) const SERVER_PROFILE: &str = "std/http1-listener-plain-bounded@1";
pub(super) const SERVER_ACCEPT_OPERATION: &str = "conduit.host/http-server-accept@1";
pub(super) const SERVER_RESPOND_OPERATION: &str = "conduit.host/http-server-respond@1";
pub(super) const SERVER_RESOURCE: &str = "conduit.resource/network/http-listener@1";
pub(super) const SERVER_AUTHORITY: &str = "conduit.authority/http-listener@1";

pub(super) static HTTP_CLIENT_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: CLIENT_IMPLEMENTATION,
    budget: client_budget,
    prepare: prepare_client,
};
pub(super) static HTTP_SERVER_FACTORY: InstalledFactory = InstalledFactory {
    implementation_id: SERVER_IMPLEMENTATION,
    budget: server_budget,
    prepare: prepare_server,
};

pub(crate) fn client_offer() -> CapabilityOffer {
    let contract = conduit_web::http_client_semantics();
    let request_kind = conduit_web::http_request_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let operation = host_operation(
        CLIENT_OPERATION,
        request_kind.as_str(),
        conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
        conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
    );
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("std-http-client-http1"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(CLIENT_PROFILE),
            implementation_id: ImplementationId::from(CLIENT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(CLIENT_ARTIFACT),
        },
        host_operations: vec![operation.clone()],
        resource_requirements: vec![resource_requirement(CLIENT_RESOURCE, 1)],
        authority_requirements: vec![authority(
            CLIENT_AUTHORITY,
            &operation,
            request_kind.as_str(),
        )],
        limits: contract.limits,
    }
}

pub(crate) fn server_offer() -> CapabilityOffer {
    let contract = conduit_web::http_server_semantics();
    let request_kind = conduit_web::http_request_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let response_kind = conduit_web::http_response_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let accept = host_operation(
        SERVER_ACCEPT_OPERATION,
        request_kind.as_str(),
        0,
        conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
    );
    let respond = host_operation(
        SERVER_RESPOND_OPERATION,
        response_kind.as_str(),
        conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
        0,
    );
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("std-http-server-http1"),
        kind_id: contract.kind_id,
        kind_contract_revision: contract.kind_contract_revision,
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(SERVER_PROFILE),
            implementation_id: ImplementationId::from(SERVER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(SERVER_ARTIFACT),
        },
        host_operations: vec![accept.clone(), respond.clone()],
        resource_requirements: vec![resource_requirement(SERVER_RESOURCE, 1)],
        authority_requirements: vec![
            authority(SERVER_AUTHORITY, &accept, request_kind.as_str()),
            authority(SERVER_AUTHORITY, &respond, response_kind.as_str()),
        ],
        limits: contract.limits,
    }
}

fn host_operation(
    contract: &str,
    subject: &str,
    input: u32,
    output: u32,
) -> HostOperationRequirement {
    HostOperationRequirement {
        contract_id: HostOperationContractId::from(contract),
        target_kind: Some(kind_id(subject)),
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    }
}

fn authority(
    contract: &str,
    operation: &HostOperationRequirement,
    subject: &str,
) -> AuthorityRequirement {
    AuthorityRequirement {
        contract_id: AuthorityContractId::from(contract),
        host_operation_contract_id: operation.contract_id.clone(),
        subject_kind: kind_id(subject),
    }
}

pub(super) struct HttpClientOperation {
    pending: bool,
    completed: u16,
}

impl HttpClientOperation {
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
                    request: RequestId(u32::from(self.completed)),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(
                        value,
                        conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
                    )
                    .expect("planned HTTP request is bounded"),
                }
            }
            OperationInput::HostOperationCompleted { request, outcome }
                if self.pending && request == RequestId(u32::from(self.completed)) =>
            {
                self.pending = false;
                match (outcome.disposition, outcome.output, outcome.failure) {
                    (HostOperationDisposition::Completed, Some(output), None) => {
                        self.completed += 1;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 1)
                    }
                    (HostOperationDisposition::Cancelled, _, _) => fail(FailureCode::Cancelled, 2),
                    (HostOperationDisposition::Failed, _, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 3),
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 4),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        if self.completed == conduit_web::HTTP_MAXIMUM_IN_FLIGHT {
            OperationAction::Complete
        } else {
            OperationAction::Await
        }
    }

    pub(super) fn cancel(&mut self) {
        self.pending = false;
    }
}

#[derive(Clone, Copy)]
enum ServerPending {
    Accept,
    Respond,
}

pub(super) struct HttpServerOperation {
    empty: ValueRef,
    released: Option<ValueRef>,
    pending: Option<ServerPending>,
    accepted: u16,
}

impl HttpServerOperation {
    pub(super) fn start(&mut self) -> OperationAction {
        self.request_accept()
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if self.pending.is_none() && self.accepted > 0 => {
                self.pending = Some(ServerPending::Respond);
                OperationAction::RequestHostOperation {
                    request: RequestId(u32::from(self.accepted) * 2 - 1),
                    operation: HostOperationId(1),
                    input: BoundedValueRef::new(
                        value,
                        conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
                    )
                    .expect("planned HTTP response is bounded"),
                }
            }
            OperationInput::HostOperationCompleted { outcome, .. } => {
                let Some(pending) = self.pending.take() else {
                    return fail(FailureCode::InvalidLifecycle, 10);
                };
                match (
                    pending,
                    outcome.disposition,
                    outcome.output,
                    outcome.failure,
                ) {
                    (
                        ServerPending::Accept,
                        HostOperationDisposition::Completed,
                        Some(output),
                        None,
                    ) => {
                        self.accepted += 1;
                        OperationAction::Emit {
                            port: PortId(0),
                            value: output.value,
                        }
                    }
                    (ServerPending::Respond, HostOperationDisposition::Completed, None, None) => {
                        if self.accepted == conduit_web::HTTP_MAXIMUM_IN_FLIGHT {
                            self.released = Some(self.empty);
                            OperationAction::Complete
                        } else {
                            self.request_accept()
                        }
                    }
                    (_, HostOperationDisposition::Denied, _, _) => {
                        fail(FailureCode::HostOperationDenied, 11)
                    }
                    (_, HostOperationDisposition::Cancelled, _, _) => {
                        fail(FailureCode::Cancelled, 12)
                    }
                    (_, HostOperationDisposition::Failed, _, Some(failure)) => {
                        OperationAction::Fail(failure)
                    }
                    _ => fail(FailureCode::InvalidLifecycle, 13),
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.pending.is_none() => {
                OperationAction::Complete
            }
            _ => fail(FailureCode::InvalidLifecycle, 14),
        }
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.released = Some(self.empty);
    }

    pub(super) fn take_released_value(&mut self) -> Option<ValueRef> {
        self.released.take()
    }

    fn request_accept(&mut self) -> OperationAction {
        self.pending = Some(ServerPending::Accept);
        OperationAction::RequestHostOperation {
            request: RequestId(u32::from(self.accepted) * 2),
            operation: HostOperationId(0),
            input: BoundedValueRef::new(self.empty, 0)
                .expect("empty HTTP accept command is bounded"),
        }
    }
}

fn client_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &client_offer())?;
    Ok(OperationBudget {
        value_items: 2,
        value_bytes: conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES
            + conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
        host_requests: usize::from(conduit_web::HTTP_MAXIMUM_IN_FLIGHT),
        sign_items: 64,
        maximum_value_bytes: conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
    })
}

fn server_budget(placement: &PlannedGear) -> Result<OperationBudget, String> {
    validate(placement, &server_offer())?;
    Ok(OperationBudget {
        value_items: conduit_web::HTTP_MAXIMUM_IN_FLIGHT * 2 + 1,
        value_bytes: u32::from(conduit_web::HTTP_MAXIMUM_IN_FLIGHT)
            * (conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES
                + conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES),
        host_requests: usize::from(conduit_web::HTTP_MAXIMUM_IN_FLIGHT) * 2,
        sign_items: 128,
        maximum_value_bytes: conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
    })
}

fn prepare_client(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, &client_offer())?;
    Ok(InstalledOperation::HttpClient(HttpClientOperation {
        pending: false,
        completed: 0,
    }))
}

fn prepare_server(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledOperation, String> {
    validate(placement, &server_offer())?;
    let empty = values
        .store(&[])
        .map_err(|error| format!("store HTTP accept command: {error:?}"))?;
    Ok(InstalledOperation::HttpServer(HttpServerOperation {
        empty,
        released: None,
        pending: None,
        accepted: 0,
    }))
}

fn validate(placement: &PlannedGear, offer: &CapabilityOffer) -> Result<(), String> {
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_operations != offer.host_operations
    {
        return Err("planned HTTP identity differs from the installed realization".into());
    }
    Ok(())
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
