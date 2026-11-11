#[path = "../../../targets/std/tests/common_episode_loop/mod.rs"]
mod common;
#[path = "../src/commands/pete_replay.rs"]
mod pete_replay;

use common::{experience_episode_harness, inputs, records, run, terminal, Collector};
use conduit_core::{ConnectionTrack, PortDirection, TerminalDisposition};
use conduit_human::ExperienceTemporalPolicy;
use conduit_pete::{
    PeteCognitionBattery, PeteCognitionBatteryInput, PeteCognitionFrame, PeteCognitionSafety,
    PeteCognitionSign, PeteCognitionUtterance,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{ExternalForeInput, HostedFrameBack, RunControl, StdHostComposition};
use conduit_time::{NativeTemporalInstant, NativeTemporalScale};

const PROGRAM: &str = include_str!("../../../plots/experience-episode-loop/main.conduit");

const PETE_PROGRAM: &str = include_str!("../../../plots/pete-cognition/main.conduit");

fn pete_harness(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
) -> common::Harness {
    pete_harness_for(model, PETE_PROGRAM, "pete-cognition", None)
}

fn pete_harness_for(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
    program: &str,
    entry: &str,
    checkpoint: Option<&[u8]>,
) -> common::Harness {
    pete_harness_with_transform(
        model,
        program,
        entry,
        checkpoint,
        conduit_pete::prepare_pete_experience_input_bytes,
    )
}

fn pete_harness_with_transform(
    model: Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>,
    program: &str,
    entry: &str,
    checkpoint: Option<&[u8]>,
    transform: fn(&[u8]) -> Result<Vec<u8>, u16>,
) -> common::Harness {
    let config = conduit_std_host::StdHostConfig {
        host_id: "host/pete-cognition".into(),
        boot_id: if checkpoint.is_some_and(|bytes| bytes != conduit_chat::initial_episode_memory())
        {
            "boot/pete-cognition-replacement".into()
        } else {
            "boot/pete-cognition".into()
        },
        offer_generation: conduit_core::OfferGeneration(1),
    };
    let back = HostedFrameBack::new(
        conduit_pete::pete_cognition_inputs_offer(),
        conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES as u32,
        transform,
    )
    .unwrap();
    let composition = StdHostComposition::minimal();
    let host = match checkpoint {
        Some(checkpoint) => {
            conduit_std_host::StdHost::new_with_local_model_frame_and_episode_checkpoint(
                config,
                composition,
                model,
                back,
                checkpoint,
            )
        }
        None => conduit_std_host::StdHost::new_with_local_model_and_frame_back(
            config,
            composition,
            model,
            back,
        ),
    }
    .unwrap();
    let mut ports = vec![
        ("context", PortDirection::Input, 32_768),
        (
            "frames",
            PortDirection::Input,
            conduit_pete::MAXIMUM_PETE_COGNITION_FRAME_BYTES as u32,
        ),
        ("episodes", PortDirection::Output, 32_768),
    ];
    if checkpoint.is_some() {
        ports.push((
            "checkpoints",
            PortDirection::Output,
            conduit_chat::MAXIMUM_EPISODE_CHECKPOINT_BYTES as u32,
        ));
    }
    common::plan_host(host, program, entry, &ports, false, |startup, profile| {
        conduit_pete::install_pete_cognition_catalog(startup, profile)?;
        if let Some(initial) = checkpoint {
            conduit_chat::install_episode_resumable_memory_catalog(initial, startup, profile)?;
        }
        Ok(())
    })
}

fn at(ticks: u64) -> NativeTemporalInstant {
    NativeTemporalInstant::new(
        "clock/pete-episodes".into(),
        1,
        NativeTemporalScale::Milliseconds,
        ticks,
        0,
    )
    .unwrap()
}

fn frame(revision: u64, message: &str, retain: bool) -> PeteCognitionFrame {
    let now = at(100 * revision);
    let sign = |kind| PeteCognitionSign::new(format!("{kind}/{revision}")).unwrap();
    PeteCognitionFrame::new(
        PeteCognitionBattery::new(90, false, 13_000, now.clone(), sign("battery"))
            .unwrap()
            .try_into()
            .unwrap(),
        Some(PeteCognitionSafety::new(false, now.clone(), true, sign("body")).unwrap()),
        "pete/wake/2".into(),
        now.clone(),
        revision,
        ExperienceTemporalPolicy::new(10, 20).unwrap(),
        PeteCognitionUtterance::new(message.into(), now, retain, sign("utterance")).unwrap(),
    )
    .unwrap()
}

fn create_battery_stream(revision: u64, capacity_mah: u16) -> Vec<u8> {
    use conduit_pete::{
        CREATE_CHARGING_SOURCES_PACKET_ID, CREATE_GROUP_ZERO_PACKET_ID, STREAM_HEADER,
    };
    let mut packet = [0u8; 26];
    let charge_mah: u16 = if revision == 1 { 500 } else { 90 };
    packet[16] = if revision == 1 { 3 } else { 0 };
    packet[17..19].copy_from_slice(&13_000u16.to_be_bytes());
    packet[22..24].copy_from_slice(&charge_mah.to_be_bytes());
    packet[24..26].copy_from_slice(&capacity_mah.to_be_bytes());
    let mut stream = vec![STREAM_HEADER, 29, CREATE_GROUP_ZERO_PACKET_ID];
    stream.extend(packet);
    stream.extend([CREATE_CHARGING_SOURCES_PACKET_ID, 2]);
    let checksum = stream.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte));
    stream.push(0u8.wrapping_sub(checksum));
    stream
}

