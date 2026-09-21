use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{
    kind_id, resource_requirement, ArtifactId, AuthorityContractId, AuthorityRequirement, Back,
    BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId, HostCallContractId,
    HostCallRequirement, ImplementationId, PlannedGear,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef, ValueStorage,
};

pub(super) const CLIENT_IMPLEMENTATION: &str = "std/kernel-http-client-http1";
pub(super) const CLIENT_ARTIFACT: &str = "conduit-std-host/http-client-http1";
pub(super) const CLIENT_PROFILE: &str = "std/http1-plain-bounded";
pub(super) const CLIENT_OPERATION: &str = "conduit.host/http-client-exchange";
pub(super) const CLIENT_RESOURCE: &str = "conduit.resource/network/http-client";
pub(super) const CLIENT_AUTHORITY: &str = "conduit.authority/http-outbound";

pub(super) const SERVER_IMPLEMENTATION: &str = "std/kernel-http-server-http1";
pub(super) const SERVER_ARTIFACT: &str = "conduit-std-host/http-server-http1";
pub(super) const SERVER_PROFILE: &str = "std/http1-listener-plain-bounded";
pub(super) const SERVER_ACCEPT_OPERATION: &str = "conduit.host/http-server-accept";
pub(super) const SERVER_RESPOND_OPERATION: &str = "conduit.host/http-server-respond";
pub(super) const SERVER_RESOURCE: &str = "conduit.resource/network/http-listener";
pub(super) const SERVER_AUTHORITY: &str = "conduit.authority/http-listener";

pub(super) static HTTP_CLIENT_FACTORY: BackFactory = BackFactory {
    implementation_id: CLIENT_IMPLEMENTATION,
    budget: client_budget,
    prepare: prepare_client,
};
pub(super) static HTTP_SERVER_FACTORY: BackFactory = BackFactory {
    implementation_id: SERVER_IMPLEMENTATION,
    budget: server_budget,
    prepare: prepare_server,
};

pub(crate) fn client_offer() -> CapabilityOffer {
    client_offer_for(
        "std-http-client-http1",
        CLIENT_PROFILE,
        CLIENT_IMPLEMENTATION,
        CLIENT_ARTIFACT,
    )
}

pub(crate) fn client_offer_for(
    capability: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
) -> CapabilityOffer {
    let contract = conduit_web::http_client_semantics().into_semantic_contract();
    let request_kind = conduit_web::http_request_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let operation = host_call(
        CLIENT_OPERATION,
        request_kind.as_str(),
        conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
        conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
    );
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(capability),
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
            host_calls: vec![operation.clone()],
            resource_requirements: vec![resource_requirement(CLIENT_RESOURCE, 1)],
            authority_requirements: vec![authority(
                CLIENT_AUTHORITY,
                &operation,
                request_kind.as_str(),
            )],
        },
    )
    .build()
}

pub(crate) fn server_offer() -> CapabilityOffer {
    let contract = conduit_web::http_server_semantics().into_semantic_contract();
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
    let accept = host_call(
        SERVER_ACCEPT_OPERATION,
        request_kind.as_str(),
        0,
        conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
    );
    let respond = host_call(
        SERVER_RESPOND_OPERATION,
        response_kind.as_str(),
        conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
        0,
    );
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("std-http-server-http1"),
            execution_profile_id: ExecutionProfileId::from(SERVER_PROFILE),
            implementation_id: ImplementationId::from(SERVER_IMPLEMENTATION),
            artifact_id: ArtifactId::from(SERVER_ARTIFACT),
            host_calls: vec![accept.clone(), respond.clone()],
            resource_requirements: vec![resource_requirement(SERVER_RESOURCE, 1)],
            authority_requirements: vec![
                authority(SERVER_AUTHORITY, &accept, request_kind.as_str()),
                authority(SERVER_AUTHORITY, &respond, response_kind.as_str()),
            ],
        },
    )
    .build()
}

