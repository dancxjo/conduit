use super::*;
extern crate std;
use alloc::{format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, AuthorityGrant, AuthorityGrantId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConnectionBase, ExecutionProfileId, HostAdvertisement, HostId,
    HostOperationContractId, HostOperationRequirement, HostProfileId, ImplementationId,
    ImplementationOffer, KindContractRevision, OfferGeneration, PROTOCOL_VERSION, PortDescriptor,
    PortDirection, PortTemporal, kind_id, port_id, resource_offer,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, KernelEvent, Operation,
    OperationAction, OperationInput, PortId, RequestId, SignSink, ValueRef, ValueStorage,
    scheduler::{FixedScheduler, OperationDriver, SchedulerStatus},
};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};
use conduit_runtime::lowering::{MAXIMUM_KERNEL_PORTS_PER_NODE, lower_plan_fragment};

const SOURCE_KIND: &str = "test/http-request-source";
const SOURCE_REVISION: &str = "test/http-request-source@1";
const SOURCE_IMPLEMENTATION: &str = "test/kernel-http-request-source@1";
const SINK_KIND: &str = "test/http-response-sink";
const SINK_REVISION: &str = "test/http-response-sink@1";
const SINK_IMPLEMENTATION: &str = "test/kernel-http-response-sink@1";
const OBSERVE_OPERATION: &str = "test/observe-http-response@1";
const FIXTURE_PROFILE: &str = "test/http-kernel-fixture@1";
const MAX_NODES: usize = 3;
const MAX_CORDS: usize = 2;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const VALUE_SLOTS: usize = 8;
const VALUE_BYTES: usize = REQUEST_BYTES + RESPONSE_BYTES;
const SIGN_CAPACITY: usize = 96;

#[derive(Clone, Copy)]
struct Source {
    value: ValueRef,
    emitted: bool,
}

impl Operation for Source {
    fn start(&mut self) -> OperationAction {
        OperationAction::Emit {
            port: PortId(0),
            value: self.value,
        }
    }
    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        invalid(1)
    }
    fn advance(&mut self) -> OperationAction {
        if self.emitted {
            OperationAction::Complete
        } else {
            self.emitted = true;
            OperationAction::Complete
        }
    }
    fn cancel(&mut self) {}
}

#[derive(Clone, Copy)]
struct Client {
    pending: bool,
    emitted: bool,
}

impl Operation for Client {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending && !self.emitted => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, REQUEST_BYTES as u32).unwrap(),
                }
            }
            OperationInput::HostOperationCompleted {
                request: RequestId(0),
                outcome,
            } if self.pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none()
                && outcome.output.is_some() =>
            {
                self.pending = false;
                self.emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: outcome.output.unwrap().value,
                }
            }
            OperationInput::Closed { port: PortId(0) } if self.emitted && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(2),
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}

#[derive(Clone, Copy)]
struct Sink {
    pending: bool,
    observed: bool,
}

impl Operation for Sink {
    fn start(&mut self) -> OperationAction {
        OperationAction::Await
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0),
                value,
            } if !self.pending => {
                self.pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, RESPONSE_BYTES as u32).unwrap(),
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
                self.observed = true;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.observed && !self.pending => {
                OperationAction::Complete
            }
            _ => invalid(3),
        }
    }
    fn cancel(&mut self) {
        self.pending = false;
    }
}

enum PlannedOperation {
    Source(Source),
    Client(Client),
    Sink(Sink),
}

impl Operation for PlannedOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source(v) => v.start(),
            Self::Client(v) => v.start(),
            Self::Sink(v) => v.start(),
        }
    }
    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match self {
            Self::Source(v) => v.resume(input),
            Self::Client(v) => v.resume(input),
            Self::Sink(v) => v.resume(input),
        }
    }
    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source(v) => v.advance(),
            Self::Client(v) => v.advance(),
            Self::Sink(v) => v.advance(),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(v) => v.cancel(),
            Self::Client(v) => v.cancel(),
            Self::Sink(v) => v.cancel(),
        }
    }
}

const fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