fn create_frame(revision: u64, message: &str, retain: bool) -> PeteCognitionFrame {
    use conduit_pete::{
        decode_observation_bundle, lower_charging_sources, lower_group_zero,
        pete_cognition_battery_from_create, CreatePortableObservation,
    };

    let bundle = decode_observation_bundle(&create_battery_stream(revision, 1_000)).unwrap();
    let observation = CreatePortableObservation {
        group_zero: lower_group_zero(&bundle.group_zero).unwrap(),
        charging_sources: lower_charging_sources(&bundle.charging_sources).unwrap(),
    };
    let fixture = frame(revision, message, retain);
    let battery = pete_cognition_battery_from_create(
        &observation,
        PeteCognitionSign::new(format!("create/battery/{revision}")).unwrap(),
        at(100 * revision - 1),
    )
    .unwrap()
    .unwrap();
    PeteCognitionFrame::new(
        battery.try_into().unwrap(),
        fixture.body().clone(),
        fixture.provenance_revision().clone(),
        fixture.reference().clone(),
        revision,
        *fixture.temporal_policy(),
        fixture.utterance().clone(),
    )
    .unwrap()
}

#[cfg(target_os = "linux")]
#[test]
fn pete_create_serial_samples_enter_one_cognition_play() {
    assert_pete_create_serial_play(SerialSample::Available);
}

#[cfg(target_os = "linux")]
#[test]
fn pete_continues_when_create_capacity_is_unavailable() {
    assert_pete_create_serial_play(SerialSample::MissingCapacity);
}

