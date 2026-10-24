use super::*;
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

fn frame() -> (In, u32) {
    (serde_json::from_str(r#"{"kind":"presence-leave","protocol":1,"credential_id":"credential/test","body_id":"body/test","part_id":"part/test","host_id":"host/test","boot_id":"boot/test","sequence":2}"#).unwrap(), 100)
}

#[test]
fn pending_operation_retains_one_follow_up_and_waits_for_actual_result() {
    let mut work = PendingAction::default();
    let (started, observed) = mpsc::sync_channel(1);
    let (release, held) = mpsc::sync_channel(1);
    work.start_operation(move || {
        started.send(()).unwrap();
        held.recv().unwrap();
        Ok(())
    })
    .unwrap();
    observed.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(
        work.take_response().is_none(),
        "staging is not a commit acknowledgement"
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let extra = calls.clone();
    assert!(work
        .start_operation(move || {
            extra.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
        .is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(work
        .defer((
            frame().0,
            conduit_face::MAX_OWNER_FACE_RESPONSE_BYTES as u32 + 1
        ))
        .is_err());
    work.defer(frame()).unwrap();
    assert!(work.defer(frame()).is_err());
    assert!(work.take_deferred().is_none());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(1);
    let response = loop {
        if let Some(response) = work.take_response() {
            break response;
        }
        assert!(Instant::now() < deadline);
        std::thread::park_timeout(Duration::from_millis(1));
    };
    assert!(
        matches!(response, Out::FaceInteractionResponse { accepted: true, code, .. } if code.is_empty())
    );
    assert!(!work.is_pending());
    assert!(work.take_deferred().is_some());
    assert!(work.take_deferred().is_none());
}

#[test]
fn disconnected_worker_is_outcome_unknown_and_never_accepted() {
    let (send, receive) = mpsc::sync_channel(1);
    let mut work = PendingAction {
        result: Some(receive),
        deferred: None,
    };
    drop(send);
    assert!(
        matches!(work.take_response(), Some(Out::FaceInteractionResponse { accepted: false, code, .. }) if code == "control-outcome-unknown")
    );
    assert!(!work.is_pending());
}

#[test]
fn refusal_mapping_preserves_stale_show_and_unknown_outcome() {
    for (internal, external) in [
        ("stale owner Mask Show", "stale-face-or-show"),
        ("control-outcome-unknown", "control-outcome-unknown"),
        ("browser-route-carrier-mismatch", "mask-route-unavailable"),
    ] {
        assert!(
            matches!(response(Err(internal.into())), Out::FaceInteractionResponse { accepted: false, code, .. } if code == external)
        );
    }
}

#[test]
fn worker_permits_remain_finite_until_the_operation_releases_them() {
    let active = AtomicUsize::new(0);
    let mut permits = Vec::new();
    for _ in 0..super::super::MAX_CONNECTIONS {
        permits.push(WorkerPermit::acquire(&active).unwrap());
    }
    assert!(WorkerPermit::acquire(&active).is_err());
    assert_eq!(
        active.load(Ordering::Acquire),
        super::super::MAX_CONNECTIONS
    );
    drop(permits.pop());
    permits.push(WorkerPermit::acquire(&active).unwrap());
    drop(permits);
    assert_eq!(active.load(Ordering::Acquire), 0);
    assert!(
        matches!(response(Err("browser owner operation capacity exhausted".into())),
        Out::FaceInteractionResponse { accepted: false, code, .. } if code=="owner-action-capacity")
    );
}
