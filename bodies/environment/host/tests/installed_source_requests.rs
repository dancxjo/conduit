#![cfg(all(target_os = "linux", feature = "carrier-source"))]
//! Ordinary admitted kernel requests against a confined loopback provider.
//! This is process/ownership proof, not physical-device or membership evidence.
use conduit_core::*;
use conduit_environment_host::source_operator::*;
use conduit_environment_runtime::environmental_reading::source_observation::{
    ObservationContract, SourceOutcome,
};
use conduit_host_make::*;
use conduit_kernel::scheduler::{HostCallRequest, SchedulerStatus};
use conduit_std_host::isolated_http_base::{IsolatedHttpBaseConfig, ISOLATED_HTTP_IMPLEMENTATION};
use conduit_std_host::{AdmittedRemoteFragment, StdHost, StdHostConfig};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

struct Device {
    endpoint: std::net::SocketAddr,
    requests: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    held: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Device {
    fn new(configuration: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let requests = Arc::new(AtomicU64::new(0));
        let counter = requests.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let held = Arc::new(AtomicBool::new(false));
        let holding = held.clone();
        let worker = std::thread::spawn(move || {
            while !stopped.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                    assert!(request.len() < 4096);
                }
                assert!(request.starts_with(b"GET /environment HTTP/1.1\r\n"));
                let sequence = counter.fetch_add(1, Ordering::SeqCst) + 1;
                let deadline = std::time::Instant::now() + Duration::from_secs(2);
                while holding.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(2));
                }
                let body = serde_json::json!({"role":"conduit.environment-wifi-carrier@1", "build":"01".repeat(32), "configuration":configuration, "body_member":false, "samples":sequence, "attempts":sequence, "sensor_error":false, "temperature_centi_celsius":2150, "pressure_centi_pascal":10000000, "humidity_percent_q10":null, "sampled_at_boot_ms":123}).to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        Self {
            endpoint,
            requests,
            stop,
            held,
            worker: Some(worker),
        }
    }
    fn count(&self) -> u64 {
        self.requests.load(Ordering::SeqCst)
    }
}
impl Drop for Device {
    fn drop(&mut self) {
        self.held.store(false, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        self.worker.take().unwrap().join().expect("fixture worker");
    }
}
fn checked() -> CheckedHostConfiguration {
    let packages = MakePackageSet::compose(&[&conduit_host_rp2040::Rp2040MakePackage]).unwrap();
    check_host_configuration(
        parse_host_configuration_conduit(include_str!(
            "../../../../targets/rp2040/profiles/pico-environment-usb.host.conduit"
        ))
        .unwrap(),
        &MakeCatalog::canonical().with_packages(&packages),
        &packages,
    )
    .unwrap()
}
fn ready<F: std::future::Future>(future: F) -> F::Output {
    struct Noop;
    impl std::task::Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let waker = std::task::Waker::from(Arc::new(Noop));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) {
        std::task::Poll::Ready(value) => value,
        std::task::Poll::Pending => panic!("bounded synchronous native fixture suspended"),
    }
}
fn plan(
    host: &HostAdvertisement,
    capability: &CapabilityId,
    grants: &[AuthorityGrant],
    http: bool,
) -> Plan {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_catalog(&mut startup, &mut profiles).unwrap();
    conduit_web::install_http_catalogs(&mut startup, &mut profiles).unwrap();
    startup
        .insert_value_kind_alias(
            "Observation",
            ObservationContract::prepare()
                .unwrap()
                .kind(false)
                .unwrap()
                .outputs[0]
                .value_kind
                .clone(),
        )
        .unwrap();
    startup
        .insert_value_kind_alias(
            "Request",
            conduit_web::http_request_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
        )
        .unwrap();
    startup
        .insert_value_kind_alias(
            "Response",
            conduit_web::http_response_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
        )
        .unwrap();
    let source = if http {
        "plot admitted (\n >> begin: Request...|\n result: Response...| >>\n) {\n operation: http/client()\n begin >> operation.request\n operation.response >> result\n}\n"
    } else {
        "plot admitted (\n >> begin: Empty...|\n result: Observation...| >>\n) {\n operation: environment/read-environmental-source()\n begin >> operation.request\n operation.observation >> result\n}\n"
    };
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "admitted", &profiles).unwrap();
    let placements = conduit_planner::PlacementChoices {
        by_gear: expanded
            .expanded
            .gears
            .iter()
            .map(|gear| {
                (
                    gear.gear_id.clone(),
                    conduit_planner::PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: capability.clone(),
                    },
                )
            })
            .collect(),
    };
    let mut fore = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, &expanded.input_bindings),
        (PortDirection::Output, &expanded.output_bindings),
    ] {
        for binding in bindings {
            fore.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if http {
                        conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES
                            .max(conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES)
                    } else {
                        128
                    },
                },
            );
        }
    }
    conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &[host.clone()],
        &placements,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: if http {
                conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES
                    .max(conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES)
            } else {
                128
            },
            authority_grants: grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &fore,
    )
    .unwrap()
}
fn pending(admitted: &mut AdmittedRemoteFragment) -> HostCallRequest {
    for _ in 0..64 {
        if let Some(request) = admitted.runtime_mut().next_host_request() {
            return request;
        }
        assert!(!matches!(
            admitted.runtime_mut().step().unwrap(),
            SchedulerStatus::Cancelled | SchedulerStatus::Drained
        ));
    }
    panic!("ordinary admitted request not dispatched");
}
fn begin(
    host: &mut StdHost,
    plan: &Plan,
    input: &[u8],
) -> (AdmittedRemoteFragment, HostCallRequest) {
    let mut admitted = host.prepare_remote_fragment(&plan.fragments[0]).unwrap();
    admitted
        .runtime_mut()
        .admit_local_fore_input(&"begin".into(), 0, input)
        .unwrap();
    admitted
        .runtime_mut()
        .close_local_fore_input(&"begin".into())
        .unwrap();
    let request = pending(&mut admitted);
    (admitted, request)
}
fn output(admitted: &mut AdmittedRemoteFragment) -> Vec<u8> {
    for _ in 0..64 {
        if let Some(bytes) = admitted
            .runtime_mut()
            .take_local_fore_output(&"result".into())
            .unwrap()
        {
            return bytes;
        }
        assert!(!matches!(
            admitted.runtime_mut().step().unwrap(),
            SchedulerStatus::Cancelled
        ));
    }
    panic!("ordinary typed Fore output absent");
}
fn complete(
    host: StdHost,
    equipment: DetachedCollectorEquipment,
    admitted: AdmittedRemoteFragment,
    request: HostCallRequest,
) -> (
    StdHost,
    DetachedCollectorEquipment,
    AdmittedRemoteFragment,
    Result<bool, Refusal>,
) {
    std::thread::spawn(move || ready(equipment.complete_admitted_request(host, admitted, request)))
        .join()
        .expect("ordinary native worker returns all affine owners")
}

