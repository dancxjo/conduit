use super::*;
use crate::server::body_execution_proposal::tests::proposed_server;
use conduit_core::bind_sign;
use serde_json::{json, Value};
use std::io::{Read, Write};

fn request(action: Value) -> Vec<u8> {
    serde_json::to_vec(
        &json!({"schema": "conduit.patchbay/body-execution-request@1", "action": action}),
    )
    .unwrap()
}

fn claim_request(server: &PatchbayHtmlServer) -> Vec<u8> {
    let plan = server.body_planning.as_ref().unwrap().current_plan();
    let fragment = &plan.forms[0].plan.fragments[0];
    request(
        json!({"kind": "Claim", "plan_id": plan.plan_id, "host_id": fragment.host_id, "boot_id": fragment.boot_id}),
    )
}

#[test]
fn claim_report_and_terminal_update_one_exact_snapshot_atomically() {
    let mut server = proposed_server();
    let bytes = claim_request(&server);
    server.apply_body_execution(&bytes).unwrap();
    let claim = server
        .snapshot
        .body_planning
        .as_ref()
        .unwrap()
        .execution_claims[0]
        .clone();
    let before = server.encoded_snapshot.clone();
    assert!(
        matches!(server.apply_body_execution(&bytes), Err(ServerError::Interaction(reason)) if reason == "BodyExecutionOutstandingClaim")
    );
    assert_eq!(server.encoded_snapshot, before);
    let planning = server.body_planning.as_ref().unwrap();
    let sign = |sequence| {
        bind_sign(
            &claim.host_id,
            &claim.boot_id,
            Some(&claim.play.active_play_id),
            sequence,
        )
        .sign_id
    };
    let wake = planning
        .wake()
        .body_plan_ready(planning.current_plan(), sign(0))
        .unwrap()
        .body_play_started(planning.current_plan(), &claim.play, sign(1))
        .unwrap();
    server
        .apply_body_execution(&request(
            json!({"kind": "Started", "play": claim.play, "wake_at_start": wake}),
        ))
        .unwrap();
    assert_eq!(server.body_planning.as_ref().unwrap().wake(), &wake);
    let retained: conduit_body::BodyBiographyEvidence =
        serde_json::from_slice(&server.current_body_evidence().unwrap()).unwrap();
    retained.validate().unwrap();
    assert_eq!(retained.wakes, vec![wake.clone()]);
    assert_eq!(
        &retained.body,
        server.body_planning.as_ref().unwrap().body()
    );
    let history_before_terminal = server.current_body_evidence().unwrap();
    server.apply_body_execution(&request(json!({"kind": "Terminal", "play": claim.play, "disposition": "completed", "terminal_sign_id": sign(2)}))).unwrap();
    let snapshot = server.snapshot.body_planning.as_ref().unwrap();
    assert!(snapshot.execution_claims[0].started_reported);
    assert_eq!(snapshot.lifecycle, conduit_body::WakeLifecycle::Playing);
    assert_eq!(
        server.current_body_evidence().unwrap(),
        history_before_terminal
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&server.encoded_snapshot).unwrap()["body_planning"],
        serde_json::to_value(snapshot).unwrap()
    );
    let lull = request(json!({"kind": "Lull", "wake_id": wake.wake_id}));
    server.apply_body_execution(&lull).unwrap();
    let retained: conduit_body::BodyBiographyEvidence =
        serde_json::from_slice(&server.current_body_evidence().unwrap()).unwrap();
    assert_eq!(retained.body.state, conduit_body::BodyState::Lulled);
    assert_eq!(
        retained.wakes[0].lifecycle,
        conduit_body::WakeLifecycle::Lulled
    );
    assert_eq!(
        server
            .body_planning
            .as_ref()
            .unwrap()
            .snapshot()
            .execution_claims[0]
            .play,
        claim.play
    );
    let before = server.encoded_snapshot.clone();
    assert!(server.apply_body_execution(&lull).is_err());
    assert_eq!(server.encoded_snapshot, before);
}

#[test]
fn malformed_stale_unavailable_and_overflow_requests_do_not_mutate() {
    let mut server = proposed_server();
    let bytes = claim_request(&server);
    let before = server.encoded_snapshot.clone();
    for invalid in [
        Vec::new(),
        b"{}".to_vec(),
        vec![b' '; MAX_EXECUTION_REQUEST_BYTES + 1],
        request(json!({"kind": "Claim", "extra": true})),
    ] {
        assert!(matches!(
            server.apply_body_execution(&invalid),
            Err(ServerError::InvalidRequest)
        ));
        assert_eq!(server.encoded_snapshot, before);
    }
    let mut stale: Value = serde_json::from_slice(&bytes).unwrap();
    stale["action"]["boot_id"] = json!("boot/stale");
    assert!(server
        .apply_body_execution(&serde_json::to_vec(&stale).unwrap())
        .is_err());
    assert_eq!(server.encoded_snapshot, before);
    server.snapshot.interaction.revision = u64::MAX;
    assert!(
        matches!(server.apply_body_execution(&bytes), Err(ServerError::Interaction(reason)) if reason == "BodyExecutionRevisionExhausted")
    );
    assert!(!server
        .body_planning
        .as_ref()
        .unwrap()
        .has_outstanding_execution_claim());
    server
        .body_planning
        .as_mut()
        .unwrap()
        .mark_current_unsatisfied("sign/host-left".into())
        .unwrap();
    assert!(
        matches!(server.apply_body_execution(&bytes), Err(ServerError::Interaction(reason)) if reason == "BodyProposalUnavailable")
    );
    assert_eq!(server.encoded_snapshot, before);
}

#[test]
fn loopback_claim_route_refuses_second_start_without_killing_server() {
    let server = proposed_server();
    let bytes = claim_request(&server);
    let address = server.local_addr().unwrap();
    let worker = std::thread::spawn(move || server.serve_count(3));
    for (body, status) in [
        (&bytes[..], "200 OK"),
        (&bytes[..], "409 Conflict"),
        (&b"{}"[..], "400 Bad Request"),
    ] {
        let mut stream = TcpStream::connect(address).unwrap();
        write!(
            stream,
            "POST /api/body-execution HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status}")),
            "{response}"
        );
    }
    worker.join().unwrap().unwrap();
}