fn fixture_offer(
    kind: &str,
    revision: &str,
    implementation: &str,
    direction: PortDirection,
) -> CapabilityOffer {
    let descriptor = PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id(if direction == PortDirection::Output {
            conduit_std_catalog::HTTP_REQUEST_INFO_ID
        } else {
            conduit_std_catalog::HTTP_RESPONSE_INFO_ID
        }),
        direction,
        temporal: PortTemporal::Flow { closes: true },
    };
    let observe = (direction == PortDirection::Input).then(|| HostOperationRequirement {
        contract_id: HostOperationContractId::from(OBSERVE_OPERATION),
        target_kind: Some(kind_id(conduit_std_catalog::HTTP_RESPONSE_INFO_ID)),
        maximum_in_flight: 1,
        maximum_input_bytes: RESPONSE_BYTES as u32,
        maximum_output_bytes: 0,
    });
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(kind),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(revision),
        inputs: (direction == PortDirection::Input)
            .then_some(vec![descriptor.clone()])
            .unwrap_or_default(),
        outputs: (direction == PortDirection::Output)
            .then_some(vec![descriptor])
            .unwrap_or_default(),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(FIXTURE_PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from("test/http-fixture@1"),
        },
        host_operations: observe.into_iter().collect(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: RESPONSE_BYTES as u32,
        },
    }
}

fn catalogs() -> (conduit_form::StartupCatalog, conduit_form::ProfileCatalog) {
    use conduit_form::{KindDefinition, KindSignature};
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_std_catalog::install_http_catalogs(&mut startup, &mut profile).unwrap();
    for offer in [
        fixture_offer(
            SOURCE_KIND,
            SOURCE_REVISION,
            SOURCE_IMPLEMENTATION,
            PortDirection::Output,
        ),
        fixture_offer(
            SINK_KIND,
            SINK_REVISION,
            SINK_IMPLEMENTATION,
            PortDirection::Input,
        ),
    ] {
        startup
            .insert(KindSignature {
                kind: offer.kind_id.as_str().into(),
                startup_parameters: Vec::new(),
            })
            .unwrap();
        profile
            .insert(KindDefinition {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: Vec::new(),
            })
            .unwrap();
    }
    (startup, profile)
}

fn advertisement() -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("conduitos-http-host"),
        boot_id: BootId::from("conduitos-http-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(PROFILE),
        resources: vec![resource_offer("conduitos-http-client-0", RESOURCE_CLASS, 1)],
        capabilities: vec![
            fixture_offer(
                SOURCE_KIND,
                SOURCE_REVISION,
                SOURCE_IMPLEMENTATION,
                PortDirection::Output,
            ),
            offer(),
            fixture_offer(
                SINK_KIND,
                SINK_REVISION,
                SINK_IMPLEMENTATION,
                PortDirection::Input,
            ),
        ],
        planner_capabilities: Vec::new(),
    }
}

