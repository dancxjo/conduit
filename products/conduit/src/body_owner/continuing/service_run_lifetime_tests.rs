//! Actual installed service dispatch, original worker/IMAGE, no external network.
use super::*;
use conduit_core::*;
use conduit_host_make::*;
use conduit_std_host::{ImmediateSourcePreparationBudget, StdHostConfig};
use std::{
    io::{Read, Write},
    net::TcpListener,
};

fn png(number: u8) -> Vec<u8> {
    fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], bytes: &[u8]) {
        output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        output.extend_from_slice(kind);
        output.extend_from_slice(bytes);
        let mut crc = !0u32;
        for byte in kind.iter().chain(bytes) {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = (crc >> 1) ^ if crc & 1 != 0 { 0xedb88320 } else { 0 };
            }
        }
        output.extend_from_slice(&(!crc).to_be_bytes());
    }
    let raw = [
        0, number, 20, 30, number, 20, 30, 0, number, 20, 30, number, 20, 30,
    ];
    let mut compressed = vec![0x78, 0x01, 0x01, 14, 0, !14u8, 0xff];
    compressed.extend_from_slice(&raw);
    let (mut a, mut b) = (1u32, 0u32);
    for byte in raw {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    compressed.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut bytes = vec![137, 80, 78, 71, 13, 10, 26, 10];
    chunk(
        &mut bytes,
        b"IHDR",
        &[0, 0, 0, 2, 0, 0, 0, 2, 8, 2, 0, 0, 0],
    );
    chunk(&mut bytes, b"IDAT", &compressed);
    chunk(&mut bytes, b"IEND", &[]);
    bytes
}
fn deck() -> Result<conduit_std_host::ambient_wallpaper::acquisition::PreparedDeck, String> {
    use conduit_std_host::ambient_wallpaper::{acquisition::*, MAXIMUM_APOD_RESPONSE_BYTES};
    let admission = PreparationAdmission::admit(
        "2026-11-16".parse().unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap();
    let mut number = 0;
    prepare_deck(admission,|url,bound|if bound==MAXIMUM_APOD_RESPONSE_BYTES {
        Ok(serde_json::to_vec(&serde_json::json!({"date":url.rsplit('=').next().unwrap(),"title":"Service fixture photograph","media_type":"image","hdurl":"https://apod.nasa.gov/fixture.png","copyright":"Fixture photographer"})).unwrap())
    }else{number+=1;Ok(png(number))}).map_err(|e|format!("deck fixture: {e:?}"))
}
fn host() -> (StdHost, JoinHandle<()>) {
    use conduit_std_offers::*;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = listener.local_addr().unwrap();
    listener.set_nonblocking(true).unwrap();
    let http = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(
                        Instant::now() < deadline,
                        "resident service never requested actual weather"
                    );
                    thread::sleep(Duration::from_millis(2));
                }
                Err(error) => panic!("fixture accept: {error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = [0u8; 2048];
        let mut length = 0;
        while !request[..length].windows(4).any(|b| b == b"\r\n\r\n") {
            let n = stream.read(&mut request[length..]).unwrap();
            assert!(n > 0);
            length += n;
        }
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - 1;
        let body = format!(
            r#"{{"current_units":{{"time":"unixtime","temperature_2m":"°C","weather_code":"wmo code"}},"current":{{"time":{seconds},"temperature_2m":12.5,"weather_code":0}}}}"#
        );
        write!(
            stream,
            "HTTP/1.0 200 OK\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let workspace = ambient_home_workspace_bytes().unwrap() * 2;
    let source = format!(
        r#"host service-ambient {{
      schema = 1
      target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
      base = {{kind: "ambient/resident-observation", implementation: "std/resident-ambient-observation-worker@1", initialization: {{timezone: "America/Los_Angeles", weather_endpoint: "{endpoint}", source_pool: "ambient/source", memory_pool: "ambient/memory", memory_bytes: 67108864, preparation_bytes: 58720256, retained_bytes: 16777216}}}}
      base = {{kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}}
      base = {{kind: "compute/cpu", implementation: "hosted/cpu-64@1", initialization: {{pointer_bits: 64, memory_pool: "ambient/cpu", memory_bytes: {workspace}}}}}
      capability = {{kind: "time/every", pinned_implementation: "std/kernel-time-every@1"}}
      capability = {{kind: "ambient/resident-observation", pinned_implementation: "std/resident-ambient-observation-worker@1"}}
      capability = {{kind: "face/resident-ambient-wallpaper", pinned_implementation: "std/resident-ambient-wallpaper@1"}}
      capability = {{kind: "face/resident-ambient-clock-weather", pinned_implementation: "std/resident-ambient-clock-weather@1"}}
      need = {{id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}}
      need = {{id: "ambient/source", class: "conduit.resource/resident-ambient-observation-source@1", slots: 1, bytes: 8192}}
      need = {{id: "ambient/memory", class: "conduit.resource/runtime-memory@1", slots: 67108864, bytes: 67108864}}
      need = {{id: "ambient/cpu", class: "conduit.resource/runtime-memory@1", slots: {workspace}, bytes: {workspace}}}
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
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/resident-service".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let config = StdHostConfig {
        host_id: "host/resident-service".into(),
        boot_id: "boot/resident-service".into(),
        offer_generation: OfferGeneration(1),
    };
    let offer = ambient_resident_source_offer().unwrap();
    let grant = AuthorityGrant {
        grant_id: "fixture/root-resident-ambient".into(),
        contract_id: AMBIENT_RESIDENT_SOURCE_AUTHORITY.into(),
        host_call_contract_id: AMBIENT_RESIDENT_SOURCE_CALL.into(),
        subject_kind: offer.kind_id,
        capability_id: offer.capability_id,
        host_id: config.host_id.clone(),
        boot_id: config.boot_id.clone(),
    };
    // SAFETY: explicit loopback fixture grant and generated finite photos;
    // reviewed actual provider/worker, original IMAGE preparation and cancellation.
    let host = unsafe {
        StdHost::from_selected_ambient_observation_image(
            config,
            &image,
            &catalog,
            &checked,
            ImmediateSourcePreparationBudget {
                preparation_bytes: 1048576,
                retained_bytes: 1048576,
            },
            grant,
            deck,
        )
    }
    .unwrap();
    (host, http)
}
#[test]
fn actual_resident_service_uses_face_runner_until_lull_but_explicit_start_stays_finite() {
    use crate::durable_host::{fresh_identity, write_json_atomic, Installation, INSTALL_SCHEMA};
    let (host, http) = host();
    let root = std::env::temp_dir().join(fresh_identity("resident-service", "dispatch"));
    std::fs::create_dir_all(&root).unwrap();
    let installation = Installation {
        schema: INSTALL_SCHEMA.into(),
        host_id: host.advertisement().host_id.as_str().into(),
        release_source_identity: "fixture/resident-service".into(),
        release_bundle_sha256: crate::durable_host::digest(b"fixture"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let text = include_str!("../../../../../plots/ambient-home/resident.conduit")
        .replace("60000ms", "5ms");
    let source = crate::plot_source::parse(&text).unwrap();
    let plot = source
        .expand_named_for_authoring("ambient-resident")
        .unwrap();
    let resident = conduit_body::ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let mut owner = Owner::open(host, resident, None, "Resident service fixture").unwrap();
    owner.set_resident_plot_name(&plot).unwrap();
    owner.persist(&root).unwrap();
    std::fs::write(root.join("body/source.conduit"), text).unwrap();
    let mut worker = owner.start_service_run_from_wake(&root, 25).unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut play = None;
    let mut both_at = None;
    loop {
        assert!(
            !worker.progress(&mut owner, &root).unwrap(),
            "standing Wake ended before explicit Lull"
        );
        if owner.runtime_face.iter().all(Option::is_some) {
            let current = owner.current_play_id().unwrap().clone();
            if let Some(previous) = &play {
                assert_eq!(previous, &current);
            } else {
                play = Some(current);
            }
            let face = owner.local_face_snapshot().unwrap();
            assert!(face.subjects.iter().any(|s| s.name == "Wallpaper"));
            assert!(face.subjects.iter().any(|s| s.name == "Clock and weather"));
            let began = both_at.get_or_insert_with(Instant::now);
            if began.elapsed() >= Duration::from_millis(100) {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "service did not expose both actual facet routes"
        );
        thread::sleep(Duration::from_millis(2));
    }
    http.join().unwrap();
    worker.request_lull().unwrap();
    while !worker.progress(&mut owner, &root).unwrap() {
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(2));
    }
    assert!(owner.current_play_id().is_none());
    let mut finite = owner.start_service_run(&root, 25).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !finite.progress(&mut owner, &root).unwrap() {
        assert!(
            Instant::now() < deadline,
            "explicit finite start did not reach its deadline"
        );
        thread::sleep(Duration::from_millis(2));
    }
    assert!(owner.current_play_id().is_none());
    assert_eq!(
        owner.last_execution.as_ref().unwrap()["terminal"],
        serde_json::json!(TerminalDisposition::Cancelled {
            reason: CancellationReason::OperatorRequested
        })
    );
    drop(owner);
    std::fs::remove_dir_all(root).unwrap();
}
