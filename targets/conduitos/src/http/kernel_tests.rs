use super::*;
extern crate std;
use alloc::{format, vec, vec::Vec};
use conduit_core::{
    ArtifactId, AuthorityGrant, AuthorityGrantId, BaseImplementationId, BootId, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostAdvertisement, HostCallContractId,
    HostCallRequirement, HostId, HostProfileId, ImplementationId, ImplementationOffer,
    KindIdentity, OfferGeneration, PROTOCOL_VERSION, PortDescriptor, PortDirection, PortTemporal,
    kind_id, port_id, resource_offer,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallDisposition, HostCallId, HostCallOutcome, KernelEvent, PortId, RequestId, SignSink,
    ValueRef, ValueStorage,
    scheduler::{FixedScheduler, SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome},
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};

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
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const VALUE_SLOTS: usize = 8;
const VALUE_BYTES: usize = REQUEST_BYTES + RESPONSE_BYTES;
const SIGN_CAPACITY: usize = 96;

#[derive(Clone, Copy)]
struct Source {
    value: ValueRef,
    emitted: bool,
}

impl Source {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.emitted {
            return StepOutcome::Complete;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if io.send(PortId(0), self.value).is_err() {
            return invalid(1);
        }
        self.emitted = true;
        StepOutcome::Progress
    }
}

#[derive(Clone, Copy)]
struct Client {
    pending: bool,
    emitted: bool,
}

impl Client {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            let Some(output) = outcome.output else {
                return invalid(2);
            };
            if request != RequestId(0)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.failure.is_some()
                || !io.output_ready(PortId(0))
                || io.consume_host_completion().is_err()
                || io.send(PortId(0), output.value).is_err()
            {
                return invalid(2);
            }
            self.pending = false;
            self.emitted = true;
            return StepOutcome::Progress;
        }
        if !self.emitted
            && let Some(value) = io.input(PortId(0))
        {
            let Ok(input) = BoundedValueRef::new(value, REQUEST_BYTES as u32) else {
                return invalid(2);
            };
            if io.consume(PortId(0)).is_err()
                || io
                    .request_host_call(RequestId(0), HostCallId(0), input)
                    .is_err()
            {
                return invalid(2);
            }
            self.pending = true;
            return StepOutcome::Progress;
        }
        if self.emitted && io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return invalid(2);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

#[derive(Clone, Copy)]
struct Sink {
    pending: bool,
    observed: bool,
}

