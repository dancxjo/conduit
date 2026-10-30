//! Ordinary planned/lowered recovery kernel and actual Linux files. Resource
//! offers, grants, producer context and publisher binding remain explicit CPU
//! fixtures; this does not start a Body or prove a live storage Host offer.
use super::*;
use conduit_kernel::{self as k, scheduler::*, ValueStorage};
use conduit_linux_snapshot_store::{FileOperationRefusal, PreparedFileSnapshotOperation};
use conduit_resource_snapshot::{PUBLISH_OPERATION, READ_OPERATION};
#[path = "../../../../../targets/std/resource-snapshot/tests/support/directory.rs"]
mod directory;
#[path = "../../../../../mechanisms/io/resource-snapshot/tests/support/fixture.rs"]
mod publisher_fixture;

#[derive(Clone, Copy)]
enum Mode {
    Valid,
    Missing,
    InvalidJson,
    InvalidChunk,
}
#[test]
fn actual_file_read_completes_the_original_recovery_kernel_request_without_allocations() {
    execute(Mode::Valid);
}
#[test]
fn missing_file_does_not_commit_recovered_history() {
    execute(Mode::Missing);
}
#[test]
fn invalid_json_in_a_valid_file_envelope_fails_before_kernel_success() {
    execute(Mode::InvalidJson);
}
#[test]
fn valid_json_with_invalid_recording_content_cannot_commit_history() {
    execute(Mode::InvalidChunk);
}
fn execute(mode: Mode) {
    let contract = ObservationContract::prepare().unwrap();
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    let mut forms =
        conduit_environment_runtime::observation::PreparedObservationQuantityForms::prepare(
            &contract,
        )
        .unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    chunk
        .append(1, 10, RecordingDisposition::Available, Some(observation))
        .unwrap();
    chunk
        .append(3, 20, RecordingDisposition::Checksum, None)
        .unwrap();
    let content = match mode {
        Mode::InvalidJson => "xx".to_owned(),
        Mode::InvalidChunk => "{}".to_owned(),
        _ => chunk.seal().unwrap().to_owned(),
    };
    let mut reference = reference();
    reference.extent.bytes = content.len() as u64;
    let encoded = reference.encode().unwrap();
    let source = RecordingRecoverySource::compile(&reference).unwrap();
    let (mut host, grant) = fixture_store(&reference);
    let (mut generations, mut recovery, mut singleton) = software(&source, &mut host);
    let plan = PreparedRecordingRecovery::prepare(&source, &host, &bases(), &[grant]).unwrap();
    let fragment = &plan.plan().fragments[0];
    let prepared = PreparedRecordingKernel::prepare(
        fragment,
        plan.lowered(),
        &mut generations,
        &mut singleton,
        &mut recovery,
    )
    .unwrap();
    let read_node = prepared.roles.read;
    let recovery_node = prepared.roles.recovery;
    let placement_id = &plan
        .lowered()
        .nodes
        .iter()
        .find(|n| n.node == read_node)
        .unwrap()
        .placement_id;
    let read = fragment
        .placements
        .iter()
        .find(|g| &g.placement_id == placement_id)
        .unwrap();
    let root = directory::Root::prepare();
    let directory = root.directory();
    if !matches!(mode, Mode::Missing) {
        // Independent publisher binding fixture: same durable residence Host,
        // old store Boot, separate write authority. No read grant is promoted.
        let (mut write, stored) =
            publisher_fixture::placement("fixture/recorder/old-boot", true, content.len());
        assert_eq!(stored.encode().unwrap(), reference.encode().unwrap());
        write.host_id = "fixture/recorder".into();
        write.authority[0].host_id = write.host_id.clone();
        write.resources[0].content.as_mut().unwrap().owner_host = write.host_id.clone();
        assert_eq!(write.host_id, host.host_id);
        conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(&directory, &write, &stored)
            .unwrap()
            .publish(&write.authority[0], content.as_bytes())
            .unwrap();
    }
    let mut files = PreparedFileSnapshotOperation::prepare(&directory, read, read_node).unwrap();
    let mut kernel = prepared.kernel;
    let history = |kernel: &RecordingKernel| {
        let RecordingBack::Recovery(back) = &kernel.drivers()[usize::from(recovery_node.0)] else {
            unreachable!()
        };
        back.recovered()
    };
    let mut requests = 0;
    let mut terminal = false;
    let mut completed_read = false;
    let (_, memory) = allocation_probe::observe(|| {
        for _ in 0..80 {
            if let Some(request) = kernel.next_host_request() {
                requests += 1;
                assert_eq!(request.node, read_node);
                assert_eq!(history(&kernel), RecoveredHistory::default());
                let result = {
                    let input = kernel.values().get(request.input.value).unwrap();
                    assert_eq!(input, encoded);
                    files.perform(request, input)
                };
                let outcome = match result {
                    Ok(bytes) => {
                        assert!(matches!(mode, Mode::Valid | Mode::InvalidChunk));
                        assert_eq!(bytes, content.as_bytes());
                        let value = kernel.store_host_value(bytes).unwrap();
                        completed_read = true;
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output: Some(
                                k::BoundedValueRef::new(value, files.output_bound()).unwrap(),
                            ),
                            failure: None,
                        }
                    }
                    Err(error) => {
                        let failure = match error {
                            FileOperationRefusal::Operation(failure) => {
                                assert!(matches!(mode, Mode::InvalidJson));
                                assert_eq!(failure.detail, 214);
                                failure
                            }
                            FileOperationRefusal::Storage(
                                conduit_linux_snapshot_store::FileSnapshotRefusal::Missing,
                            ) => {
                                assert!(matches!(mode, Mode::Missing));
                                k::Failure {
                                    code: k::FailureCode::HostCallFailed,
                                    detail: 1,
                                }
                            }
                            _ => panic!("unexpected file refusal: {error:?}"),
                        };
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Failed,
                            output: None,
                            failure: Some(failure),
                        }
                    }
                };
                // Even completed file I/O cannot advance history before the
                // original kernel request receives its own completion.
                for _ in 0..3 {
                    kernel.step().unwrap();
                }
                assert_eq!(history(&kernel), RecoveredHistory::default());
                kernel
                    .complete_host_call(request.node, request.request, outcome)
                    .unwrap();
                files.finish(request).unwrap();
                assert_eq!(history(&kernel), RecoveredHistory::default());
            }
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert!(matches!(mode, Mode::Valid));
                    terminal = true;
                    break;
                }
                Err(SchedulerError::BackFailed(_)) => {
                    assert!(!matches!(mode, Mode::Valid));
                    terminal = true;
                    break;
                }
                Err(error) => panic!("unexpected kernel refusal: {error:?}"),
                _ => {}
            }
        }
    });
    assert!(terminal);
    assert_eq!(requests, 1);
    assert_eq!(
        completed_read,
        matches!(mode, Mode::Valid | Mode::InvalidChunk)
    );
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(
        history(&kernel),
        if matches!(mode, Mode::Valid) {
            RecoveredHistory {
                chunks: 1,
                samples: 2,
                first_sequence: Some(1),
                last_sequence: Some(3),
            }
        } else {
            RecoveredHistory::default()
        }
    );
}

