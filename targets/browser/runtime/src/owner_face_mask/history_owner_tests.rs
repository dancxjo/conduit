//! Actual owner response/ABI entrance with lifecycle and Line CPU fixtures.
//! No socket, existing-device membership, sensor Play or physical Show is claimed.
use super::*;
use conduit_body::*;
use conduit_core::*;
use conduit_face::*;

fn fixture() -> (serde_json::Value, Face, RemoteOwnerMaskRouteSeal) {
    let browser = crate::installed_browser::membership_advertisement(
        "host/history-browser-owner-entry".into(),
        "boot/history-browser-owner-entry".into(),
    );
    let owner = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/history-owner-entry".into(),
        boot_id: "boot/history-owner-entry".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/owner".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![],
    };
    let local = conduit_browser_mask_offer::planned_mask(
        &browser,
        HISTORY_COMPONENT_MASK_SOURCE,
        HISTORY_COMPONENT_MASK_ROOT,
    )
    .unwrap();
    let body = Body::born(
        local.plan.source_document_id.clone(),
        local.plan.checked_plot_id.clone(),
        1,
        "sign/history-owner-entry/born".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut biography =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "History fixture".into())
            .unwrap();
    let part = PartId::bind(&body.body_id, browser.host_id.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("fixture/history-browser-owner-entry").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/history-owner-entry/admitted".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: browser.host_id.clone(),
                boot_id: browser.boot_id.clone(),
                offer_generation: browser.offer_generation,
                proof_id: proof,
                sequence: 1,
            },
            "sign/history-owner-entry/present".into(),
        )
        .unwrap();
    biography
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let mut session =
        BodyLifecycleSession::open_admitted(biography, &browser.host_id, &browser.boot_id).unwrap();
    session
        .propose(
            vec![BodyPlotPlan {
                plot: ResidentPlot::new(
                    local.plan.source_document_id.clone(),
                    local.plan.checked_plot_id.clone(),
                ),
                plan: local.plan,
            }],
            &browser.host_id,
            &browser.boot_id,
        )
        .unwrap();
    let realization = session.realization().unwrap().clone();
    let active = BodyPlayIdentity::bind(&realization.plan, 1);
    let sign = |n| {
        bind_sign(
            &browser.host_id,
            &browser.boot_id,
            Some(&active.active_play_id),
            n,
        )
        .sign_id
    };
    let wake = realization
        .wake
        .body_plan_ready(&realization.plan, sign(0))
        .unwrap()
        .body_play_started(&realization.plan, &active, sign(1))
        .unwrap();
    session
        .started(&browser.host_id, &browser.boot_id, active.clone(), wake)
        .unwrap();
    let mut history = common::prepared(1);
    history
        .read_next(&common::chunk(&[
            (1, 1000, true),
            (2, 2000, false),
            (4, 11000, true),
        ]))
        .unwrap();
    let fragment = history
        .contribution(FaceContributionBasis {
            checked_plot_id: local.mask.plot_identity.checked_plot_id,
            plan_id: realization.plan.plan_id,
            active_play_id: active.active_play_id,
            required_interaction_context: None,
        })
        .unwrap();
    let face = BodyFace::project(
        &session.evidence().body,
        Some(&session.realization().unwrap().wake),
        REVISION,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![FaceContribution::from_face(
            FaceContributionRole::Foreground,
            fragment,
        )],
    )
    .unwrap()
    .face;
    assert!(face.subjects.iter().any(|s| s.role == FaceRole::Body));
    assert!(!face.actions.is_empty());
    let line = |name: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            name,
            &format!("binding/{name}"),
            "conduit.base/websocket-rfc6455@1".into(),
            "fixture/history-owner-entry",
            source,
            sink,
            LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64 * 1024,
                maximum_buffered_bytes: 256 * 1024,
                maximum_frame_bytes: 64 * 1024,
            },
        );
        line.contract = conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0].contract;
        line.binding.credential =
            LinkCredentialReference::Opaque("credential/history-owner-entry".into());
        line.binding.authority =
            LinkAuthorityReference::Grant("authority/history-owner-entry".into());
        line
    };
    let sent = line("history-face", &owner, &browser);
    let returned = line("history-show", &browser, &owner);
    let interaction = line("history-interaction", &browser, &owner);
    let planned = conduit_browser_mask_offer::planned_owner_face_show_interaction_mask(
        &browser,
        &owner,
        &sent,
        &returned,
        &interaction,
        HISTORY_COMPONENT_MASK_SOURCE,
        HISTORY_COMPONENT_MASK_ROOT,
    )
    .unwrap();
    let route = RemoteOwnerMaskRouteSeal::seal_current_with_interaction(
        &session,
        &face,
        &owner,
        &browser,
        &planned,
        &sent,
        &returned,
        &interaction,
    )
    .unwrap();
    let basis = serde_json::json!({"body_id":body.body_id,"host_id":browser.host_id,"boot_id":browser.boot_id});
    (basis, face, route)
}
fn input(bytes: &[u8]) {
    assert!(bytes.len() <= abi::conduit_browser_owner_face_input_capacity());
    // The existing ABI buffer belongs to this test thread and is not borrowed
    // across a call. All writes stay within the reported finite extent.
    unsafe {
        std::slice::from_raw_parts_mut(
            abi::conduit_browser_owner_face_input_ptr() as *mut u8,
            bytes.len(),
        )
        .copy_from_slice(bytes);
    }
}
fn output() -> serde_json::Value {
    // Decode before the next ABI call may replace this thread's output buffer.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            abi::conduit_browser_owner_face_output_ptr() as *const u8,
            abi::conduit_browser_owner_face_output_len(),
        )
    };
    serde_json::from_slice(bytes).unwrap()
}
fn prepare_frame(basis: &serde_json::Value, face: &Face, route: &RemoteOwnerMaskRouteSeal) -> i32 {
    let basis = serde_json::to_vec(basis).unwrap();
    let frame = serde_json::to_vec(&serde_json::json!({"kind":"face-snapshot-response","protocol":1,
        "response":OwnerFaceSnapshotResponse::Snapshot {schema:OWNER_FACE_RESPONSE_SCHEMA.into(),
            face:Box::new(face.clone()),interactions_admitted:true,route:Some(Box::new(route.clone()))}})).unwrap();
    assert!(
        frame.len() <= MAX_OWNER_FACE_RESPONSE_BYTES,
        "actual owner frame {} exceeds its unchanged bound",
        frame.len()
    );
    let mut bytes = basis.clone();
    bytes.extend_from_slice(&frame);
    input(&bytes);
    abi::conduit_browser_owner_face_prepare(basis.len(), frame.len())
}
#[test]
fn complete_body_history_frame_reaches_actual_browser_abi_and_preserves_context_actions() {
    abi::conduit_browser_owner_face_clear();
    let (basis, face, route) = fixture();
    assert_eq!(prepare_frame(&basis, &face, &route), 0);
    let prepared = output();
    assert_eq!(prepared["face_id"], face.identity.as_str());
    assert_eq!(
        prepared["component"]["recipe_identity"],
        HISTORY_COMPONENT_RECIPE_ID
    );
    assert!(prepared["component"]["fallback"].is_null());
    assert_eq!(
        prepared["component"]["history"]["samples"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        prepared["actions"].as_array().unwrap().len(),
        face.actions.len()
    );
    assert!(
        prepared["subjects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["role"] == "Body")
    );
    assert_eq!(prepared["show_state"], "prepared");
    let ack = serde_json::to_vec(&serde_json::json!({"show_id":prepared["show_id"],
        "face_id":prepared["face_id"],"face_revision":prepared["face_revision"]}))
    .unwrap();
    input(&ack);
    assert_eq!(abi::conduit_browser_owner_face_ack(ack.len()), 0);
    let available = output();
    assert_eq!(available["show_state"], "available");
    export("history-body-view.json", &available);
    export("history-body-face.json", &face);
    assert_eq!(abi::conduit_browser_owner_face_canonical_snapshot(), 0);
    assert_eq!(output()["face"], serde_json::to_value(&face).unwrap());
    let mut stale = face.clone();
    stale.revision += 1;
    stale.reseal_prepared().unwrap();
    assert_ne!(prepare_frame(&basis, &stale, &route), 0);
    assert_eq!(abi::conduit_browser_owner_face_canonical_snapshot(), 0);
    assert_eq!(output()["face"], serde_json::to_value(&face).unwrap());
    abi::conduit_browser_owner_face_clear();
}