impl Sink {
    fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.pending {
            let Some((request, outcome)) = io.host_completion() else {
                return StepOutcome::Await;
            };
            if request != RequestId(0)
                || outcome.disposition != HostCallDisposition::Completed
                || outcome.output.is_some()
                || outcome.failure.is_some()
                || io.consume_host_completion().is_err()
            {
                return invalid(3);
            }
            self.pending = false;
            self.observed = true;
            return StepOutcome::Progress;
        }
        if let Some(value) = io.input(PortId(0)) {
            let Ok(input) = BoundedValueRef::new(value, RESPONSE_BYTES as u32) else {
                return invalid(3);
            };
            if io.consume(PortId(0)).is_err()
                || io
                    .request_host_call(RequestId(0), HostCallId(0), input)
                    .is_err()
            {
                return invalid(3);
            }
            self.pending = true;
            return StepOutcome::Progress;
        }
        if self.observed && io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return invalid(3);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

enum PlannedBack {
    Source(Source),
    Client(Client),
    Sink(Sink),
}

impl StepBack<PORTS> for PlannedBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Source(v) => v.step(io),
            Self::Client(v) => v.step(io),
            Self::Sink(v) => v.step(io),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(_) => {}
            Self::Client(v) => v.pending = false,
            Self::Sink(v) => v.pending = false,
        }
    }
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
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
        value_kind: if direction == PortDirection::Output {
            conduit_web::http_request_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        } else {
            conduit_web::http_response_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone()
        },
        direction,
        temporal: PortTemporal::Flow { closes: true },
    };
    let observe = (direction == PortDirection::Input).then(|| HostCallRequirement {
        contract_id: HostCallContractId::from(OBSERVE_OPERATION),
        target_kind: Some(
            conduit_web::http_response_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
        ),
        maximum_in_flight: 1,
        maximum_input_bytes: RESPONSE_BYTES as u32,
        maximum_output_bytes: 0,
    });
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(kind),
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(revision),
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
        host_calls: observe.into_iter().collect(),
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
    use conduit_form::{KindProjection, KindSignature};
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_web::install_http_catalogs(&mut startup, &mut profile).unwrap();
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
            .insert(KindProjection {
                kind_id: offer.kind_id,
                kind_contract_revision: offer.kind_contract_revision,
                inputs: offer.inputs,
                outputs: offer.outputs,
                configuration: Default::default(),
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
        bases: vec![],
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
        "form http-local {{\n source: {SOURCE_KIND}\n client: http/client\n sink: {SINK_KIND}\n source.value >> client.request\n client.response >> sink.value\n}}\n"
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
        host_call_contract_id: requirement.host_call_contract_id.clone(),
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
            &[BaseImplementationId::from("conduit.base/local@1")],
            options_without_authority,
        )
        .is_err()
    );
    let grants = [grant];
    let plan = plan_with_options(
        &checked,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
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
        .find(|item| item.kind_id.as_str() == conduit_web::HTTP_CLIENT_KIND)
        .unwrap();
    assert_eq!(http.authority.len(), 1);
    assert_eq!(http.resources.len(), 1);
    assert_eq!(http.host_calls[0].maximum_in_flight, 1);
    let lowered = lower_plan_fragment(fragment).unwrap();

    let request = conduit_web::encode_request(&conduit_web::HttpRequest {
        transaction_id: conduit_web::HttpTransactionId(7),
        method: conduit_web::HttpMethod::Get,
        target: conduit_web::HttpTarget {
            scheme: "http".into(),
            authority: "192.0.2.9:8080".into(),
            path_and_query: "/ready".into(),
        },
        headers: Vec::new(),
        body: conduit_web::HttpBody::inline(Vec::new()),
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
    let mut bindings = FixedHostCallBindings::<9>::new(MAX_NODES as u16);
    for operation in &lowered.host_calls {
        bindings.install(operation.node, operation.binding).unwrap();
    }
    bindings.seal().unwrap();
    let mut drivers = [None, None, None];
    for (index, placement) in fragment.placements.iter().enumerate() {
        let back = match placement.implementation_id.as_str() {
            SOURCE_IMPLEMENTATION => PlannedBack::Source(Source {
                value: request_value,
                emitted: false,
            }),
            IMPLEMENTATION => PlannedBack::Client(Client {
                pending: false,
                emitted: false,
            }),
            SINK_IMPLEMENTATION => PlannedBack::Sink(Sink {
                pending: false,
                observed: false,
            }),
            _ => panic!("unexpected implementation"),
        };
        drivers[index] = Some(back);
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
    >::new_with_host_calls(
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
                .host_calls
                .iter()
                .find(|item| item.node == request.node && item.binding.call == request.call)
                .unwrap();
            let input = kernel.host_value(request.input.value).unwrap();
            if binding.contract_id.as_str() == HOST_CALL {
                native
                    .exchange(input, true, &mut endpoint, &mut output)
                    .unwrap();
                let value = kernel.store_host_value(output.as_bytes()).unwrap();
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
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
                    .complete_host_call(
                        request.node,
                        request.request,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
        }
        match kernel.step().unwrap() {
            SchedulerStatus::Drained => break,
            SchedulerStatus::Progress { .. } | SchedulerStatus::Idle => {}
            SchedulerStatus::Cancelled => panic!("unexpected cancellation"),
        }
    }
    let response = conduit_web::decode_response(&observed).unwrap();
    assert_eq!(response.transaction_id, conduit_web::HttpTransactionId(7));
    assert_eq!(response.status, 201);
    assert_eq!(response.body.as_inline(), Some(b"ready".as_slice()));
    assert!(kernel.signs().len() > 0);
}
