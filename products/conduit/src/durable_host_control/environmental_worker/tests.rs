//! Opted-in process proof of the actual production actor during native IO.
use super::*;
use crate::durable_host::selected_environmental_sources::{AttachedEquipment, SelectedInstrument};
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, CapabilityId, GearId, LineScope,
    LineSecurity, LinkLimits, OfferGeneration, PortDirection,
};
use conduit_environment_host::source_operator::{CollectorRoot, NetworkSourceConfiguration};
use conduit_planner::{PlacementChoice, PlacementChoices};
use conduit_std_host::{isolated_http_base::IsolatedHttpBaseConfig, StdHostConfig};
use std::{
    collections::BTreeMap,
    net::TcpListener,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

struct GatedDevice {
    endpoint: std::net::SocketAddr,
    armed: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    count: Arc<AtomicU64>,
    entered: mpsc::Receiver<()>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl GatedDevice {
    fn new(configuration: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = listener.local_addr().unwrap();
        let armed = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let count = Arc::new(AtomicU64::new(0));
        let (tx, entered) = mpsc::channel();
        let (a, r, s, c) = (armed.clone(), release.clone(), stop.clone(), count.clone());
        let worker = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(value) => value,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(2));
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
                let sequence = c.fetch_add(1, Ordering::SeqCst) + 1;
                if a.swap(false, Ordering::SeqCst) {
                    tx.send(()).unwrap();
                    let deadline = Instant::now() + Duration::from_millis(1500);
                    while !r.load(Ordering::SeqCst)
                        && !s.load(Ordering::SeqCst)
                        && Instant::now() < deadline
                    {
                        std::thread::sleep(Duration::from_millis(2));
                    }
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
            armed,
            release,
            stop,
            count,
            entered,
            worker: Some(worker),
        }
    }
}
impl Drop for GatedDevice {
    fn drop(&mut self) {
        self.release.store(true, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        let _ = self.worker.take().unwrap().join();
    }
}
fn mixed_plan(
    host: &StdHost,
    peer: &StdHost,
    capability: &CapabilityId,
    grants: &[conduit_core::AuthorityGrant],
) -> Plan {
    let (mut startup, profiles) = crate::plot_source::standard_catalogs().unwrap();
    startup
        .insert_value_kind_alias(
            "NativeObservation",
            host.advertisement()
                .capabilities
                .iter()
                .find(|offer| &offer.capability_id == capability)
                .unwrap()
                .outputs[0]
                .value_kind
                .clone(),
        )
        .unwrap();
    let source = "plot native_actor (\n >> begin: Empty...|\n observation: NativeObservation...| >>\n text: Text...| >>\n) {\n native: environment/read-environmental-source()\n upper: text/upper()\n begin >> native.request\n native.observation >> observation\n \"actor peer\" >> upper\n upper >> text\n}\n";
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "native_actor", &profiles)
            .unwrap();
    let advertisements = [host.advertisement().clone(), peer.advertisement().clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .expanded
            .gears
            .iter()
            .map(|gear| {
                let (ad, selected) =
                    if gear.kind_id.as_str() == "environment/read-environmental-source" {
                        (&advertisements[0], capability.clone())
                    } else {
                        let ad = if gear.kind_id.as_str() == conduit_text::TEXT_UPPER_KIND {
                            &advertisements[1]
                        } else {
                            &advertisements[0]
                        };
                        (
                            ad,
                            ad.capabilities
                                .iter()
                                .find(|offer| offer.kind_id == gear.kind_id)
                                .unwrap()
                                .capability_id
                                .clone(),
                        )
                    };
                (
                    GearId::from(gear.gear_id.as_str()),
                    PlacementChoice {
                        host_id: ad.host_id.clone(),
                        capability_id: selected,
                    },
                )
            })
            .collect(),
    };
    let mut line = process_owned_line_offer_with_limits(
        "actor/native-line",
        "actor/native-line/binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "actor/native-line/instance",
        &advertisements[0],
        &advertisements[1],
        LinkLimits {
            maximum_in_flight_items: 4,
            maximum_payload_bytes: 4096,
            maximum_buffered_bytes: 16384,
            maximum_frame_bytes: 8192,
        },
    );
    line.contract.scope = LineScope::LocalNetwork;
    line.contract.security = LineSecurity::PlaintextNetwork;
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
                    byte_capacity: 4096,
                },
            );
        }
    }
    conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &advertisements,
        &placements,
        &[
            "conduit.base/local@1".into(),
            "conduit.base/websocket-rfc6455@1".into(),
        ],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: grants,
            protected_resource_grants: &[],
            line_offers: &[line],
        },
        &fore,
    )
    .unwrap()
}
fn exchange(runtime: &mut DurableHostRuntime, token: &[u8; 32], frame: Vec<u8>) -> Vec<Vec<u8>> {
    match handle(
        Request::ExchangeRemote {
            protocol: PROTOCOL,
            token: token.to_vec(),
            frame,
        },
        token,
        runtime,
    ) {
        Response::RemoteExchanged { responses, .. } => responses,
        Response::Refused { code, .. } => panic!("actual remote exchange refused: {code}"),
        _ => panic!("remote exchange returned another response"),
    }
}
fn host_is_out(runtime: &DurableHostRuntime) -> bool {
    match &runtime.host {
        HostSource::Bare(owner) => owner.is_playing(),
        _ => panic!("fixture has no Body"),
    }
}

