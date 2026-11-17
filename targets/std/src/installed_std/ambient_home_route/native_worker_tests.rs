//! Actual native worker, HTTP provider, BodyKernel and two committed Face routes.
use super::*;
use crate::{
    ambient_observation::PreparedAmbientObservation,
    ambient_wallpaper::{MAXIMUM_APOD_RESPONSE_BYTES, acquisition::*},
    *,
};
use conduit_core::*;
use conduit_host_make::*;
use conduit_std_offers::*;
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
fn deck() -> PreparedDeck {
    let admission = PreparationAdmission::admit(
        chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap();
    let mut number = 0;
    prepare_deck(admission,|url,bound|if bound==MAXIMUM_APOD_RESPONSE_BYTES {
        Ok(serde_json::to_vec(&serde_json::json!({"date":url.rsplit('=').next().unwrap(),"title":"Prepared worker photograph","copyright":"Fixture photographer","media_type":"image","hdurl":"https://apod.nasa.gov/fixture.png"})).unwrap())
    }else{number+=1;let mut out=Vec::new();PngEncoder::new(&mut out).write_image(&[number,2,3,255],1,1,ExtendedColorType::Rgba8).unwrap();Ok(out)}).unwrap()
}
fn weather_server() -> (
    SocketAddr,
    thread::JoinHandle<()>,
    Arc<AtomicUsize>,
    SyncSender<()>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = calls.clone();
    let (start, begun) = sync_channel(1);
    let server = thread::spawn(move || {
        // Preparation owns no network deadline. An actual Start releases this
        // finite gate; a failed parent drops its sender and retires the server.
        if begun.recv().is_err() {
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "native worker never requested weather"
                    );
                    thread::sleep(Duration::from_millis(1));
                }
                Err(e) => panic!("fixture accept: {e}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0u8; 1536];
        let mut len = 0;
        while !request[..len].windows(4).any(|s| s == b"\r\n\r\n") {
            assert!(len < request.len());
            let n = stream.read(&mut request[len..]).unwrap();
            assert_ne!(n, 0);
            len += n;
        }
        assert!(request[..len].starts_with(b"GET /v1/forecast?"));
        counted.fetch_add(1, Ordering::AcqRel);
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 1;
        let body = format!(
            r#"{{"current_units":{{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"}},"current":{{"time":{seconds},"temperature_2m":12.5,"weather_code":0}}}}"#
        );
        write!(
            stream,
            "HTTP/1.0 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    (address, server, calls, start)
}
fn native_host(endpoint: SocketAddr) -> StdHost {
    let cpu_bytes = crate::ambient_home_offer::workspace_bytes().unwrap() * 2;
    let source = format!(
        r#"host ambient-native-body {{
      schema = 1
      target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
      base = {{kind: "ambient/observation", implementation: "std/ambient-observation-worker@1", initialization: {{timezone: "America/Los_Angeles", maximum_updates: 100, weather_endpoint: "{endpoint}", source_pool: "ambient/source", memory_pool: "ambient/memory", memory_bytes: 67108864, preparation_bytes: 58720256, retained_bytes: 16777216}}}}
      base = {{kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}}
      base = {{kind: "compute/cpu", implementation: "hosted/cpu-64@1", initialization: {{pointer_bits: 64, memory_pool: "cpu/memory", memory_bytes: {cpu_bytes}}}}}
      capability = {{kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}}
      capability = {{kind: "ambient/home-observation", pinned_implementation: "std/ambient-observation-worker@1"}}
      capability = {{kind: "face/ambient-wallpaper", pinned_implementation: "std/ambient-wallpaper@1"}}
      capability = {{kind: "face/ambient-clock-weather", pinned_implementation: "std/ambient-clock-weather@1"}}
      need = {{id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}}
      need = {{id: "ambient/source", class: "conduit.resource/ambient-observation-source@1", slots: 1, bytes: 8192}}
      need = {{id: "ambient/memory", class: "conduit.resource/runtime-memory@1", slots: 67108864, bytes: 67108864}}
      need = {{id: "cpu/memory", class: "conduit.resource/runtime-memory@1", slots: {cpu_bytes}, bytes: {cpu_bytes}}}
      limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 134217728, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}
    }}"#
    );
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = check_host_configuration(
        parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/native-worker-two-facets".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let selected = image
        .manifest
        .base_selections
        .iter()
        .find(|b| b.kind == AMBIENT_SOURCE_BASE_KIND)
        .unwrap()
        .clone();
    let config = StdHostConfig {
        host_id: "fixture/ambient-worker-body".into(),
        boot_id: "fixture/worker-boot".into(),
        offer_generation: OfferGeneration(1),
    };
    let host_id = config.host_id.clone();
    let boot_id = config.boot_id.clone();
    unsafe {
        StdHost::from_selected_timer_and_worker_source_factory_image(
            config,
            &image,
            &catalog,
            &checked,
            ImmediateSourcePreparationBudget {
                preparation_bytes: 1048576,
                retained_bytes: 1048576,
            },
            None,
            None,
            move |_, _| {
                let kind = conduit_semantic_catalog::ambient_observe_kind()?;
                let offer = ambient_source_offer()?;
                // Current fixture authority is independently supplied, never
                // extracted from the selected IMAGE, offer, observation or Plan.
                let grant = AuthorityGrant {
                    grant_id: "fixture/independent-worker-read".into(),
                    contract_id: AMBIENT_SOURCE_AUTHORITY.into(),
                    host_call_contract_id: AMBIENT_SOURCE_CALL.into(),
                    subject_kind: kind.kind_id.clone(),
                    host_id,
                    boot_id,
                    capability_id: offer.capability_id.clone(),
                };
                let definition = NativeSourceDefinition {
                    kind,
                    grant,
                    base: BaseProviderEntry {
                        base_id: selected.id.as_str().into(),
                        provider_instance_id: "fixture/actual-worker-provider".into(),
                        provider_generation: 1,
                        implementation_id: AMBIENT_SOURCE_IMPLEMENTATION.into(),
                        mechanism_family: AMBIENT_SOURCE_BASE_KIND.into(),
                        enforcement_class: BaseEnforcementClass::Cooperative,
                        lifecycle: BaseLifecycle::Ready,
                        capabilities: vec![offer],
                        resources: vec![
                            ResourceOffer {
                                pool_id: "ambient/source".into(),
                                class_id: AMBIENT_SOURCE_RESOURCE.into(),
                                capacity_units: 1,
                                compute: None,
                                content: None,
                            },
                            ResourceOffer {
                                pool_id: "ambient/memory".into(),
                                class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
                                capacity_units: 67108864,
                                compute: None,
                                content: None,
                            },
                        ],
                    },
                    resource_generation: ResourceGenerationId(
                        "fixture/ambient-worker-generation@1".into(),
                    ),
                    configuration: vec![
                        ConfigurationEntry {
                            key: "timezone".into(),
                            value: ConfigurationValue::Text("America/Los_Angeles".into()),
                        },
                        ConfigurationEntry {
                            key: "maximum-updates".into(),
                            value: ConfigurationValue::U64(100),
                        },
                    ],
                    input_bytes: 8,
                    output_bytes: 8192,
                    resource_bytes: 8192,
                };
                Ok((
                    definition,
                    ImmediateSourcePreparationBudget {
                        preparation_bytes: AMBIENT_SOURCE_PROVIDER_PREPARATION_BYTES,
                        retained_bytes: 16 * 1024 * 1024,
                    },
                    NativeWorkerBudget {
                        requested_stack_bytes: 2 * 1024 * 1024,
                        maximum_stack_mapping_bytes: 2 * 1024 * 1024 + 64 * 1024,
                        capture_timeout: Duration::from_secs(3),
                        cancellation_timeout: Duration::from_secs(1),
                    },
                    move || {
                        Ok(Box::new(PreparedAmbientObservation::prepare(
                            "America/Los_Angeles",
                            endpoint,
                            deck(),
                            100,
                        )?)
                            as Box<dyn NativeSourceWorkerProvider>)
                    },
                ))
            },
        )
    }
    .unwrap()
}
struct Both {
    frames: [Option<CommittedFaceContribution>; 2],
    counts: [usize; 2],
    control: RunControl,
    stopped: bool,
}
impl BodyFaceContributionObserver for Both {
    fn contribute(
        &mut self,
        frame: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution> {
        let index = match frame.fragment().subjects[0].name.as_str() {
            "Wallpaper" => 0,
            "Clock and weather" => 1,
            _ => panic!("unexpected Face route"),
        };
        if let Some(old) = &self.frames[index] {
            assert_eq!(old.fragment().basis, frame.fragment().basis);
            assert_eq!(old.fragment().subjects, frame.fragment().subjects);
        }
        assert!(!frame.fragment().subjects.iter().any(|s| s.name == "Home"));
        self.frames[index] = Some(frame);
        self.counts[index] += 1;
        if !self.stopped && self.counts.iter().all(|n| *n >= 2) {
            self.stopped = true;
            self.control
                .request_stop(RunControlRequestId::new("worker/two-observations-stop").unwrap())
                .unwrap();
        }
        Ok(())
    }
}
#[test]
fn original_worker_http_observations_commit_two_facets_in_one_cancellable_body_play() {
    let (endpoint, server, calls, start) = weather_server();
    let mut start = Some(start);
    let mut host = native_host(endpoint);
    let (plan, wake) = common::planned_aspects(&host, 100, true);
    let memory = host.kernel_resources.image_memory_observer();
    let baseline = memory.load(Ordering::Acquire);
    let starts = Cell::new(0);
    let control = RunControl::default();
    let mut both = Both {
        frames: [None, None],
        counts: [0, 0],
        control: control.clone(),
        stopped: false,
    };
    let result = host
        .run_selected_image_body_plan_with_face_to_with_start(
            BodyRunRequest {
                wake: &wake,
                plan: &plan,
                control: &control,
                keyboard: None,
            },
            &[],
            false,
            &mut NoOutput,
            &mut both,
            &mut Vec::new(),
            &mut ThreadTimer,
            |_, _| {
                starts.set(starts.get() + 1);
                start
                    .take()
                    .expect("one actual Start")
                    .send(())
                    .expect("fixture server awaits actual Start");
                Ok(())
            },
        )
        .unwrap();
    assert!(
        result.report().failure.is_none(),
        "{:?}",
        result.report().failure
    );
    assert!(matches!(
        result.report().terminal,
        TerminalDisposition::Cancelled { .. }
    ));
    assert_eq!(starts.get(), 1);
    assert!(both.counts.iter().all(|n| *n >= 2));
    let wallpaper = both.frames[0].as_ref().unwrap().fragment();
    let clock = both.frames[1].as_ref().unwrap().fragment();
    assert_eq!(wallpaper.basis, clock.basis);
    assert_ne!(wallpaper.subjects[0].identity, clock.subjects[0].identity);
    let content = wallpaper
        .properties
        .iter()
        .find(|p| p.name == "image-content")
        .unwrap();
    let conduit_face::FacePropertyValue::Content(bytes) = &content.value else {
        panic!("image content reference")
    };
    let reference = BoundedResourceRef::validate_encoded(bytes).unwrap();
    assert_eq!(reference.content_profile, "media/image-rgba8@1");
    assert_eq!(reference.extent.bytes, 4);
    assert!(clock.properties.iter().any(|p| p.name == "availability"
        && p.value == conduit_face::FacePropertyValue::Text("fresh".into())));
    assert!(
        clock
            .properties
            .iter()
            .any(|p| p.name == "temperature-celsius-milli"
                && p.value == conduit_face::FacePropertyValue::Signed(12500))
    );
    server.join().unwrap();
    assert_eq!(calls.load(Ordering::Acquire), 1);
    drop(result);
    drop(both);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
    let inspection = host.native_source_worker_inspection().unwrap();
    assert!(!inspection.cancellation_overdue);
    assert!(inspection.idle_backing_is_reserved);
    // The existing native preparation boundary admits another exact child
    // only when the real worker has no running request or retained reply.
    let fragment = plan
        .plots
        .iter()
        .flat_map(|p| &p.plan.fragments)
        .find(|f| {
            f.placements
                .iter()
                .any(|g| g.kind_id.as_str() == "ambient/home-observation")
        })
        .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
    let prospective = conduit_body::BodyPlayIdentity::bind(&plan, host.next_kernel_play_sequence);
    let ready = host
        .native_source
        .as_mut()
        .unwrap()
        .prepare_body(
            &host.base_registry,
            &host.advertisement,
            fragment,
            &lowered,
            (&wake, &plan, &prospective),
        )
        .unwrap()
        .expect("actual worker must be ready and quiescent after Body cancellation");
    drop(ready);
    assert_eq!(memory.load(Ordering::Acquire), baseline);
    drop(host);
    assert_eq!(
        memory.load(Ordering::Acquire),
        crate::kernel_preparation::StagedImageMemory::counter_backing_bytes()
    );
}