#[cfg(target_os = "linux")]
#[test]
fn pete_continues_after_a_create_read_timeout() {
    assert_pete_create_serial_play(SerialSample::Timeout);
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum SerialSample {
    Available,
    MissingCapacity,
    Timeout,
}

#[cfg(target_os = "linux")]
fn assert_pete_create_serial_play(first_sample: SerialSample) {
    let normal = first_sample == SerialSample::Available;
    use conduit_pete::{capture_pete_cognition_battery, UartProfile};
    use conduit_std_host::std_create_uart::{
        monotonic_millis, StdCreateUartBase, StdCreateUartObservation,
    };
    use std::{
        ffi::CStr,
        fs::File,
        io::{Read, Write},
        os::fd::FromRawFd,
        path::PathBuf,
    };

    let (device, master, slave) = match std::env::var_os("CONDUIT_PETE_CREATE_SERIAL")
        .filter(|_| normal)
    {
        Some(path) => (PathBuf::from(path), None, None),
        None => {
            let mut master = -1;
            let mut slave = -1;
            assert_eq!(
                unsafe {
                    libc::openpty(
                        &mut master,
                        &mut slave,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                },
                0
            );
            let (master, slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
            let mut path = [0; 256];
            use std::os::fd::AsRawFd;
            assert_eq!(
                unsafe { libc::ttyname_r(slave.as_raw_fd(), path.as_mut_ptr(), path.len()) },
                0
            );
            let device = unsafe { CStr::from_ptr(path.as_ptr()) }
                .to_str()
                .unwrap()
                .into();
            (device, Some(master), Some(slave))
        }
    };
    let mut provider = StdCreateUartBase::open(StdCreateUartObservation {
        base_id: "pete/cognition-create".into(),
        device_path: device,
        profile: UartProfile::CREATE_OI,
        maximum_write_wait_ms: 250,
    })
    .unwrap();
    drop(slave);
    let peer = master.map(|mut master| {
        std::thread::spawn(move || {
            for revision in 1..=2 {
                let mut commands = [0; 6];
                master.read_exact(&mut commands).unwrap();
                assert_eq!(commands, [128, 131, 148, 2, 0, 34]);
                let capacity = if first_sample == SerialSample::MissingCapacity && revision == 1 {
                    0
                } else {
                    1_000
                };
                if first_sample != SerialSample::Timeout || revision != 1 {
                    master
                        .write_all(&create_battery_stream(revision, capacity))
                        .unwrap();
                }
                let mut pause = [0; 2];
                master.read_exact(&mut pause).unwrap();
                assert_eq!(pause, [150, 0]);
            }
            master
        })
    });
    use conduit_std_host::{
        BodyLiveForeAdmission, BodyLiveForeQueue, ExternalForeDelivery, ExternalForeOutputAdapter,
    };
    struct Output {
        episodes: Collector,
        first: Option<std::sync::mpsc::Sender<()>>,
    }
    impl ExternalForeOutputAdapter for Output {
        fn deliver(&mut self, delivery: ExternalForeDelivery) -> Result<(), String> {
            self.episodes.deliver(delivery)?;
            if let Some(first) = self.first.take() {
                first.send(()).map_err(|error| error.to_string())?;
            }
            Ok(())
        }
    }
    struct CloseOnDrop(BodyLiveForeQueue);
    impl Drop for CloseOnDrop {
        fn drop(&mut self) {
            let _ = self.0.close();
        }
    }

    let use_ollama = normal && std::env::var_os("CONDUIT_PETE_SERIAL_OLLAMA").is_some();
    let model = if use_ollama {
        ollama_episode_model()
    } else {
        common::fenced_model().0
    };
    let mut harness = pete_harness(model);
    let control = RunControl::default().with_debug_body(common::context().body_id);
    let seed = inputs(&[]).pop().unwrap();
    let queue = BodyLiveForeQueue::for_local_flow_fragment(
        &harness.fragment,
        "frames",
        seed,
        control.clone(),
        1,
        2,
    )
    .unwrap();
    let producer_queue = queue.clone();
    let (first, first_episode) = std::sync::mpsc::channel();
    let producer = std::thread::spawn(move || {
        let _close = CloseOnDrop(producer_queue.clone());
        producer_queue.wait_until_play_started().unwrap();
        let mut capture_ticks = Vec::new();
        for (index, message) in [
            "The hallway switch label is COPPER KITE.",
            "What was the switch label?",
        ]
        .into_iter()
        .enumerate()
        {
            let revision = (index + 1) as u64;
            let sign = |kind| PeteCognitionSign::new(format!("serial/{kind}/{revision}")).unwrap();
            let capture = capture_pete_cognition_battery(
                &mut provider,
                sign("battery"),
                "clock/pete-create-proof".into(),
                if first_sample == SerialSample::Timeout && revision == 1 {
                    50
                } else {
                    1_000
                },
                monotonic_millis,
            );
            let (battery, tick) = match capture {
                Ok(Some(battery)) => {
                    let tick = *battery.observed_at().ticks();
                    (battery.try_into().unwrap(), Some(tick))
                }
                Ok(None) => (PeteCognitionBatteryInput::Missing, None),
                Err(conduit_pete::CreateCognitionCaptureError::Read {
                    pause_failure: None,
                    ..
                }) => (
                    PeteCognitionBatteryInput::unavailable(provider.identity().base_id.clone())
                        .unwrap(),
                    None,
                ),
                Err(error) => panic!("capture Create battery: {error:?}"),
            };
            let now = NativeTemporalInstant::new(
                "clock/pete-create-proof".into(),
                1,
                NativeTemporalScale::Milliseconds,
                monotonic_millis().unwrap(),
                0,
            )
            .unwrap();
            capture_ticks.push(tick);
            let input = frame_input(
                PeteCognitionFrame::new(
                    battery,
                    None,
                    "pete/create-proof".into(),
                    now.clone(),
                    revision,
                    ExperienceTemporalPolicy::new(10, 20).unwrap(),
                    PeteCognitionUtterance::new(
                        message.into(),
                        now,
                        revision == 1,
                        sign("utterance"),
                    )
                    .unwrap(),
                )
                .unwrap(),
            );
            assert_eq!(
                producer_queue.submit(&input.bytes).unwrap(),
                BodyLiveForeAdmission::Accepted {
                    sequence: index as u64
                }
            );
            if index == 0 {
                first_episode
                    .recv_timeout(std::time::Duration::from_secs(if use_ollama {
                        90
                    } else {
                        5
                    }))
                    .unwrap();
            }
        }

        if let Some(peer) = peer {
            peer.join().unwrap();
        }
        capture_ticks
    });
    let mut output = Output {
        episodes: Collector::default(),
        first: Some(first),
    };
    let result = harness.host.run_external_plot_live_to(
        harness.fragment.clone(),
        &queue,
        &mut output,
        &mut Vec::new(),
        &mut common::NoopTimer,
        &control,
    );
    let capture_ticks = producer.join().unwrap();
    if !normal {
        assert!(capture_ticks[0].is_none());
        assert!(capture_ticks[1].is_some());
    }
    let report = result.unwrap();
    let collector = output.episodes;
    let status = queue.status();
    assert_eq!(status.kernel_admitted, 2);
    assert!(status.kernel_closed && status.play_terminal);
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episodes = records(&collector.0);
    assert_eq!(episodes.len(), 2);
    assert_eq!(
        episodes[1].recalled().get()[0].observation(),
        episodes[0].observation()
    );
    assert_eq!(episodes[0].body_id(), episodes[1].body_id());
    assert_eq!(episodes[0].wake_id(), episodes[1].wake_id());
    let response = episodes[1].response().unwrap().content();
    assert!(
        response.to_ascii_uppercase().contains("COPPER KITE"),
        "{response}"
    );
    for (index, episode) in episodes.iter().enumerate() {
        let recorded = common::model_projection::expanded_model_evidence(
            &serde_json::from_str(episode.evidence().as_ref().unwrap()).unwrap(),
            episode.observation().content(),
        );
        let evidence = recorded["evidence"].as_array().unwrap();
        let battery = evidence.iter().find(|item| {
            item["identity"]
                .as_str()
                .is_some_and(|id| id.starts_with("pete/battery/"))
                && item["classifications"]["availability"] == "present"
        });
        assert_eq!(
            battery.and_then(|item| item["observed_at"]["ticks"].as_u64()),
            capture_ticks[index]
        );
        if first_sample == SerialSample::Timeout && index == 0 {
            let unavailable = evidence
                .iter()
                .find(|item| item["identity"] == "pete/battery/unavailable")
                .unwrap();
            assert_eq!(
                unavailable["classifications"]["availability"],
                "source_unavailable"
            );
            assert_eq!(
                unavailable["sources"],
                serde_json::json!([{"source":"pete/cognition-create"}])
            );
        }
        let homeostasis = evidence
            .iter()
            .find(|item| item["identity"] == "pete/homeostasis")
            .unwrap();
        let state: serde_json::Value =
            serde_json::from_str(homeostasis["utf8_content"].as_str().unwrap()).unwrap();
        assert_eq!(state["motion"], "unknown");
        if capture_ticks[index].is_some() {
            assert_eq!(
                homeostasis["sources"],
                serde_json::json!([{"sign": format!("serial/battery/{}", index + 1)}])
            );
            assert!(state["charge_percent"].is_string());
        } else {
            assert_eq!(homeostasis["sources"], serde_json::json!([]));
            assert!(state["charge_percent"].is_null());
            assert_eq!(state["energy"], "unknown");
        }
    }
    let snapshot = assert_pete_replay(&harness, &report, 2);
    if let Some(path) = std::env::var_os("CONDUIT_PETE_REPLAY_SNAPSHOT").filter(|_| normal) {
        std::fs::write(path, snapshot.encode().unwrap()).unwrap();
    }
}

fn frame_input(frame: PeteCognitionFrame) -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: "frames".into(),
        track: ConnectionTrack::Payload,
        bytes: frame.encode().unwrap(),
    }
}

fn episode(revision: u64, message: &str, retain: bool) -> ExternalForeInput {
    ExternalForeInput {
        front_port_id: "experiences".into(),
        track: ConnectionTrack::Payload,
        bytes: conduit_pete::situate_pete_frame(&frame(revision, message, retain))
            .unwrap()
            .encode()
            .unwrap(),
    }
}

#[test]
fn pete_situation_evidence_and_episode_recall_reach_one_model_turn() {
    assert_recall(
        experience_episode_harness(PROGRAM),
        [
            episode(1, "The switch label is COPPER KITE.", true),
            episode(2, "What was the switch label?", false),
        ],
    );
}

#[test]
fn pete_frames_reach_situation_and_recall_in_one_play() {
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness(model);
    harness.requests = requests;
    assert!(harness.fragment.placements.iter().any(|gear| {
        gear.kind_id.as_str() == conduit_pete::PETE_COGNITION_INPUTS_KIND
            && gear.implementation_id.as_str() == "std/pete-situate-experience-inputs@1"
    }));
    assert_recall(
        harness,
        [
            frame_input(frame(1, "The switch label is COPPER KITE.", true)),
            frame_input(frame(2, "What was the switch label?", false)),
        ],
    );
}

#[test]
fn patchbay_replays_pete_cognition_cords_from_the_installed_play() {
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness(model);
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(create_frame(1, "The switch label is COPPER KITE.", true)),
        frame_input(create_frame(2, "What was the switch label?", false)),
    ]);
    let mut collector = Collector::default();
    let control = RunControl::default().with_debug_body(common::context().body_id);
    let report = run(&mut harness, &incoming, &mut collector, &control).unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episodes = records(&collector.0);
    assert_eq!(episodes.len(), 2);
    assert_eq!(
        episodes[1].recalled().get()[0].observation(),
        episodes[0].observation()
    );
    assert!(episodes[1]
        .response()
        .unwrap()
        .content()
        .contains("COPPER KITE"));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for (index, request) in requests.iter().enumerate() {
        let request: serde_json::Value = common::request_data(request);
        let recorded: serde_json::Value =
            serde_json::from_str(episodes[index].evidence().as_ref().unwrap()).unwrap();
        assert_eq!(recorded, request["current_experience"]);
        let expanded = common::model_projection::expanded_model_evidence(
            &request["current_experience"],
            request["observation"]["content"].as_str().unwrap(),
        );
        let evidence = &expanded["evidence"];
        let revision = index + 1;
        assert_eq!(
            evidence[0]["sources"][0]["sign"],
            format!("create/battery/{revision}")
        );
        assert_eq!(evidence[0]["observed_at"]["ticks"], 100 * revision - 1);
        let homeostasis: serde_json::Value =
            serde_json::from_str(evidence[3]["utf8_content"].as_str().unwrap()).unwrap();
        assert_eq!(homeostasis["charge_percent"], ["50.0%", "9.0%"][index]);
        assert_eq!(homeostasis["charging"], revision == 1);
    }
    let second: serde_json::Value = common::request_data(&requests[1]);
    let historical = &second["recalled"][0]["historical_evidence"];
    assert_eq!(historical["original_provenance"]["revision"], 1);
    assert_eq!(
        historical["evidence"][0]["sources"][0]["sign"],
        "utterance/1"
    );
    let recorded: serde_json::Value =
        serde_json::from_str(episodes[1].recalled().get()[0].evidence().as_ref().unwrap()).unwrap();
    assert_eq!(recorded, second["recalled"][0]);
    assert_pete_replay(&harness, &report, 2);
}