#[test]
fn owned_file_kernel_requires_exact_play_before_dequeue_or_step() {
    use conduit_environment_linux::{FileRecoveryRefusal, PreparedFileRecovery};
    let contract = ObservationContract::prepare().unwrap();
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    chunk
        .append(1, 10, RecordingDisposition::CaptureTimeout, None)
        .unwrap();
    let content = chunk.seal().unwrap();
    let mut reference = reference();
    reference.extent.bytes = content.len() as u64;
    let source = RecordingRecoverySource::compile(&reference).unwrap();
    let (mut host, grant) = fixture_store(&reference);
    let (mut generations, mut recovery, mut singleton) = software(&source, &mut host);
    let plan = PreparedRecordingRecovery::prepare(&source, &host, &bases(), &[grant]).unwrap();
    let fragment = &plan.plan().fragments[0];
    // CPU fixture, not a started Body or independent resource admission.
    let fragment_play = c::bind_active_play(&fragment.plan_id, &host.host_id, &host.boot_id, 7);
    let root = directory::Root::prepare();
    let directory = root.directory();
    let (mut write, stored) =
        publisher_fixture::placement("fixture/recorder/old-boot", true, content.len());
    write.host_id = "fixture/recorder".into();
    write.authority[0].host_id = write.host_id.clone();
    write.resources[0].content.as_mut().unwrap().owner_host = write.host_id.clone();
    conduit_linux_snapshot_store::PreparedFileSnapshot::prepare(&directory, &write, &stored)
        .unwrap()
        .publish(&write.authority[0], content.as_bytes())
        .unwrap();
    let mut wrong_preparation = fragment_play.clone();
    wrong_preparation.boot_id = "fixture/foreign-boot".into();
    assert!(matches!(
        PreparedFileRecovery::prepare(
            &directory,
            fragment,
            plan.lowered(),
            &wrong_preparation,
            &mut generations,
            &mut singleton,
            &mut recovery
        ),
        Err(FileRecoveryRefusal::Scope)
    ));
    let mut driver = PreparedFileRecovery::prepare(
        &directory,
        fragment,
        plan.lowered(),
        &fragment_play,
        &mut generations,
        &mut singleton,
        &mut recovery,
    )
    .unwrap();
    let foreign = c::bind_active_play(
        &fragment.plan_id,
        &host.host_id,
        &"fixture/foreign-boot".into(),
        7,
    );
    let signs = driver.kernel().signs().events().count();
    assert!(matches!(
        driver.step(&foreign),
        Err(FileRecoveryRefusal::Scope)
    ));
    assert!(matches!(
        driver.service_next(&foreign),
        Err(FileRecoveryRefusal::Scope)
    ));
    assert_eq!(driver.kernel().signs().events().count(), signs);
    assert!(matches!(
        driver.step(&wrong_preparation),
        Err(FileRecoveryRefusal::Scope)
    ));
    assert!(matches!(
        driver.service_next(&wrong_preparation),
        Err(FileRecoveryRefusal::Scope)
    ));
    let mut requests = 0;
    let mut drained = false;
    let (_, memory) = allocation_probe::observe(|| {
        for _ in 0..80 {
            let current = &fragment_play;
            if let Some(completion) = driver.service_next(current).unwrap() {
                requests += 1;
                assert_eq!(completion.disposition, k::HostCallDisposition::Completed);
                assert!(completion.refusal.is_none());
                assert_eq!(driver.history(), RecoveredHistory::default());
            }
            if driver.step(current).unwrap() == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
    });
    assert!(drained);
    assert_eq!(requests, 1);
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(driver.history().samples, 1);
    driver.cancel(&fragment_play).unwrap();
}