#[test]
fn source_and_http_requests_use_existing_admission_and_reject_stale_effects_before_io() {
    let checked = checked();
    let device = Device::new(checked.configuration_id().to_owned());
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "fixture/ordinary-source".into(),
        boot_id: "fixture/ordinary-source-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut collector = CollectorRoot::new_installed(host);
    let source = collector
        .attach_network(
            NetworkSourceConfiguration {
                http: IsolatedHttpBaseConfig {
                    executable: env!("CARGO_BIN_EXE_conduit-environment-http-provider").into(),
                    base_instance_id: "fixture/acquisition".into(),
                    provider_generation: 1,
                    authority: format!("device.invalid:{}", device.endpoint.port()),
                    endpoint: device.endpoint,
                    resource_generation_id: ResourceGenerationId(
                        "fixture/acquisition-generation".into(),
                    ),
                },
                firmware: [1; 32],
            },
            &checked,
            Duration::from_secs(31),
        )
        .unwrap();
    let descriptor = collector.source_descriptor(source).unwrap();
    let grants = collector.native_authority_grants().unwrap();
    let inventory = collector.seal_inventory().unwrap();
    assert_eq!(descriptor.host(), &inventory.host_id);
    assert_eq!(descriptor.boot(), &inventory.boot_id);
    let source_plan = plan(&inventory, descriptor.capability(), &grants, false);
    let http_capability = inventory
        .capabilities
        .iter()
        .find(|offer| {
            offer.implementation.implementation_id.as_str() == ISOLATED_HTTP_IMPLEMENTATION
        })
        .unwrap()
        .capability_id
        .clone();
    let http_plan = plan(&inventory, &http_capability, &grants, true);
    let (mut host, equipment) = collector.detach().unwrap();
    let (admitted, request) = begin(&mut host, &source_plan, &[]);
    let initial_play = admitted.identity().play_sequence;
    assert!(!host.native_resources_idle());
    let before = device.count();
    let wrong = HostCallRequest {
        request: conduit_kernel::RequestId(request.request.0 + 1),
        ..request
    };
    let (host, equipment, admitted, result) = complete(host, equipment, admitted, wrong);
    assert!(result.is_err());
    assert_eq!(device.count(), before);
    let (host, equipment, mut admitted, result) = complete(host, equipment, admitted, request);
    assert_eq!(result, Ok(true));
    let reading = ObservationContract::prepare()
        .unwrap()
        .decode(&output(&mut admitted))
        .unwrap();
    assert!(matches!(reading.outcome, SourceOutcome::Available(_)));
    let before = device.count();
    let (mut host, equipment, admitted, result) = complete(host, equipment, admitted, request);
    assert!(result.is_err());
    assert_eq!(device.count(), before);
    host.release_remote_fragment(admitted).unwrap();
    assert!(host.native_resources_idle());
    let (mut admitted, cancelled_request) = begin(&mut host, &source_plan, &[]);
    admitted.runtime_mut().cancel().unwrap();
    let (mut host, equipment, admitted, result) =
        complete(host, equipment, admitted, cancelled_request);
    assert!(result.is_err());
    assert_eq!(device.count(), before);
    host.release_remote_fragment(admitted).unwrap();
    assert!(host.native_resources_idle());
    let request_value = conduit_web::HttpRequest {
        transaction_id: conduit_web::HttpTransactionId::new(9).unwrap(),
        method: conduit_web::HttpMethod::Get,
        target: conduit_web::HttpTarget::new(
            format!("device.invalid:{}", device.endpoint.port()),
            "/environment".into(),
            conduit_web::HttpScheme::Http,
        )
        .unwrap(),
        headers: Default::default(),
        body: conduit_web::HttpBody::inline([]),
    };
    let encoded = conduit_web::encode_request(&request_value).unwrap();
    let (admitted, http_request) = begin(&mut host, &http_plan, &encoded);
    assert!(admitted.identity().play_sequence > initial_play);
    let (mut host, equipment, mut admitted, result) =
        complete(host, equipment, admitted, http_request);
    assert_eq!(result, Ok(true));
    assert_eq!(device.count(), before + 1);
    let response = conduit_web::decode_response(&output(&mut admitted)).unwrap();
    assert_eq!(response.status, 200);
    host.release_remote_fragment(admitted).unwrap();
    assert!(host.native_resources_idle());
    assert_eq!(host.advertisement(), &inventory);
    let collector = equipment.resume(host).unwrap();
    let retained = collector.source_descriptor(source).unwrap();
    assert_eq!(retained.session(), descriptor.session());
    assert_eq!(retained.capability(), descriptor.capability());
}