fn assert_pete_replay(
    harness: &common::Harness,
    report: &conduit_std_host::StdRunReport,
    expected_frames: usize,
) -> conduit_browser_patchbay_workbench::RendererSnapshot {
    let snapshot = pete_replay::snapshot(
        &harness.fragment,
        &harness.authoring,
        &common::context(),
        report,
    )
    .unwrap();
    let timeline = snapshot.timeline.as_ref().unwrap();
    if expected_frames > 2 {
        // Continuous history remains bounded. A longer fault exercise must
        // disclose eviction rather than claim an exact trace beyond its horizon.
        assert!(timeline.gap.is_some());
        assert!(
            timeline.evicted_events > 0
                || !timeline
                    .trace
                    .as_ref()
                    .unwrap()
                    .missing_parent_sequences
                    .is_empty()
        );
        return snapshot;
    }
    assert!(timeline.gap.is_none());
    for cord in snapshot
        .face
        .subjects
        .iter()
        .filter(|subject| subject.role == conduit_face::FaceRole::Cord)
    {
        assert!(
            timeline.events.iter().any(|event| {
                event.subject == cord.identity
                    && event
                        .value
                        .as_ref()
                        .is_some_and(|value| value.total_bytes > 0)
            }),
            "missing traffic on {}",
            cord.identity
        );
    }

    let response = timeline
        .events
        .iter()
        .rposition(|event| {
            event.event == "value-sent"
                && event.subject.ends_with("/front/output/episodes")
                && event
                    .value
                    .as_ref()
                    .is_some_and(|value| value.total_bytes > 0)
        })
        .unwrap();
    let trace = timeline.trace.as_ref().unwrap();
    assert!(trace.missing_parent_sequences.is_empty());
    let mut steps = trace.steps.iter();
    for (event, port) in [
        ("value-received", "/contextualize/input/capture"),
        ("value-received", "/contextualize/input/outcome"),
        ("value-received", "/model/input/request"),
        ("value-received", "/retain/input/response"),
        ("value-received", "/memory/input/update"),
        ("value-received", "/recall/input/memory"),
        ("value-received", "/contextualize/input/capture"),
        ("value-received", "/contextualize/input/outcome"),
        ("value-received", "/model/input/request"),
        ("value-received", "/retain/input/response"),
        ("value-sent", "/front/output/episodes"),
    ] {
        assert!(
            steps
                .by_ref()
                .any(|step| step.event == event && step.subject.ends_with(port)),
            "missing {event} on {port} in {trace:?}"
        );
    }

    for port in [
        "/experience/input/inputs",
        "/recall/input/experience",
        "/retain/input/pending",
    ] {
        let input = timeline.events[..response]
            .iter()
            .rposition(|event| event.event == "value-received" && event.subject.ends_with(port))
            .unwrap();
        assert!(
            trace.steps.iter().any(|step| step.event_index == input),
            "missing final episode input {port} in {trace:?}"
        );
    }
    assert_eq!(
        trace
            .steps
            .iter()
            .filter(|step| {
                step.event == "value-received" && step.subject.contains("front/input/frames")
            })
            .count(),
        expected_frames
    );
    let bytes = snapshot.encode().unwrap();
    conduit_browser_patchbay_workbench::RendererSnapshot::decode(&bytes, 1).unwrap();
    snapshot
}

