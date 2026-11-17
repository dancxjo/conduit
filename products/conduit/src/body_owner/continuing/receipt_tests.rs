//! Persist actual local kernel reports without treating global terminal as audible.
use super::*;
#[test]
fn local_persistence_retains_exact_negative_chime_receipt_beside_global_terminal() {
    let (root, mut owner) = super::super::service_workset_tests::fixture();
    let mut worker = owner.start_service_run(&root, 5000).unwrap();
    let WorkerMode::Local(local) = &mut worker.mode else {
        panic!("local worker fixture")
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match local.started.try_recv() {
            Ok((play, wake)) => {
                owner.retain_started(&root, play, wake).unwrap();
                assert!(local.acknowledge.take().unwrap().send(Ok(None)).is_ok());
            }
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => (),
        }
        if local.thread.as_ref().unwrap().is_finished() {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "actual local worker must terminate"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
    let (host, result, output) = local.thread.take().unwrap().join().unwrap();
    owner.host.restore_after_play(host).unwrap();
    let RunReport::Reference(mut report) = result.unwrap() else {
        panic!("reference report fixture")
    };
    // Negative report-projection fixture; not a claim this Plot invoked audio.
    // Actual cue effects/receipts are separately proved by owning Std fake-backend tests.
    report
        .startup_chimes
        .push(conduit_std_host::body_execution::StartupChimeCallReceipt {
            node: conduit_kernel::NodeId(0),
            request: Some(conduit_kernel::RequestId(17)),
            disposition: Some(conduit_kernel::HostCallDisposition::Denied),
            failure: Some(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallDenied,
                detail: 7,
            }),
            kernel_accepted: false,
        });
    let expected = serde_json::to_value(&report.startup_chimes).unwrap();
    let terminal = serde_json::to_value(report.terminal).unwrap();
    let wake = serde_json::to_value(&report.wake_at_start).unwrap();
    owner
        .retain_finished(&root, Ok(RunReport::Reference(report)), output)
        .unwrap();
    let retained = state::execution(&root).unwrap().unwrap();
    assert_eq!(retained["startup_chimes"], expected);
    assert_eq!(retained["startup_chimes"][0]["disposition"], "denied");
    assert_eq!(retained["terminal"], terminal);
    assert_eq!(retained["wake_at_start"], wake);
    std::fs::remove_dir_all(root).unwrap();
}
