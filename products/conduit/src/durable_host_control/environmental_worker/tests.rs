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
        atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
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
    fault: Arc<AtomicU8>,
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
        let fault = Arc::new(AtomicU8::new(0));
        let f = fault.clone();
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
                let mode = f.load(Ordering::SeqCst);
                if mode == 1 {
                    // Existing provider timeout is two seconds; exceed it without
                    // changing production deadlines or resuming the failed worker.
                    let deadline = Instant::now() + Duration::from_millis(2300);
                    while Instant::now() < deadline && !s.load(Ordering::SeqCst) {
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    continue;
                }
                let configuration = if mode == 2 {
                    "private-config-must-not-leak"
                } else {
                    &configuration
                };
                let sequence = if mode == 3 { 1 } else { sequence };
                let body = serde_json::json!({"role":"conduit.environment-wifi-carrier@1", "build":"01".repeat(32), "configuration":configuration, "body_member":false, "samples":sequence, "attempts":sequence, "sensor_error":false, "temperature_centi_celsius": if mode == 4 { serde_json::json!("private-payload-must-not-leak") } else { serde_json::json!(2150) }, "pressure_centi_pascal":10000000, "humidity_percent_q10":null, "sampled_at_boot_ms":123}).to_string();
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .ok();
            }
        });
        Self {
            endpoint,
            armed,
            release,
            stop,
            count,
            fault,
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
    http: bool,
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
    let source = "plot native_actor (\n >> begin: Empty...|\n observation: NativeObservation...| >>\n text: Text >>\n) {\n native: environment/read-environmental-source()\n upper: text/upper()\n begin >> native.request\n native.observation >> observation\n \"actor peer\" >> upper\n upper >> text\n}\n";
    let source = if http {
        let input = host
            .advertisement()
            .capabilities
            .iter()
            .find(|offer| &offer.capability_id == capability)
            .unwrap()
            .inputs[0]
            .value_kind
            .clone();
        startup
            .insert_value_kind_alias("NativeRequest", input)
            .unwrap();
        source
            .replace("Empty...|", "NativeRequest...|")
            .replace("environment/read-environmental-source", "http/client")
            .replace("native.observation", "native.response")
    } else {
        source.to_owned()
    };
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
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
                let (ad, selected) = if matches!(
                    gear.kind_id.as_str(),
                    "environment/read-environmental-source" | conduit_web::HTTP_CLIENT_KIND
                ) {
                    (&advertisements[0], capability.clone())
                } else {
                    let ad = if gear.kind_id.as_str() == conduit_text::TEXT_UPPER_KIND {
                        &advertisements[0]
                    } else {
                        &advertisements[1]
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
        &advertisements[1],
        &advertisements[0],
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
            let placement = &placements.by_gear[&binding.gear_id];
            let offer = advertisements
                .iter()
                .find(|ad| ad.host_id == placement.host_id)
                .unwrap()
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .unwrap();
            fore.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: if http && &placement.capability_id == capability {
                        offer.limits.max_queue_bytes
                    } else {
                        offer.limits.max_queue_bytes.min(256)
                    },
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
            connection_byte_capacity: 256,
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
        HostSource::Body { owner, .. } => owner.host.is_playing(),
        HostSource::Transitioning => panic!("fixture ownership is transitioning"),
    }
}

#[test]
fn ordinary_actor_keeps_control_and_session_while_native_io_returns_after_cancel() {
    exercise_actor(true, false, false, false);
}

#[test]
fn ordinary_actor_commits_native_result_and_returns_original_resources() {
    exercise_actor(false, false, false, false);
}

#[test]
fn resumed_body_keeps_selected_native_inventory_and_commits_source_result() {
    exercise_actor(false, true, false, false);
}

#[test]
fn ordinary_actor_dispatches_selected_http_through_original_native_lease() {
    exercise_actor(false, false, true, false);
}

fn exercise_actor(cancel: bool, resume_body: bool, http: bool, owned_local: bool) {
    exercise_actor_mode(cancel, resume_body, http, owned_local, false)
}
fn exercise_actor_mode(
    cancel: bool,
    resume_body: bool,
    http: bool,
    owned_local: bool,
    cadence: bool,
) {
    exercise_actor_fault(cancel, resume_body, http, owned_local, cadence, 0)
}
fn exercise_actor_fault(
    cancel: bool,
    resume_body: bool,
    http: bool,
    owned_local: bool,
    cadence: bool,
    fault: u8,
) {
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
    let capability = if http {
        &inventory
            .capabilities
            .iter()
            .find(|offer| {
                offer.implementation.implementation_id.as_str()
                    == conduit_std_host::isolated_http_base::ISOLATED_HTTP_IMPLEMENTATION
            })
            .unwrap()
            .capability_id
    } else {
        session.capability()
    };
    let plan = mixed_plan(&host, &peer, capability, &grants, http);
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
    if owned_local {
        if fault != 0 {
            owned_run::exercise_refusal(runtime, device, fault);
        } else if cadence {
            owned_run::exercise_continuing_source(runtime, device);
        } else {
            owned_run::exercise_owned_source(runtime, device, cancel);
        }
        return;
    }
    let body_root = if resume_body {
        let root = std::env::temp_dir().join(format!(
            "conduit-native-resume-{}",
            crate::birth_identity::fresh_uuid().unwrap()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("installation.json"), serde_json::to_vec(&serde_json::json!({
            "schema":"conduit.install/durable-host@1", "host_id":"actor/native-host",
            "release_source_identity":"source/test", "release_bundle_sha256":format!("sha256:{}", "a".repeat(64)),
            "product_executable":"fixture-unused", "body_state":null, "joined_body_state":null
        })).unwrap()).unwrap();
        let prior_host = StdHost::new_with_config(StdHostConfig {
            host_id: "actor/native-host".into(),
            boot_id: "actor/prior-boot".into(),
            offer_generation: OfferGeneration(1),
        });
        let resident = conduit_body::ResidentPlot::new(
            plan.source_document_id.clone(),
            plan.checked_plot_id.clone(),
        );
        let mut owner = crate::durable_host::owner::Owner::open(
            prior_host,
            resident,
            None,
            "Native actor Body",
        )
        .unwrap();
        owner.persist(&root).unwrap();
        drop(owner);
        runtime = runtime.with_owned_body(&root).unwrap();
        assert_eq!(
            runtime.truth().advertisement,
            inventory,
            "ordinary environmental Body resume preserves exact selected inventory"
        );
        Some(root)
    } else {
        None
    };
    let input = if http {
        conduit_web::encode_request(&conduit_web::HttpRequest {
            transaction_id: conduit_web::HttpTransactionId::new(9).unwrap(),
            method: conduit_web::HttpMethod::Get,
            target: conduit_web::HttpTarget::new(
                format!("actor-device.invalid:{}", device.endpoint.port()),
                "/environment".into(),
                conduit_web::HttpScheme::Http,
            )
            .unwrap(),
            headers: Default::default(),
            body: conduit_web::HttpBody::inline([]),
        })
        .unwrap()
    } else {
        Vec::new()
    };
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
        .admit_local_fore_input(&"begin".into(), 0, &input)
        .unwrap();
    runtime
        .remote_fragment
        .as_mut()
        .unwrap()
        .runtime_mut()
        .close_local_fore_input(&"begin".into())
        .unwrap();
    let before_native = device.count.load(Ordering::SeqCst);
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
    if cancel {
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
    }
    // Cancellation cannot release an in-flight provider's original reservation.
    device.release.store(true, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(3);
    while runtime.environmental_worker_active() && Instant::now() < deadline {
        runtime.progress_environmental_worker().unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(!runtime.environmental_worker_active() && !host_is_out(&runtime));
    let completed_counter = device.count.load(Ordering::SeqCst);
    assert!(
        completed_counter > before_native,
        "native operation performs a new sensor request"
    );
    assert!(runtime.pending_native_source.is_none());
    assert_eq!(runtime.truth().advertisement, inventory);
    assert!(!runtime.host.native_resources_idle());
    let admitted = runtime.remote_fragment.as_mut().unwrap();
    assert_eq!(admitted.identity(), &identity);
    if cancel {
        assert!(
            !matches!(
                admitted
                    .runtime_mut()
                    .take_local_fore_output(&"observation".into()),
                Ok(Some(_))
            ),
            "late native bytes must not become an ordinary committed output"
        );
    } else {
        let mut output = None;
        for _ in 0..64 {
            if let Some(value) = admitted
                .runtime_mut()
                .take_local_fore_output(&"observation".into())
                .unwrap()
            {
                output = Some(value);
                break;
            }
            admitted.runtime_mut().step().unwrap();
        }
        let output = output.expect("completed native result reaches the admitted Fore");
        if http {
            let response = conduit_web::decode_response(&output).unwrap();
            assert_eq!(response.status, 200);
        } else {
            let observation =
                conduit_environment_measurement::source_observation::ObservationContract::prepare()
                    .unwrap()
                    .decode(&output)
                    .unwrap();
            assert!(
                matches!(observation.outcome,
            conduit_environment_measurement::source_observation::SourceOutcome::Available(sample)
            if sample.source_counter == completed_counter && sample.reported_source_ms == Some(123)),
                "the committed value retains the real fixture sample and source coordinate"
            );
        }
        assert!(
            admitted
                .runtime_mut()
                .take_local_fore_output(&"observation".into())
                .unwrap()
                .is_none(),
            "the same result is not published twice"
        );
    }
    runtime.release_remote().unwrap();
    assert!(runtime.host.native_resources_idle());
    assert_eq!(runtime.truth().advertisement, inventory);
    assert!(runtime.selected_environmental_equipment.is_some());
    if let Some(root) = body_root {
        fs::remove_dir_all(root).unwrap();
    }
}

#[path = "owned_run_tests.rs"]
mod owned_run;

#[test]
fn owned_body_executes_retained_source_through_typed_fore_and_reuses_equipment() {
    exercise_actor(false, false, false, true);
}

#[test]
fn owned_body_cancels_delayed_source_without_publishing_late_fore() {
    exercise_actor(true, false, false, true);
}

#[test]
fn owned_source_cadence_projects_current_face_then_lull_and_readmission() {
    exercise_actor_mode(false, false, false, true, true);
}

#[test]
fn owned_source_reports_transport_timeout_without_late_fore_or_reuse() {
    exercise_actor_fault(false, false, false, true, false, 1)
}
#[test]
fn owned_source_reports_checked_configuration_mismatch_without_payload() {
    exercise_actor_fault(false, false, false, true, false, 2)
}
#[test]
fn owned_source_reports_semantic_counter_rollback_without_reopening_equipment() {
    exercise_actor_fault(false, false, false, true, false, 3)
}

#[test]
fn owned_source_reports_malformed_observation_without_payload_or_reuse() {
    exercise_actor_fault(false, false, false, true, false, 4)
}