#[test]
fn future_frame_is_refused_before_model_admission() {
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness(model);
    let frame = frame(1, "What is the label?", true);
    let future = PeteCognitionFrame::new(
        frame.battery().clone(),
        frame.body().clone(),
        frame.provenance_revision().clone(),
        at(99),
        *frame.revision(),
        *frame.temporal_policy(),
        frame.utterance().clone(),
    )
    .unwrap();
    let mut incoming = inputs(&[]);
    incoming.push(frame_input(future));
    let mut collector = Collector::default();
    let failure = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .expect_err("future observations must refuse");
    assert!(
        failure.contains("pete-cognition/situate") && failure.contains("detail: 3"),
        "{failure}"
    );
    assert!(collector.0.is_empty());
    assert!(requests.lock().unwrap().is_empty());
}

fn assert_recall(mut harness: common::Harness, episodes: [ExternalForeInput; 2]) {
    let mut incoming = inputs(&[]);
    incoming.extend(episodes);
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episodes = records(&collector.0);
    let [first, second] = episodes.as_slice() else {
        panic!("expected two episodes");
    };
    assert_eq!(first.body_id(), second.body_id());
    assert_eq!(first.wake_id(), second.wake_id());
    assert_eq!(
        second.recalled().get()[0].observation(),
        first.observation()
    );
    assert!(second.response().unwrap().content().contains("COPPER KITE"));
    assert_eq!(second.recalled().get()[0].response(), first.response());

    let requests = harness.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let second: serde_json::Value = common::request_data(&requests[1]);
    let selected = if second["schema"] == conduit_chat::EPISODE_RETRIEVAL_DATA_SCHEMA {
        common::model_projection::expanded_model_evidence(
            &second["current_experience"],
            second["observation"]["content"].as_str().unwrap(),
        )
    } else {
        second["selected_experience"].clone()
    };
    assert_eq!(selected["revision"], 2);
    assert_eq!(
        selected["evidence"][0]["identity"],
        "pete/battery/battery/2"
    );
    assert_eq!(
        selected["evidence"][1]["identity"],
        "pete/utterance/utterance/2"
    );
    assert_eq!(
        selected["evidence"][1]["classifications"]["origin"],
        "human_reported"
    );
    assert_eq!(selected["evidence"][2]["identity"], "pete/body/body/2");
    assert_eq!(selected["evidence"][2]["observed_at"]["ticks"], 200);
    assert!(second["recalled"][0].get("response").is_none());
    let derived = &selected["evidence"][3];
    assert_eq!(derived["identity"], "pete/homeostasis");
    assert_eq!(
        derived["classifications"]["origin"],
        "deterministic_derived"
    );
    assert_eq!(
        derived["sources"],
        serde_json::json!([
            {"sign": "battery/2"}, {"sign": "body/2"}
        ])
    );
    let homeostasis: serde_json::Value =
        serde_json::from_str(derived["utf8_content"].as_str().unwrap()).unwrap();
    assert_eq!(homeostasis["energy"], "critical");
    assert_eq!(homeostasis["motion"], "inhibited");
    assert_eq!(homeostasis["thermal"], "unknown");
    assert_eq!(homeostasis["charging"], false);
    assert_eq!(homeostasis["charge_percent"], "9.0%");
    assert!(homeostasis["policy_identity"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    let kernel = report.kernel.unwrap();
    println!(
        "CONDUIT_PLOT_EVIDENCE={{\"plan_id\":\"{}\",\"play_id\":\"{}\",\"workload_revision\":null}}",
        harness.plan_id,
        kernel.active_play_id.as_str()
    );
}

fn ollama_episode_model() -> Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter>
{
    use conduit_ai::LocalModelKindProfile;
    use conduit_std_host::hosted_local_model::OllamaDiscovery;

    let model = std::env::var("CONDUIT_PETE_LIVE_MODEL").unwrap_or_else(|_| "gemma3:latest".into());
    let memory_mib = std::env::var("CONDUIT_PETE_LIVE_MEMORY_MIB")
        .unwrap_or_else(|_| "6144".into())
        .parse()
        .expect("CONDUIT_PETE_LIVE_MEMORY_MIB must be an integer");
    println!("Live Pete model: {model}; admitted memory: {memory_mib} MiB");
    let adapter = OllamaDiscovery::discover_at("http://127.0.0.1:11434", &model)
        .unwrap()
        .initialize(memory_mib, vec![LocalModelKindProfile::GenerateFlow])
        .unwrap();
    Box::new(adapter)
}

#[test]
#[ignore = "requires a running loopback Ollama with gemma3:latest already installed"]
fn live_pete_experience_episode_uses_earlier_memory() {
    let mut harness = pete_harness(ollama_episode_model());
    let question = "What exact label did I report for the hallway switch?";
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(frame(1, "The hallway switch label is COPPER KITE.", true)),
        frame_input(frame(2, question, false)),
    ]);
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episodes = records(&collector.0);
    let [first, second] = episodes.as_slice() else {
        panic!("expected two completed episodes");
    };
    assert_eq!(first.body_id(), second.body_id());
    assert_eq!(first.wake_id(), second.wake_id());
    assert_eq!(
        second.recalled().get()[0].observation(),
        first.observation()
    );
    let recalled_answer = second.response().unwrap().content();
    println!("With recall: {recalled_answer}");
    let normalized = recalled_answer.trim().to_ascii_lowercase();
    for prefix in ["i do not know", "i don't know", "i don’t know"] {
        assert!(!normalized.starts_with(prefix), "{recalled_answer}");
    }
    assert!(second
        .response()
        .unwrap()
        .content()
        .to_ascii_uppercase()
        .contains("COPPER KITE"));

    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(frame(2, question, false)),
        frame_input(frame(
            3,
            "What is your current battery charge as a percentage?",
            false,
        )),
    ]);
    let mut control = Collector::default();
    let control_report = run(
        &mut harness,
        &incoming,
        &mut control,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&control_report), &TerminalDisposition::Completed);
    let control = records(&control.0);
    assert!(control[0].recalled().get().is_empty());
    let charge = control[1].response().unwrap().content();
    println!("Body question: {charge}");
    let fresh = control[0].response().unwrap().content();
    println!("Fresh Play: {fresh}");
    let fresh = fresh.trim().to_ascii_lowercase();
    // The authored persona may identify its speaker before the answer.
    let fresh = fresh.strip_prefix("pete:").unwrap_or(&fresh).trim_start();
    assert!(
        ["i do not know", "i don't know", "i don’t know"]
            .iter()
            .any(|prefix| fresh.starts_with(prefix)),
        "{fresh}"
    );
    for unrelated in ["copper kite", "battery", "charge", "volt", "%", "13000"] {
        assert!(!fresh.contains(unrelated), "{fresh}");
    }
    let compact_charge: String = charge.chars().filter(|c| !c.is_whitespace()).collect();
    let percentages: Vec<_> = compact_charge
        .split_inclusive('%')
        .filter_map(|part| part.strip_suffix('%'))
        .map(|part| {
            part.rsplit(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
                .next()
                .unwrap()
        })
        .collect();
    assert!(matches!(percentages.as_slice(), ["9" | "9.0"]), "{charge}");
}