struct Endpoint;
impl HttpNetworkBase for Endpoint {
    fn exchange(&mut self, request: &[u8], response: &mut [u8]) -> Result<usize, NetworkFailure> {
        assert_eq!(request, b"GET /ready HTTP/1.1\r\nHost: 192.0.2.9:8080\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
        let value = b"HTTP/1.1 201 Created\r\ncontent-length: 5\r\n\r\nready";
        response[..value.len()].copy_from_slice(value);
        Ok(value.len())
    }
}

#[test]
fn ordinary_form_plans_and_plays_native_http_through_the_production_kernel() {
    std::thread::Builder::new()
        .name("conduitos-http-fixed-kernel".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(run_ordinary_form)
        .unwrap()
        .join()
        .unwrap();
}

fn run_ordinary_form() {
    let source = format!(
        "form 0\n\nhttp-local {{\n source: {SOURCE_KIND}\n client: http/client\n sink: {SINK_KIND}\n source.value -> client.request\n client.response -> sink.value\n}}\n"
    );
    let (_startup, profile) = catalogs();
    let checked = conduit_form::parse(&source, &profile).unwrap();
    let host = advertisement();
    let hosts = [host.clone()];
    let placements = default_placements(&checked, &hosts).unwrap();
    let requirement = &host.capabilities[1].authority_requirements[0];
    let grant = AuthorityGrant {
        grant_id: AuthorityGrantId::from("grant/conduitos-http-local"),
        contract_id: requirement.contract_id.clone(),
        host_operation_contract_id: requirement.host_operation_contract_id.clone(),
        subject_kind: requirement.subject_kind.clone(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: host.capabilities[1].capability_id.clone(),
    };
    let options_without_authority = PlanningOptions {
        connection_bases: &alloc::collections::BTreeMap::new(),
        line_candidates: &alloc::collections::BTreeMap::new(),
        connection_item_capacity: 1,
        connection_byte_capacity: RESPONSE_BYTES as u32,
        authority_grants: &[],
        protected_resource_grants: &[],
        line_offers: &[],
    };
    assert!(
        plan_with_options(
            &checked,
            &hosts,
            &placements,
            &[ConnectionBase::Local],
            options_without_authority,
        )
        .is_err()
    );
    let grants = [grant];
    let plan = plan_with_options(
        &checked,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &alloc::collections::BTreeMap::new(),
            line_candidates: &alloc::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: RESPONSE_BYTES as u32,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    assert!(conduit_core::verify_plan(&plan));
    let fragment = &plan.fragments[0];
    let http = fragment
        .placements
        .iter()
        .find(|item| item.kind_id.as_str() == conduit_std_catalog::HTTP_CLIENT_KIND)
        .unwrap();
    assert_eq!(http.authority.len(), 1);
    assert_eq!(http.resources.len(), 1);
    assert_eq!(http.host_operations[0].maximum_in_flight, 1);
    let lowered = lower_plan_fragment(fragment).unwrap();

    let request = conduit_std_catalog::encode_request(&conduit_std_catalog::HttpRequest {
        transaction_id: conduit_std_catalog::HttpTransactionId(7),
        method: conduit_std_catalog::HttpMethod::Get,
        target: conduit_std_catalog::HttpTarget {
            scheme: "http".into(),
            authority: "192.0.2.9:8080".into(),
            path_and_query: "/ready".into(),
        },
        headers: Vec::new(),
        body: Vec::new(),
    })
    .unwrap();
    let mut values = FixedValueStore::<VALUE_SLOTS, VALUE_BYTES>::new(VALUE_BYTES as u32).unwrap();
    let request_value = values.store(&request).unwrap();
    let nodes = lowered.node_specs.as_slice().try_into().unwrap();
    let cords = [lowered.cords[0].spec, lowered.cords[1].spec];
    let mut routes = FixedRoutes::<{ MAX_NODES * PORTS }, 2>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut bindings = FixedHostOperationBindings::<9>::new(MAX_NODES as u16);
    for operation in &lowered.host_operations {
        bindings.install(operation.node, operation.binding).unwrap();
    }
    bindings.seal().unwrap();
    let mut drivers = [None, None, None];
    for (index, placement) in fragment.placements.iter().enumerate() {
        let operation = match placement.implementation_id.as_str() {
            SOURCE_IMPLEMENTATION => PlannedOperation::Source(Source {
                value: request_value,
                emitted: false,
            }),
            IMPLEMENTATION => PlannedOperation::Client(Client {
                pending: false,
                emitted: false,
            }),
            SINK_IMPLEMENTATION => PlannedOperation::Sink(Sink {
                pending: false,
                observed: false,
            }),
            _ => panic!("unexpected implementation"),
        };
        drivers[index] = Some(OperationDriver::<PlannedOperation, PORTS>::new(operation).unwrap());
    }
    let [Some(first), Some(second), Some(third)] = drivers else {
        panic!("all drivers")
    };
    let signs = FixedSignLog::<SIGN_CAPACITY>::new(
        (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32,
    )
    .unwrap();
    let mut kernel = FixedScheduler::<
        _,
        _,
        _,
        MAX_NODES,
        MAX_CORDS,
        PORTS,
        2,
        { MAX_NODES * PORTS },
        2,
        9,
        3,
    >::new_with_host_operations(
        nodes,
        cords,
        routes,
        bindings,
        [first, second, third],
        values,
        signs,
    )
    .unwrap();
    let mut native = NativeHttpClient::prepare();
    let mut output = FixedHttpOutput::new();
    let mut endpoint = Endpoint;
    let mut observed = Vec::new();
    for _ in 0..64 {
        while let Some(request) = kernel.next_host_request() {
            let binding = lowered
                .host_operations
                .iter()
                .find(|item| {
                    item.node == request.node && item.binding.operation == request.operation
                })
                .unwrap();
            let input = kernel.host_value(request.input.value).unwrap();
            if binding.contract_id.as_str() == HOST_OPERATION {
                native
                    .exchange(input, true, &mut endpoint, &mut output)
                    .unwrap();
                let value = kernel.store_host_value(output.as_bytes()).unwrap();
                kernel
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: Some(
                                BoundedValueRef::new(value, RESPONSE_BYTES as u32).unwrap(),
                            ),
                            failure: None,
                        },
                    )
                    .unwrap();
            } else {
                assert_eq!(binding.contract_id.as_str(), OBSERVE_OPERATION);
                observed.extend_from_slice(input);
                kernel
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
        }
        match kernel.step().unwrap() {
            SchedulerStatus::Complete => break,
            SchedulerStatus::Progress { .. } | SchedulerStatus::Idle => {}
            SchedulerStatus::Cancelled => panic!("unexpected cancellation"),
        }
    }
    let response = conduit_std_catalog::decode_response(&observed).unwrap();
    assert_eq!(
        response.transaction_id,
        conduit_std_catalog::HttpTransactionId(7)
    );
    assert_eq!(response.status, 201);
    assert_eq!(response.body, b"ready");
    assert!(kernel.signs().len() > 0);
}
