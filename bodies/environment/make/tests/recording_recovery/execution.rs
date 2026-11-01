//! Exact ordinary lowered graph; only the external store completion is a CPU
//! fixture. This is neither browser persistence nor a Body start receipt.
use super::*;
use conduit_kernel::{self as k, scheduler::*, ValueStorage};

#[test]
fn lowered_recovery_requires_the_selected_read_before_committing_history() {
    execute(k::HostCallDisposition::Completed, false);
}
#[test]
fn lowered_recovery_refuses_failed_cancelled_and_invalid_reads() {
    execute(k::HostCallDisposition::Failed, false);
    execute(k::HostCallDisposition::Cancelled, false);
    execute(k::HostCallDisposition::Completed, true);
}
fn execute(disposition: k::HostCallDisposition, invalid: bool) {
    let contract = ObservationContract::prepare().unwrap();
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    chunk
        .append(1, 10, RecordingDisposition::CaptureTimeout, None)
        .unwrap();
    chunk
        .append(3, 20, RecordingDisposition::Checksum, None)
        .unwrap();
    let content = if invalid { "{}" } else { chunk.seal().unwrap() }.to_owned();
    let mut reference = reference();
    reference.extent.bytes = content.len() as u64;
    let encoded = reference.encode().unwrap();
    let source = RecordingRecoverySource::compile(&reference).unwrap();
    let (mut host, grant) = fixture_store(&reference);
    let (mut generations, mut recovery, mut singleton) = software(&source, &mut host);
    let prepared = PreparedRecordingRecovery::prepare(&source, &host, &bases(), &[grant]).unwrap();
    let fragment = &prepared.plan().fragments[0];
    let lowered = prepared.lowered();
    let prepared = PreparedRecordingKernel::prepare(
        fragment,
        lowered,
        &mut generations,
        &mut singleton,
        &mut recovery,
    )
    .unwrap();
    let read_node = prepared.roles.read;
    let recovery_node = prepared.roles.recovery;
    let mut kernel = prepared.kernel;
    let recovered = |kernel: &RecordingKernel| {
        let RecordingBack::Recovery(back) = &kernel.drivers()[usize::from(recovery_node.0)] else {
            unreachable!()
        };
        back.recovered()
    };
    let mut requests = 0;
    let mut terminal = false;
    let successful = disposition == k::HostCallDisposition::Completed && !invalid;
    let ((), memory) = allocation_probe::observe(|| {
        for _ in 0..80 {
            if let Some(request) = kernel.next_host_request() {
                requests += 1;
                assert_eq!(request.node, read_node);
                assert_eq!(kernel.values().get(request.input.value).unwrap(), encoded);
                assert_eq!(recovered(&kernel), RecoveredHistory::default());
                // A pending read cannot advance recovery, even when other nodes run.
                for _ in 0..3 {
                    kernel.step().unwrap();
                }
                assert_eq!(recovered(&kernel), RecoveredHistory::default());
                let output = (disposition == k::HostCallDisposition::Completed).then(|| {
                    let value = kernel.store_host_value(content.as_bytes()).unwrap();
                    k::BoundedValueRef::new(value, 4096).unwrap()
                });
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition,
                            output,
                            failure: (disposition == k::HostCallDisposition::Failed).then_some(
                                k::Failure {
                                    code: k::FailureCode::HostCallFailed,
                                    detail: 1,
                                },
                            ),
                        },
                    )
                    .unwrap();
            }
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert!(successful);
                    terminal = true;
                    break;
                }
                Err(SchedulerError::BackFailed(_)) => {
                    assert!(!successful);
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
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(
        recovered(&kernel),
        if successful {
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