#[test]
fn pete_retrieves_the_matching_episode_past_a_newer_unrelated_one() {
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness(model);
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(frame(1, "The hallway switch label is COPPER KITE.", true)),
        frame_input(frame(2, "The garden is cool and windy.", true)),
        frame_input(frame(
            3,
            "What exact label did I report for the hallway switch?",
            false,
        )),
        frame_input(frame(
            4,
            "What was the earlier reading in millivolts?",
            false,
        )),
    ]);
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let episodes = records(&collector.0);
    assert_eq!(episodes.len(), 4);
    assert!(episodes[1].recalled().get().is_empty());
    assert_eq!(episodes[2].recalled().get().len(), 1);
    assert_eq!(*episodes[2].recalled().get()[0].episode(), 1);
    assert!(episodes[2]
        .response()
        .unwrap()
        .content()
        .contains("COPPER KITE"));

    let requests = requests.lock().unwrap();
    let follow_up: serde_json::Value = common::request_data(&requests[2]);
    assert_eq!(
        follow_up["retrieval"]["relevance_profile"],
        conduit_chat::EPISODE_SHARED_TERM_PROFILE
    );
    assert_eq!(follow_up["recalled"][0]["episode"], 1);
    assert!(
        follow_up["recalled"][0]["historical_evidence"]["evidence"][0]["utf8_content"]
            .as_str()
            .unwrap()
            .contains("COPPER KITE")
    );
    assert!(!requests[2].contains("garden"));
    let battery_query: serde_json::Value = common::request_data(&requests[3]);
    let recalled = battery_query["recalled"].as_array().unwrap();
    assert!(recalled.iter().any(|memory| {
        let item = &memory["historical_evidence"]["evidence"][0];
        item["origin"] == "observation"
            && item["identity"]
                .as_str()
                .is_some_and(|id| id.starts_with("pete/battery/"))
    }));
}

