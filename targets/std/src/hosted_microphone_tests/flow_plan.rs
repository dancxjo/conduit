//! Authored finite capture through ordinary checking, planning and the kernel.
use super::*;
use crate::{
    ExternalForeDelivery, ExternalForeOutputAdapter, StdHost, StdHostComposition, StdHostConfig,
};
use conduit_core::{ConnectionTrack, PlanFragment, PortDirection};
use std::collections::BTreeMap;

fn planned(script: &str) -> (PathBuf, StdHost, PlanFragment) {
    let prelude="printf 'stream: CAPTURE\naccess: RW_INTERLEAVED\nformat: S16_LE\nchannels: 1\nrate: 16000\nexact rate: 16000 (16000/1)\nbuffer_size: 1024\nperiod_size: 256\nRecording raw data stdout : Signed 16 bit Little Endian, Rate 16000 Hz, Mono\n' >&2\n";
    let script = script.replacen("#!/bin/sh\n", &format!("#!/bin/sh\n{prelude}"), 1);
    let (root, discovery) = parsed_fixture(&script, "flow-plan");
    let selected = discovery.observations[0].clone();
    let adapter = discovery
        .initialize(
            &selected,
            MicrophoneLimits {
                capture_milliseconds: 1,
                timeout: Duration::from_secs(2),
            },
        )
        .unwrap();
    let host = StdHost::new_with_microphone(
        StdHostConfig {
            host_id: "host/finite-microphone-fixture".into(),
            boot_id: "boot/finite-microphone-fixture".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        adapter,
    )
    .unwrap();
    let profile = crate::installed_std::test_catalog();
    let startup = profile.startup_catalog().unwrap();
    let source="plot finite-microphone (\n audio: audio/pcm-frames@1...| >>\n) {\n microphone: audio/capture-finite(capture-millis = 1, block-frames = 10)\n \"capture\" >> microphone.request\n microphone.audio >> audio\n}\n";
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot_for_authoring(&checked, "finite-microphone", &profile)
            .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&expanded.expanded, &hosts).unwrap();
    let empty = BTreeMap::new();
    let limits = BTreeMap::from([(
        conduit_planner::ForeBoundaryKey {
            direction: PortDirection::Output,
            front_port_id: conduit_core::port_id("audio"),
            track: ConnectionTrack::Payload,
        },
        conduit_planner::ConnectionQueueLimits {
            item_capacity: 1,
            byte_capacity: 4125,
        },
    )]);
    let grant = host
        .microphone_flow_authority_grant("grant/finite-microphone-fixture")
        .unwrap();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &empty,
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 16,
            authority_grants: &[grant],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .unwrap();
    (root, host, plan.fragments.into_iter().next().unwrap())
}
struct Collector {
    release: PathBuf,
    blocks: Vec<Vec<u8>>,
}
impl ExternalForeOutputAdapter for Collector {
    fn deliver(&mut self, output: ExternalForeDelivery) -> Result<(), String> {
        // The fake provider cannot finish until this exact kernel Fore is delivered.
        fs::write(&self.release, b"delivered").map_err(|error| error.to_string())?;
        self.blocks.push(output.bytes.to_vec());
        Ok(())
    }
}
#[test]
fn finite_capture_reaches_sealed_fore_before_provider_completes() {
    let _process = microphone_process();
    let (root,mut host,fragment)=planned("#!/bin/sh\nprintf '01234567890123456789'\ni=0\nwhile [ ! -f \"$0.release\" ] && [ $i -lt 100 ]; do sleep 0.01; i=$((i+1)); done\n[ -f \"$0.release\" ] || exit 9\nprintf '012345678901'\n");
    let mut collector = Collector {
        release: root.join("arecord.release"),
        blocks: Vec::with_capacity(2),
    };
    let report = host
        .run_external_plot_to(
            fragment,
            &[],
            &mut collector,
            &mut Vec::new(),
            &mut crate::ThreadTimer,
        )
        .unwrap();
    assert_eq!(collector.blocks.len(), 2);
    let (first, bytes) = conduit_audio::PcmFrameHeader::decode_frame(&collector.blocks[0]).unwrap();
    assert_eq!(bytes, b"01234567890123456789");
    let (tail, bytes) = conduit_audio::PcmFrameHeader::decode_frame(&collector.blocks[1]).unwrap();
    assert_eq!(bytes, b"012345678901");
    assert_eq!((first.start_frame(), first.frame_count()), (0, 10));
    assert_eq!((tail.start_frame(), tail.frame_count()), (10, 6));
    assert_eq!(first.clock_id(), tail.clock_id());
    assert!(!report.observations.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn public_finite_flow_proof_requires_exact_terminal_and_keeps_hash_only() {
    let _process = microphone_process();
    let (root, mut host, _) = planned(
        "#!/bin/sh\n[ \"$LC_ALL\" = C ] || exit 7\nprintf '01234567890123456789012345678901'\n",
    );
    let receipt = crate::microphone_flow_proof::run(
        StdHostConfig {
            host_id: "host/proof-fixture".into(),
            boot_id: "boot/proof-fixture".into(),
            offer_generation: OfferGeneration(1),
        },
        StdHostComposition::reference(),
        host.microphone.take().unwrap(),
        10,
    )
    .unwrap();
    assert_eq!(receipt.total_frames, 16);
    assert_eq!(receipt.blocks, 2);
    assert_eq!(receipt.pcm_sha256.len(), 64);
    assert!(receipt.first_delivery_microseconds <= receipt.elapsed_microseconds);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn admitted_remote_finite_capture_starts_only_on_request_and_cancels_promptly() {
    let _process = microphone_process();
    let (root, mut host, fragment) =
        planned("#!/bin/sh\ntouch \"$0.started\"\nprintf '01234567890123456789'\nsleep 10\n");
    let mut remote = host.prepare_remote_fragment(&fragment).unwrap();
    assert!(!root.join("arecord.started").exists());
    let deadline = Instant::now() + Duration::from_secs(1);
    let mut accepted = false;
    while Instant::now() < deadline {
        if let Some(request) = remote.next_host_request() {
            let flow = remote
                .host_request_view(request)
                .unwrap()
                .contract_id
                .as_str()
                == conduit_std_offers::MICROPHONE_FLOW_OPERATION;
            if flow {
                assert!(host
                    .complete_remote_microphone_host_call(&mut remote, request, || false)
                    .unwrap());
                accepted = true;
            } else {
                assert!(remote.complete_portable_host_call(request).unwrap());
            }
        } else {
            remote.step().unwrap();
        }
        if accepted && root.join("arecord.started").exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(accepted);
    assert!(root.join("arecord.started").exists());
    let started = Instant::now();
    remote.cancel().unwrap();
    assert!(started.elapsed() < Duration::from_millis(500));
    drop(remote);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_capture_authority_is_refused_before_provider_execution() {
    let _process = microphone_process();
    let (root, mut host, mut fragment) =
        planned("#!/bin/sh\ntouch \"$0.started\"\nprintf '01234567890123456789012345678901'\n");
    let capture = fragment
        .placements
        .iter_mut()
        .find(|placement| {
            placement.kind_id.as_str() == conduit_semantic_catalog::AUDIO_CAPTURE_FINITE_KIND
        })
        .unwrap();
    capture.authority.clear();
    let mut collector = Collector {
        release: root.join("arecord.release"),
        blocks: Vec::new(),
    };
    assert!(host
        .run_external_plot_to(
            fragment,
            &[],
            &mut collector,
            &mut Vec::new(),
            &mut crate::ThreadTimer
        )
        .is_err());
    assert!(!root.join("arecord.started").exists());
    assert!(collector.blocks.is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_finite_provider_releases_lease_for_a_fresh_play() {
    let _process = microphone_process();
    // The pinned executable never changes. Only its first explicit capture fails.
    let (root, mut host, fragment) = planned("#!/bin/sh\nif [ ! -f \"$0.failed-once\" ]; then touch \"$0.failed-once\"; exit 7; fi\nprintf '01234567890123456789012345678901'\n");
    let mut collector = Collector {
        release: root.join("arecord.release"),
        blocks: Vec::with_capacity(2),
    };
    let error = host
        .run_external_plot_to(
            fragment.clone(),
            &[],
            &mut collector,
            &mut Vec::new(),
            &mut crate::ThreadTimer,
        )
        .unwrap_err();
    assert!(
        error.contains("HostCallFailed") && error.contains("detail: 9"),
        "{error}"
    );
    assert!(
        error.contains("logical-gear=finite-microphone/microphone"),
        "{error}"
    );
    assert!(collector.blocks.is_empty());
    let report = host
        .run_external_plot_to(
            fragment,
            &[],
            &mut collector,
            &mut Vec::new(),
            &mut crate::ThreadTimer,
        )
        .unwrap();
    assert_eq!(collector.blocks.len(), 2);
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Completed
        })
    ));
    fs::remove_dir_all(root).unwrap();
}