#[test]
fn original_native_lease_stays_owned_through_inflight_cancellation_and_late_output_is_discarded() {
    let checked = checked();
    let device = Device::new(checked.configuration_id().to_owned());
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "fixture/inflight-source".into(),
        boot_id: "fixture/inflight-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut collector = CollectorRoot::new_installed(host);
    let source = collector
        .attach_network(
            NetworkSourceConfiguration {
                http: IsolatedHttpBaseConfig {
                    executable: env!("CARGO_BIN_EXE_conduit-environment-http-provider").into(),
                    base_instance_id: "fixture/inflight-acquisition".into(),
                    provider_generation: 1,
                    authority: format!("device.invalid:{}", device.endpoint.port()),
                    endpoint: device.endpoint,
                    resource_generation_id: ResourceGenerationId(
                        "fixture/inflight-acquisition-resource".into(),
                    ),
                },
                firmware: [1; 32],
            },
            &checked,
            Duration::from_secs(31),
        )
        .unwrap();
    let descriptor = collector.source_descriptor(source).unwrap();
    let grants = collector.native_authority_grants().unwrap();
    let inventory = collector.seal_inventory().unwrap();
    assert!(!inventory.capabilities.iter().any(|offer| offer
        .implementation
        .implementation_id
        .as_str()
        == "linux/environmental-publication@1"));
    let before = device.count();
    assert!(matches!(
        ready(collector.sample(source)),
        Err(Refusal::Authority)
    ));
    assert_eq!(device.count(), before);
    let source_plan = plan(&inventory, descriptor.capability(), &grants, false);
    let (mut host, equipment) = collector.detach().unwrap();
    let (mut admitted, request) = begin(&mut host, &source_plan, &[]);
    let work = host.prepare_native_work(&mut admitted, request).unwrap();
    assert!(!host.native_resources_idle());
    assert!(host.prepare_native_work(&mut admitted, request).is_err());
    let foreign = StdHost::new_with_config(StdHostConfig {
        host_id: inventory.host_id.clone(),
        boot_id: "fixture/foreign-work-boot".into(),
        offer_generation: inventory.offer_generation,
    });
    let refusal = foreign.return_native_work(&mut admitted, work).unwrap_err();
    let work = refusal.work;
    assert_eq!(device.count(), before);
    device.held.store(true, Ordering::SeqCst);
    let worker = std::thread::spawn(move || ready(equipment.complete_native_work(host, work)));
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    while device.count() == before && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(
        device.count() > before,
        "native IO must actually be in flight"
    );
    assert!(!worker.is_finished());
    // The actual kernel/session remains owned by this control thread. Cancellation
    // cannot release the worker's ORIGINAL lease or create a replacement owner.
    admitted.runtime_mut().cancel().unwrap();
    device.held.store(false, Ordering::SeqCst);
    let (mut host, equipment, mut work, captured) = worker.join().unwrap();
    assert!(captured.is_ok());
    assert!(!host.native_resources_idle());
    assert!(
        host.begin_native_work(&mut work).is_err(),
        "one native work cannot repeat physical IO"
    );
    let returned = host.return_native_work(&mut admitted, work).unwrap();
    assert!(!host.native_resources_idle());
    assert!(host.prepare_native_work(&mut admitted, request).is_err());
    assert!(!returned
        .complete(&mut admitted, captured.as_ref().ok().map(Vec::as_slice))
        .unwrap());
    assert!(admitted
        .runtime_mut()
        .take_local_fore_output(&"result".into())
        .unwrap()
        .is_none());
    host.release_remote_fragment(admitted).unwrap();
    assert!(host.native_resources_idle());
    assert_eq!(host.advertisement(), &inventory);
    let collector = equipment.resume(host).unwrap();
    assert_eq!(
        collector.source_descriptor(source).unwrap().session(),
        descriptor.session()
    );
}