#[test]
fn pete_recalls_the_earlier_battery_percentage_with_its_source() {
    let (model, requests) = common::fenced_model();
    let mut harness = pete_harness(model);
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(create_frame(
            1,
            "Please remember the current battery charge reading.",
            true,
        )),
        frame_input(create_frame(
            2,
            "What was your battery charge percentage in the previous episode?",
            false,
        )),
    ]);
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default(),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let requests = requests.lock().unwrap();
    let second: serde_json::Value = common::request_data(&requests[1]);
    let recalled = &second["recalled"][0]["historical_evidence"];
    assert_eq!(recalled["original_provenance"]["revision"], 1);
    let body = &recalled["evidence"][0];
    assert_eq!(body["origin"], "deterministic_derived");
    assert_eq!(body["sources"][0]["sign"], "create/battery/1");
    let content: serde_json::Value =
        serde_json::from_str(body["utf8_content"].as_str().unwrap()).unwrap();
    assert_eq!(content["charge_percent"], "50.0%");
    let current_evidence = common::model_projection::expanded_model_evidence(
        &second["current_experience"],
        second["observation"]["content"].as_str().unwrap(),
    );
    let current = current_evidence["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["identity"] == "pete/homeostasis")
        .unwrap();
    let content: serde_json::Value =
        serde_json::from_str(current["utf8_content"].as_str().unwrap()).unwrap();
    assert_eq!(content["charge_percent"], "9.0%");
}

