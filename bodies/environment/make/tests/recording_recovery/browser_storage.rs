//! Real browser storage services original production-kernel requests. Authority,
//! Body start and the process/file transport are explicit proof fixtures.
use super::batch_browser::binding;
use super::body_lifecycle::support;
use super::*;
use conduit_browser_runtime::resource_snapshot::PreparedSnapshotRecord;
use conduit_environment_browser::*;
use conduit_environment_make::{NativeBodyBinding, NativeBodyBindingRefusal};
use conduit_kernel::{self as k, scheduler::*};
use std::{path::Path, process::Command};

#[test]
#[ignore = "requires pinned Chromium; cargo xtask prove environment recording-browser"]
fn original_kernel_reads_recover_overnight_chunks_from_actual_indexeddb() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let output = root.join("target/environment-recording-kernel-browser");
    std::fs::create_dir_all(&output).unwrap();
    let restored_file = output.join("restored.json");
    if restored_file.exists() {
        std::fs::remove_file(&restored_file).unwrap();
    }
    let contract = ObservationContract::prepare().unwrap();
    let mut forms =
        conduit_environment_runtime::observation::PreparedObservationQuantityForms::prepare(
            &contract,
        )
        .unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap().to_vec();
    let mut references = Vec::new();
    let mut publications = Vec::new();
    for index in 0u64..15 {
        let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
        for offset in 1..=64 {
            let sequence = index * 64 + offset;
            let available = !sequence.is_multiple_of(17);
            chunk
                .append(
                    sequence,
                    sequence * 60_000,
                    if available {
                        RecordingDisposition::Available
                    } else {
                        RecordingDisposition::ProviderFailed
                    },
                    available.then_some(observation.as_slice()),
                )
                .unwrap();
        }
        let content = chunk.seal().unwrap();
        let mut reference = reference();
        reference.extent.bytes = content.len() as u64;
        reference.lifetime.version =
            c::ResourceVersionIdentity::from_digest([(index + 1) as u8; 32]);
        let mut placement = binding::for_reference("fixture/store/old-boot", true, &reference);
        placement.host_id = "fixture/recorder".into();
        placement.authority[0].host_id = placement.host_id.clone();
        placement.resources[0].content.as_mut().unwrap().owner_host = placement.host_id.clone();
        let mut writer = PreparedSnapshotRecord::prepare(&placement, &reference).unwrap();
        let record = writer
            .publication(&placement.authority[0], content.as_bytes())
            .unwrap()
            .to_vec();
        publications.push(serde_json::json!({"key": writer.storage_key(), "record": record}));
        references.push(reference);
    }
    let mut fixture = batch_plan::fixture(&references);
    let plan = PreparedRecordingRecovery::prepare_batch(
        &fixture.source,
        &fixture.host,
        &fixture.choices,
        &bases(),
        &fixture.grants,
    )
    .unwrap();
    let prepared = PreparedRecordingBatchKernel::<60, 59>::prepare(
        &plan.plan().fragments[0],
        plan.lowered(),
        &mut fixture.generations,
        &mut fixture.singleton,
        &mut fixture.concat,
        &mut fixture.recovery,
    )
    .unwrap();
    let fragment = &plan.plan().fragments[0];
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 1);
    let mut body = support::body_owner(
        plan.body_plot_plan().plot,
        &fragment.host_id,
        &fragment.boot_id,
        false,
    );
    let body_play = conduit_body::BodyPlayIdentity::bind(
        &plan.propose_to_body(&mut body, vec![]).unwrap().plan,
        1,
    );
    let mut lifecycle =
        NativeBodyBinding::prepare_recording(&plan, &body, &body_play, &active).unwrap();
    assert_eq!(
        lifecycle.fragment_play(&body),
        Err(NativeBodyBindingRefusal::NotStarted)
    );
    let mut reads = PreparedRecordingBrowserReads::prepare(
        fragment,
        plan.lowered(),
        &prepared.read_nodes,
        &active,
    )
    .unwrap();
    let recovery = prepared.recovery_node;
    let mut kernel = prepared.kernel;
    // Actual admission remains a fixture. Use the ordinary retained lifecycle
    // result before any kernel steps/effects, keeping Body and fragment distinct.
    support::start_body(&mut body, &body_play, &fragment.host_id, &fragment.boot_id);
    lifecycle.confirm_started(&body).unwrap();
    let mut requests = Vec::with_capacity(15);
    let mut effects = Vec::with_capacity(15);
    for _ in 0..2000 {
        if let Some(request) = kernel.next_host_request() {
            assert_eq!(lifecycle.fragment_play(&body).unwrap(), &active);
            let effect = reads.begin(&kernel, request).unwrap();
            effects.push(serde_json::to_value(effect).unwrap());
            requests.push(request);
            if requests.len() == 15 {
                break;
            }
        }
        kernel.step().unwrap();
    }
    assert_eq!(requests.len(), 15);
    let recovered = |kernel: &RecordingBatchKernel<60, 59>| {
        let RecordingBack::Recovery(back) = &kernel.drivers()[usize::from(recovery.0)] else {
            unreachable!()
        };
        back.recovered()
    };
    assert_eq!(recovered(&kernel), RecoveredHistory::default());
    std::fs::write(
        output.join("requests.json"),
        serde_json::to_vec(&serde_json::json!({
            "publications": publications, "reads": &effects,
        }))
        .unwrap(),
    )
    .unwrap();
    let status = Command::new("node")
        .current_dir(&root)
        .arg("proof/browser/node_modules/@playwright/test/cli.js")
        .args([
            "test",
            "--config",
            "proof/browser/environment-recording-kernel.config.mjs",
            "--project",
            "chromium",
            "--workers=1",
            "--retries=0",
        ])
        .status()
        .expect("pinned Playwright must be installed");
    assert!(status.success(), "actual browser storage failed");
    let returned: Vec<Returned> =
        serde_json::from_slice(&std::fs::read(restored_file).unwrap()).unwrap();
    assert_eq!(returned.len(), 15);
    assert_eq!(recovered(&kernel), RecoveredHistory::default());
    let mut seen = [false; 15];
    let ((), memory) = allocation_probe::observe(|| {
        // Request sequences are node-local. Equal sequence with another
        // placement must refuse before retiring either legitimate pending read.
        assert_eq!(requests[0].request, requests[14].request);
        let last = returned.last().unwrap();
        let foreign_scope = RecordingReadScope {
            host_id: &last.host_id,
            boot_id: &last.boot_id,
            active_play_id: &last.active_play_id,
            placement_id: &last.placement_id,
            request_sequence: last.request_sequence,
        };
        assert_eq!(
            reads.complete(
                &mut kernel,
                requests[0],
                foreign_scope,
                Ok(Some(&last.record))
            ),
            Err(BrowserReadRefusal::Identity)
        );
        // Reverse asynchronous completion cannot change Source ordering.
        for result in returned.iter().rev() {
            assert_eq!(lifecycle.fragment_play(&body).unwrap(), &active);
            let index = requests
                .iter()
                .enumerate()
                .position(|(index, request)| {
                    request.request.0 == result.request_sequence
                        && effects[index]["placement_id"].as_str()
                            == Some(result.placement_id.as_str())
                })
                .unwrap();
            assert!(!seen[index]);
            seen[index] = true;
            let scope = RecordingReadScope {
                host_id: &result.host_id,
                boot_id: &result.boot_id,
                active_play_id: &result.active_play_id,
                placement_id: &result.placement_id,
                request_sequence: result.request_sequence,
            };
            assert_eq!(
                reads
                    .complete(
                        &mut kernel,
                        requests[index],
                        scope,
                        Ok(Some(&result.record))
                    )
                    .unwrap(),
                k::HostCallDisposition::Completed
            );
            assert_eq!(recovered(&kernel), RecoveredHistory::default());
        }
        let mut drained = false;
        for _ in 0..4000 {
            if kernel.step().unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        assert!(drained);
        assert_eq!(
            recovered(&kernel),
            RecoveredHistory {
                chunks: 15,
                samples: 960,
                first_sequence: Some(1),
                last_sequence: Some(960)
            }
        );
    });
    assert!(seen.iter().all(|value| *value));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    body.lull(&fragment.host_id, &fragment.boot_id, Some(&body_play))
        .unwrap();
    assert_eq!(
        lifecycle.fragment_play(&body),
        Err(NativeBodyBindingRefusal::StaleProposal)
    );
    println!("actual_indexeddb_kernel_requests=15 samples=960 reverse_completion=true execution_allocations=0 body_start=fixture authority=fixture");
}
// Deserialize proof transport only; production compares the same borrowed scope.
#[derive(serde::Deserialize)]
struct Returned {
    host_id: String,
    boot_id: String,
    active_play_id: String,
    placement_id: String,
    request_sequence: u32,
    record: Vec<u8>,
}