fn host_call(contract: &str, subject: &str, input: u32, output: u32) -> HostCallRequirement {
    HostCallRequirement {
        contract_id: HostCallContractId::from(contract),
        target_kind: Some(kind_id(subject)),
        maximum_in_flight: 1,
        maximum_input_bytes: input,
        maximum_output_bytes: output,
    }
}

fn authority(
    contract: &str,
    operation: &HostCallRequirement,
    subject: &str,
) -> AuthorityRequirement {
    AuthorityRequirement {
        contract_id: AuthorityContractId::from(contract),
        host_call_contract_id: operation.contract_id.clone(),
        subject_kind: kind_id(subject),
    }
}

pub(super) struct HttpClientBack {
    pending: bool,
    completed: u16,
}

impl<const PORTS: usize> StepBack<PORTS> for HttpClientBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.completed == conduit_web::HTTP_MAXIMUM_IN_FLIGHT {
            return StepOutcome::Complete;
        }
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(u32::from(self.completed)) {
                return http_step_fail(FailureCode::InvalidLifecycle, 4);
            }
            match (outcome.disposition, outcome.output, outcome.failure) {
                (HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion()
                        .expect("observed HTTP response");
                    io.send(PortId(0), output.value).expect("ready HTTP output");
                    self.pending = false;
                    self.completed += 1;
                    return StepOutcome::Progress;
                }
                (HostCallDisposition::Denied, _, _) => {
                    return http_step_fail(FailureCode::HostCallDenied, 1)
                }
                (HostCallDisposition::Cancelled, _, _) => {
                    return http_step_fail(FailureCode::Cancelled, 2)
                }
                (HostCallDisposition::Failed, _, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return http_step_fail(FailureCode::InvalidLifecycle, 3),
            }
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending {
                return http_step_fail(FailureCode::InvalidLifecycle, 4);
            }
            let input =
                BoundedValueRef::new(value, conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES)
                    .expect("planned HTTP request is bounded");
            let request = RequestId(u32::from(self.completed));
            io.consume(PortId(0)).expect("present HTTP request");
            io.request_host_call(request, HostCallId(0), input)
                .expect("HTTP client Host Call");
            self.pending = true;
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
    }
}

impl HttpClientBack {}

#[derive(Clone, Copy)]
enum ServerPending {
    Accept,
    Respond,
}

pub(super) struct HttpServerBack {
    empty: ValueRef,
    released: Option<ValueRef>,
    pending: Option<ServerPending>,
    accepted: u16,
}

impl<const PORTS: usize> StepBack<PORTS> for HttpServerBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((_, outcome)) = io.host_completion() {
            let Some(pending) = self.pending else {
                return http_step_fail(FailureCode::InvalidLifecycle, 10);
            };
            match (
                pending,
                outcome.disposition,
                outcome.output,
                outcome.failure,
            ) {
                (ServerPending::Accept, HostCallDisposition::Completed, Some(output), None) => {
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion().expect("observed HTTP accept");
                    io.send(PortId(0), output.value)
                        .expect("ready accepted HTTP request");
                    self.pending = None;
                    self.accepted += 1;
                    return StepOutcome::Progress;
                }
                (ServerPending::Respond, HostCallDisposition::Completed, None, None) => {
                    io.consume_host_completion()
                        .expect("observed HTTP response send");
                    self.pending = None;
                    if self.accepted == conduit_web::HTTP_MAXIMUM_IN_FLIGHT {
                        io.discard(self.empty).expect("release empty HTTP command");
                        self.released = None;
                        return StepOutcome::Complete;
                    }
                    self.request_accept_step(io);
                    return StepOutcome::Progress;
                }
                (_, HostCallDisposition::Denied, _, _) => {
                    return http_step_fail(FailureCode::HostCallDenied, 11)
                }
                (_, HostCallDisposition::Cancelled, _, _) => {
                    return http_step_fail(FailureCode::Cancelled, 12)
                }
                (_, HostCallDisposition::Failed, _, Some(failure)) => {
                    return StepOutcome::Fail(failure)
                }
                _ => return http_step_fail(FailureCode::InvalidLifecycle, 13),
            }
        }
        if self.pending.is_none() && self.accepted == 0 {
            self.request_accept_step(io);
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            if self.pending.is_some() || self.accepted == 0 {
                return http_step_fail(FailureCode::InvalidLifecycle, 14);
            }
            let input =
                BoundedValueRef::new(value, conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES)
                    .expect("planned HTTP response is bounded");
            let request = RequestId(u32::from(self.accepted) * 2 - 1);
            io.consume(PortId(0)).expect("present HTTP response");
            io.request_host_call(request, HostCallId(1), input)
                .expect("HTTP response Host Call");
            self.pending = Some(ServerPending::Respond);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) && self.pending.is_none() {
            io.consume_closed(PortId(0))
                .expect("observed HTTP response closure");
            io.discard(self.empty).expect("release empty HTTP command");
            self.released = None;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = None;
        self.released = Some(self.empty);
    }
}

