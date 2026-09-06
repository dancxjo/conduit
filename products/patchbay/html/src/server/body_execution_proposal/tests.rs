use super::*;
use conduit_core::BaseImplementationId;
use patchbay_model::BodyPlanningSession;
use std::io::Read;

fn proposed_server() -> PatchbayHtmlServer {
    let snapshot = crate::body_workbench_fixture_snapshot(false).unwrap();
    let mut server = PatchbayHtmlServer::bind_ephemeral(&snapshot)
        .unwrap()
        .with_body_planning_forms(crate::body_workbench_fixture_forms().unwrap())
        .unwrap();
    let evidence = server.body_workload.as_ref().unwrap().evidence();
    let current = evidence
        .membership
        .parts
        .iter()
        .find_map(|part| part.current.as_ref())
        .unwrap();
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    host.host_id = current.host_id.clone();
    host.boot_id = current.boot_id.clone();
    host.offer_generation = current.offer_generation;
    let forms = patchbay_model::plan_body_workset_on_host(
        &evidence.body.workset,
        &server.body_planning_forms,
        &host,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    server.body_planning = Some(
        BodyPlanningSession::prepare(&evidence.body, 1, "sign/proposal-wake".into(), forms)
            .unwrap(),
    );
    server
}

#[test]
fn exact_proposal_round_trips_without_admission_or_state_mutation() {
    let server = proposed_server();
    let before = server.encoded_snapshot.clone();
    let biography = server.current_body_evidence().unwrap();
    let planning = server.body_planning.as_ref().unwrap();
    let bytes = server.body_execution_proposal().unwrap();
    assert!(bytes.len() <= MAX_PROPOSAL_BYTES);
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["schema"], "conduit.patchbay/body-execution-proposal@1");
    let wake: Wake = serde_json::from_value(json["wake"].clone()).unwrap();
    let plan: BodyPlan = serde_json::from_value(json["plan"].clone()).unwrap();
    assert_eq!(&wake, planning.wake());
    assert_eq!(&plan, planning.current_plan());
    plan.validate_for(&wake).unwrap();
    assert!(wake.plans.is_empty());
    assert_eq!(wake.lifecycle, WakeLifecycle::AwaitingPlan);
    assert!(json.get("observations").is_none());
    assert!(json.get("play").is_none());
    assert_eq!(server.encoded_snapshot, before);
    assert_eq!(server.current_body_evidence().unwrap(), biography);
    assert_eq!(server.body_execution_proposal().unwrap(), bytes);
}

#[test]
fn absent_unavailable_started_and_stale_proposals_refuse() {
    let mut server = proposed_server();
    let original = server.body_planning.clone().unwrap();
    server.body_planning = None;
    assert!(
        matches!(server.body_execution_proposal(), Err(ServerError::Interaction(reason)) if reason == "BodyProposalAbsent")
    );
    server.body_planning = Some(original.clone());
    server
        .body_planning
        .as_mut()
        .unwrap()
        .mark_current_unsatisfied("sign/host-loss".into())
        .unwrap();
    assert!(
        matches!(server.body_execution_proposal(), Err(ServerError::Interaction(reason)) if reason == "BodyProposalUnavailable")
    );
    let body = server
        .body_workload
        .as_ref()
        .unwrap()
        .evidence()
        .body
        .clone();
    server.body_planning = Some(
        BodyPlanningSession::start(
            &body,
            1,
            "sign/started-wake".into(),
            original.current_plan().forms.clone(),
            "sign/plan-ready".into(),
            1,
            "sign/play-started".into(),
        )
        .unwrap(),
    );
    assert!(
        matches!(server.body_execution_proposal(), Err(ServerError::Interaction(reason)) if reason == "BodyProposalAlreadyAdmitted")
    );
    let different_body =
        conduit_body::Body::born_with_forms(body.workset.clone(), 99, "sign/different-body".into())
            .unwrap();
    server.body_planning = Some(
        BodyPlanningSession::prepare(
            &different_body,
            1,
            "sign/different-wake".into(),
            original.current_plan().forms.clone(),
        )
        .unwrap(),
    );
    assert!(
        matches!(server.body_execution_proposal(), Err(ServerError::Interaction(reason)) if reason == "BodyProposalStaleWorkload")
    );
}

#[test]
fn fresh_membership_must_still_match_the_proposed_host_boot_and_generation() {
    let mut server = proposed_server();
    let original = server.body_planning.as_ref().unwrap();
    let body = server
        .body_workload
        .as_ref()
        .unwrap()
        .evidence()
        .body
        .clone();
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    let fragment = &original.current_plan().forms[0].plan.fragments[0];
    host.host_id = fragment.host_id.clone();
    host.boot_id = "boot/never-admitted".into();
    host.offer_generation = fragment.offer_generation;
    let forms = patchbay_model::plan_body_workset_on_host(
        &body.workset,
        &server.body_planning_forms,
        &host,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    server.body_planning =
        Some(BodyPlanningSession::prepare(&body, 1, "sign/stale-host-wake".into(), forms).unwrap());
    assert!(
        matches!(server.body_execution_proposal(), Err(ServerError::Interaction(reason)) if reason == "BodyProposalStaleHost")
    );
}

#[test]
fn serialization_bound_refuses_before_growing_past_capacity() {
    let mut output = BoundedOutput(Vec::with_capacity(MAX_PROPOSAL_BYTES));
    output.write_all(&vec![0; MAX_PROPOSAL_BYTES]).unwrap();
    assert!(output.write_all(&[1]).is_err());
    assert_eq!(output.0.len(), MAX_PROPOSAL_BYTES);
    assert!(output.0.iter().all(|byte| *byte == 0));
}

#[test]
fn http_route_transfers_a_proposal_and_absence_is_a_nonfatal_conflict() {
    for present in [false, true] {
        let mut server = proposed_server();
        if !present {
            server.body_planning = None;
        }
        let address = server.local_addr().unwrap();
        let worker = std::thread::spawn(move || server.serve_count(1));
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .write_all(b"GET /api/body-execution-proposal HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        worker.join().unwrap().unwrap();
        if present {
            assert!(response.starts_with("HTTP/1.1 200 OK"));
            assert!(response.contains("conduit.patchbay/body-execution-proposal@1"));
        } else {
            assert!(response.starts_with("HTTP/1.1 409 Conflict"));
            assert!(response.ends_with("BodyProposalAbsent"));
        }
    }
}