#[test]
fn ordinary_actor_keeps_control_and_session_while_native_io_returns_after_cancel() {
    let Some(worker) = std::env::var_os("CONDUIT_ENVIRONMENT_TEST_WORKER") else {
        eprintln!(
            "native actor process proof skipped: explicitly set CONDUIT_ENVIRONMENT_TEST_WORKER"
        );
        return;
    };
    let worker = PathBuf::from(worker);
    assert!(worker.is_absolute() && worker.is_file());
    let packages = conduit_workspace_make::package_set();
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(include_str!(
            "../../../../../targets/rp2040/profiles/pico-environment-usb.host.conduit"
        ))
        .unwrap(),
        &conduit_workspace_make::catalog(),
        &packages,
    )
    .unwrap();
    let device = GatedDevice::new(checked.configuration_id().to_owned());
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "actor/native-host".into(),
        boot_id: "actor/native-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let peer = StdHost::new_with_config(StdHostConfig {
        host_id: "actor/peer-host".into(),
        boot_id: "actor/peer-boot".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut collector = CollectorRoot::new_installed(host);
    let source = collector
        .attach_network(
            NetworkSourceConfiguration {
                http: IsolatedHttpBaseConfig {
                    executable: worker,
                    base_instance_id: "actor/acquisition".into(),
                    provider_generation: 1,
                    authority: format!("actor-device.invalid:{}", device.endpoint.port()),
                    endpoint: device.endpoint,
                    resource_generation_id: conduit_core::ResourceGenerationId(
                        "actor/acquisition-generation".into(),
                    ),
                },
                firmware: [1; 32],
            },
            &checked,
            Duration::from_secs(31),
        )
        .unwrap();
    let session = collector.source_descriptor(source).unwrap();
    let grants = collector.native_authority_grants().unwrap();
    let inventory = collector.seal_inventory().unwrap();
    let (host, native) = collector.detach().unwrap();
    let plan = mixed_plan(&host, &peer, session.capability(), &grants);
    let equipment = AttachedEquipment {
        native,
        instruments: vec![SelectedInstrument {
            attachment_id: "actor-device".into(),
            source_configuration: checked,
            firmware: [1; 32],
            session,
        }],
        authority_grants: grants,
    };
    let mut runtime = DurableHostRuntime::new(
        "std/x86_64/computer".into(),
        format!("sha256:{}", "a".repeat(64)),
        host,
    )
    .with_selected_environmental_equipment(Some(equipment));
    let prepared = runtime
        .prepare_remote(
            inventory.boot_id.as_str(),
            inventory.offer_generation.0,
            &plan,
        )
        .unwrap();
    assert!(!prepared.hello_frames.is_empty());
    let identity = prepared.identity.clone();
    runtime
        .remote_fragment
        .as_mut()
        .unwrap()
        .runtime_mut()
        .admit_local_fore_input(&"begin".into(), 0, &[])
        .unwrap();
    runtime
        .remote_fragment
        .as_mut()
        .unwrap()
        .runtime_mut()
        .close_local_fore_input(&"begin".into())
        .unwrap();
    device.armed.store(true, Ordering::SeqCst);
    let token = [23; 32];
    let hello = prepared.hello_frames[0].clone();
    let ready = exchange(&mut runtime, &token, hello.clone()).remove(0);
    let decoded = decode_session_frame(
        &ready,
        MAXIMUM_CONTROL_FRAME_BYTES as u32,
        MAXIMUM_CONTROL_FRAME_BYTES as u32,
    )
    .unwrap();
    assert!(matches!(decoded.message, SessionMessage::Ready));
    exchange(&mut runtime, &token, ready);
    // The ordinary actor starts its worker through the real authenticated handler.
    assert!(runtime.environmental_worker_active());
    device
        .entered
        .recv_timeout(Duration::from_millis(750))
        .expect("native IO reaches the bounded fixture gate");
    assert!(host_is_out(&runtime));
    assert!(runtime.pending_native_source.is_some());
    assert_eq!(
        runtime.remote_fragment.as_ref().unwrap().identity(),
        &identity
    );
    assert_eq!(runtime.truth().advertisement, inventory);
    let response = handle(
        Request::Status {
            protocol: PROTOCOL,
            token: token.to_vec(),
        },
        &token,
        &mut runtime,
    );
    assert!(
        matches!(response, Response::Status { advertisement, .. } if advertisement == inventory)
    );
    assert!(runtime.environmental_worker_active() && host_is_out(&runtime));
    let decoded = decode_session_frame(
        &hello,
        MAXIMUM_CONTROL_FRAME_BYTES as u32,
        MAXIMUM_CONTROL_FRAME_BYTES as u32,
    )
    .unwrap();
    let mut cancelled = vec![0; MAXIMUM_CONTROL_FRAME_BYTES];
    let length = encode_session_frame_into(
        conduit_wire::SessionFrame {
            identity: decoded.identity,
            message: SessionMessage::Cancelled { code: 9 },
        },
        &mut cancelled,
        MAXIMUM_CONTROL_FRAME_BYTES as u32,
        MAXIMUM_CONTROL_FRAME_BYTES as u32,
    )
    .unwrap();
    cancelled.truncate(length);
    let cancellation = exchange(&mut runtime, &token, cancelled);
    assert!(cancellation.iter().any(|frame| matches!(
        decode_session_frame(
            frame,
            MAXIMUM_CONTROL_FRAME_BYTES as u32,
            MAXIMUM_CONTROL_FRAME_BYTES as u32
        )
        .unwrap()
        .message,
        SessionMessage::Cancelled { code: 9 }
    )));
    assert!(runtime.environmental_worker_active() && host_is_out(&runtime));
    assert_eq!(
        runtime.remote_fragment.as_ref().unwrap().identity(),
        &identity
    );
    // Cancellation cannot release an in-flight provider's original reservation.
    device.release.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(3);
    while runtime.environmental_worker_active() && Instant::now() < deadline {
        runtime.progress_environmental_worker().unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!runtime.environmental_worker_active() && !host_is_out(&runtime));
    assert!(
        device.count.load(Ordering::SeqCst) >= 3,
        "native Source actually completed a fresh counter after cancellation"
    );
    assert!(runtime.pending_native_source.is_none());
    assert_eq!(runtime.truth().advertisement, inventory);
    assert!(!runtime.host.native_resources_idle());
    let admitted = runtime.remote_fragment.as_mut().unwrap();
    assert_eq!(admitted.identity(), &identity);
    assert!(
        !matches!(
            admitted
                .runtime_mut()
                .take_local_fore_output(&"observation".into()),
            Ok(Some(_))
        ),
        "late native bytes must not become an ordinary committed output"
    );
    runtime.release_remote().unwrap();
    assert!(runtime.host.native_resources_idle());
    assert_eq!(runtime.truth().advertisement, inventory);
    assert!(runtime.selected_environmental_equipment.is_some());
}