#[test]
fn pete_records_provider_loss_and_resumes_in_the_same_play() {
    use conduit_chat::EpisodeOutcome;
    use conduit_std_host::hosted_local_model::LocalModelAdapterTerminal;

    let (model, requests) =
        common::fenced_model_with_terminal(Some((2, LocalModelAdapterTerminal::ProviderLost)));
    let mut harness = pete_harness(model);
    let mut incoming = inputs(&[]);
    incoming.extend([
        frame_input(frame(1, "The hallway switch label is COPPER KITE.", true)),
        frame_input(frame(2, "The garden gate is closed.", true)),
        frame_input(frame(3, "What was the hallway switch label?", false)),
        frame_input(frame(4, "What was the garden gate report?", false)),
        frame_input(frame(
            5,
            "What was the earlier model provider outcome?",
            false,
        )),
    ]);
    let mut collector = Collector::default();
    let report = run(
        &mut harness,
        &incoming,
        &mut collector,
        &RunControl::default().with_debug_body(common::context().body_id),
    )
    .unwrap();
    assert_eq!(terminal(&report), &TerminalDisposition::Completed);
    let snapshot = assert_pete_replay(&harness, &report, 5);
    if let Some(path) = std::env::var_os("CONDUIT_PETE_LOSS_REPLAY_SNAPSHOT") {
        std::fs::write(path, snapshot.encode().unwrap()).unwrap();
    }

    let episodes = records(&collector.0);
    let [first, lost, resumed, recalled_loss, outcome_recall] = episodes.as_slice() else {
        panic!("expected one record for each completed attempt");
    };
    assert_eq!(first.body_id(), resumed.body_id());
    assert_eq!(first.wake_id(), resumed.wake_id());
    assert_eq!(*lost.episode(), 2);
    assert!(lost.response().is_none());
    assert!(*lost.retained());
    let evidence: serde_json::Value =
        serde_json::from_str(lost.evidence().as_ref().unwrap()).unwrap();
    assert_eq!(evidence["revision"], 2);
    let EpisodeOutcome::ProviderLost(outcome) = lost.outcome() else {
        panic!("expected typed provider loss");
    };
    assert!(outcome.attempt().starts_with("host-attempt/"));
    assert!(!outcome.implementation().is_empty());
    assert_eq!(*resumed.episode(), 3);
    assert_eq!(resumed.recalled().get().len(), 1);
    assert_eq!(*resumed.recalled().get()[0].episode(), 1);
    assert!(resumed
        .response()
        .unwrap()
        .content()
        .contains("COPPER KITE"));
    assert_eq!(*recalled_loss.episode(), 4);
    let selected = &recalled_loss.recalled().get()[0];
    assert_eq!(*selected.episode(), 2);
    assert_eq!(selected.outcome(), lost.outcome());
    assert!(selected.response().is_none());
    assert!(recalled_loss
        .response()
        .unwrap()
        .content()
        .contains("garden gate is closed"));
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 5);
    assert_eq!(
        *outcome_recall.recalled().get().last().unwrap().episode(),
        2
    );
    let outcome_prompt: serde_json::Value = common::request_data(&requests[4]);
    assert_eq!(
        outcome_prompt["recalled"]
            .as_array()
            .unwrap()
            .last()
            .unwrap()["actual_outcome"]["disposition"],
        "provider_lost"
    );
    let prompt: serde_json::Value = common::request_data(&requests[3]);
    assert!(prompt["recalled"][0].get("response").is_none());
    assert_eq!(prompt["recalled"][0]["actual_outcome"]["source"], "host");
    assert_eq!(
        prompt["recalled"][0]["actual_outcome"]["disposition"],
        "provider_lost"
    );
    assert!(prompt["recalled"][0]["actual_outcome"]
        .get("attempt")
        .is_none());
    let EpisodeOutcome::ProviderLost(audited) = selected.outcome() else {
        panic!("native audit must retain the actual failed attempt");
    };
    assert_eq!(audited.attempt(), outcome.attempt());
}

#[path = "experience_episode_loop/checkpoint.rs"]
mod pete_checkpoint_loop;

#[test]
fn stale_battery_and_safety_do_not_become_current_homeostatic_facts() {
    let frame = frame(1, "What is the current body state?", false);
    let later = PeteCognitionFrame::new(
        frame.battery().clone(),
        frame.body().clone(),
        frame.provenance_revision().clone(),
        at(200),
        *frame.revision(),
        *frame.temporal_policy(),
        frame.utterance().clone(),
    )
    .unwrap();
    let episode = conduit_pete::situate_pete_frame(&later).unwrap();
    let evidence = episode.evidence().evidence().get();
    assert_eq!(
        *evidence[0].temporal_role(),
        conduit_human::ExperienceTemporalRole::Stale
    );
    let homeostasis = &evidence[3];
    assert!(homeostasis.sources().get().is_empty());
    let content: serde_json::Value =
        serde_json::from_slice(homeostasis.encoded_content().get().as_slice()).unwrap();
    assert_eq!(content["energy"], "unknown");
    assert_eq!(content["motion"], "unknown");
    assert!(content["charging"].is_null());
    assert!(content["charge_percent"].is_null());
}

#[path = "experience_episode_loop/provenance.rs"]
mod retained_provenance;