impl HttpServerBack {
    fn request_accept_step<const PORTS: usize>(&mut self, io: &mut StepIo<PORTS>) {
        let request = RequestId(u32::from(self.accepted) * 2);
        io.request_host_call(
            request,
            HostCallId(0),
            BoundedValueRef::new(self.empty, 0).expect("empty HTTP accept command is bounded"),
        )
        .expect("HTTP accept Host Call");
        self.pending = Some(ServerPending::Accept);
    }
}

const fn http_step_fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}

impl HttpServerBack {}

fn client_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement, &client_offer())?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES
            + conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
        host_requests: usize::from(conduit_web::HTTP_MAXIMUM_IN_FLIGHT),
        sign_items: 64,
        maximum_value_bytes: conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
    })
}

fn server_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement, &server_offer())?;
    Ok(BackBudget {
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
) -> Result<InstalledBack, String> {
    validate(placement, &client_offer())?;
    Ok(InstalledBack::HttpClient(HttpClientBack {
        pending: false,
        completed: 0,
    }))
}

fn prepare_server(
    placement: &PlannedGear,
    values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement, &server_offer())?;
    let empty = values
        .store(&[])
        .map_err(|error| format!("store HTTP accept command: {error:?}"))?;
    Ok(InstalledBack::HttpServer(HttpServerBack {
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
        || placement.host_calls != offer.host_calls
    {
        return Err("planned HTTP identity differs from the installed realization".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_http_realizations_preserve_owner_issued_fronts() {
        for (offer, contract) in [
            (
                client_offer(),
                conduit_web::http_client_semantics().into_semantic_contract(),
            ),
            (
                server_offer(),
                conduit_web::http_server_semantics().into_semantic_contract(),
            ),
        ] {
            assert_eq!(offer.startup_parameters, contract.startup_parameters);
            assert_eq!(offer.shorthand, contract.shorthand);
            assert_eq!(offer.kind_id, contract.kind_id);
            assert_eq!(
                offer.kind_contract_revision,
                contract.kind_contract_revision
            );
            assert_eq!(offer.inputs, contract.inputs);
            assert_eq!(offer.outputs, contract.outputs);
            assert_eq!(offer.limits, contract.limits);
            assert!(!offer.host_calls.is_empty());
            assert_eq!(offer.resource_requirements.len(), 1);
            assert!(!offer.authority_requirements.is_empty());
        }

        let isolated = client_offer_for(
            "fixture/isolated-http",
            "fixture/isolated-profile",
            "fixture/isolated-implementation",
            "fixture/isolated-artifact",
        );
        let client = conduit_web::http_client_semantics().into_semantic_contract();
        assert_eq!(isolated.kind_id, client.kind_id);
        assert_eq!(
            isolated.kind_contract_revision,
            client.kind_contract_revision
        );
        assert_eq!(isolated.inputs, client.inputs);
        assert_eq!(isolated.outputs, client.outputs);
        assert_eq!(isolated.limits, client.limits);
    }
}
