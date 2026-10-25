//! Actual owner response/ABI entrance with lifecycle and Line CPU fixtures.
//! No socket, existing-device membership, sensor Play or physical Show is claimed.
use super::*;
use conduit_body::*;
use conduit_core::*;
use conduit_face::*;

fn fixture() -> (serde_json::Value, Face, RemoteOwnerMaskRouteSeal) {
    fixture_with_look(None)
}
fn fixture_with_look(
    look: Option<&LookInfo>,
) -> (serde_json::Value, Face, RemoteOwnerMaskRouteSeal) {
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
    let planned = match look {
        None => planned,
        Some(look) => {
            conduit_browser_mask_offer::planned_owner_face_show_interaction_mask_with_look(
                &browser,
                &owner,
                &sent,
                &returned,
                &interaction,
                HISTORY_COMPONENT_MASK_SOURCE,
                HISTORY_COMPONENT_MASK_ROOT,
                look,
            )
            .unwrap()
        }
    };
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

fn prepare_selected_frame(
    basis: &serde_json::Value,
    face: &Face,
    route: &RemoteOwnerMaskRouteSeal,
    look: &[u8],
) -> i32 {
    let basis = serde_json::to_vec(basis).unwrap();
    let frame = serde_json::to_vec(&serde_json::json!({"kind":"face-snapshot-response","protocol":1,
        "response":OwnerFaceSnapshotResponse::Snapshot {schema:OWNER_FACE_RESPONSE_SCHEMA.into(),
            face:Box::new(face.clone()),interactions_admitted:true,route:Some(Box::new(route.clone()))}})).unwrap();
    assert!(frame.len() <= MAX_OWNER_FACE_RESPONSE_BYTES);
    let mut bytes = basis.clone();
    bytes.extend_from_slice(&frame);
    input(&bytes);
    assert!(look.len() <= abi::conduit_browser_owner_face_history_look_input_capacity());
    // This test owns the current thread's finite ABI buffers.
    unsafe {
        std::slice::from_raw_parts_mut(
            abi::conduit_browser_owner_face_history_look_input_ptr() as *mut u8,
            look.len(),
        )
        .copy_from_slice(look);
    }
    abi::conduit_browser_owner_face_prepare_with_history_look(basis.len(), frame.len(), look.len())
}
fn acknowledge_current() {
    let prepared = output();
    let ack = serde_json::to_vec(&serde_json::json!({"show_id":prepared["show_id"],"face_id":prepared["face_id"],"face_revision":prepared["face_revision"]})).unwrap();
    input(&ack);
    assert_eq!(abi::conduit_browser_owner_face_ack(ack.len()), 0);
}
#[test]
fn independent_history_look_abi_reproduces_route_and_preserves_acknowledged_show_on_refusal() {
    use conduit_plot::rust_binding::NativeRustBinding;
    abi::conduit_browser_owner_face_clear();
    let (basis, face, route) = fixture();
    assert_eq!(prepare_frame(&basis, &face, &route), 0);
    acknowledge_current();
    let base = editorial_look().unwrap();
    for (unit, label) in [
        (LookTemperatureUnit::Celsius, "°C"),
        (LookTemperatureUnit::Kelvin, "K"),
        (LookTemperatureUnit::Fahrenheit, "°F"),
    ] {
        let look = LookInfo::new(
            base.generative().clone(),
            base.identity().into(),
            LookQuantityFacetOption::some(unit).unwrap(),
            base.spoken().clone(),
            3,
            base.visual().clone(),
        )
        .unwrap();
        let bytes = look.clone().encode().unwrap();
        let (selected_basis, selected_face, selected_route) = fixture_with_look(Some(&look));
        assert_eq!(selected_face, face);
        let old_show = output()["show_id"].clone();
        assert_ne!(prepare_frame(&selected_basis, &face, &selected_route), 0);
        assert_ne!(
            prepare_selected_frame(&selected_basis, &face, &selected_route, b"invalid"),
            0
        );
        let wrong = base.clone().encode().unwrap();
        assert_ne!(
            prepare_selected_frame(&selected_basis, &face, &selected_route, &wrong),
            0
        );
        // An unchanged response observes current acknowledged material even after refusals.
        let unchanged = serde_json::to_vec(&serde_json::json!({"kind":"face-snapshot-response","protocol":1,"response":OwnerFaceSnapshotResponse::Unchanged{schema:OWNER_FACE_RESPONSE_SCHEMA.into(),revision:face.revision,identity:face.identity.clone()}})).unwrap();
        if unit == LookTemperatureUnit::Celsius {
            let basis_bytes = serde_json::to_vec(&basis).unwrap();
            let mut input_bytes = basis_bytes.clone();
            input_bytes.extend_from_slice(&unchanged);
            input(&input_bytes);
            assert_eq!(
                abi::conduit_browser_owner_face_prepare(basis_bytes.len(), unchanged.len()),
                0
            );
            assert_eq!(output()["show_id"], old_show);
            assert_eq!(output()["show_state"], "available");
        }
        assert_eq!(
            prepare_selected_frame(&selected_basis, &face, &selected_route, &bytes),
            0
        );
        assert_eq!(output()["component"]["history"]["temperature_unit"], label);
        acknowledge_current();
        let acknowledged = output()["show_id"].clone();
        let basis_bytes = serde_json::to_vec(&selected_basis).unwrap();
        let mut input_bytes = basis_bytes.clone();
        input_bytes.extend_from_slice(&unchanged);
        input(&input_bytes);
        assert_ne!(
            abi::conduit_browser_owner_face_prepare(basis_bytes.len(), unchanged.len()),
            0
        );
        input(&input_bytes);
        unsafe {
            std::slice::from_raw_parts_mut(
                abi::conduit_browser_owner_face_history_look_input_ptr() as *mut u8,
                bytes.len(),
            )
            .copy_from_slice(&bytes);
        }
        assert_eq!(
            abi::conduit_browser_owner_face_prepare_with_history_look(
                basis_bytes.len(),
                unchanged.len(),
                bytes.len()
            ),
            0
        );
        assert_eq!(output()["show_id"], acknowledged);
        assert_eq!(output()["show_state"], "available");
    }
    abi::conduit_browser_owner_face_clear();
}

#[test]
fn history_abi_rejects_input_pressure_and_foreign_root_without_retiring_current_show() {
    use conduit_plot::rust_binding::NativeRustBinding;
    abi::conduit_browser_owner_face_clear();
    let (basis, face, route) = fixture();
    assert_eq!(prepare_frame(&basis, &face, &route), 0);
    acknowledge_current();
    let show = output()["show_id"].clone();
    let look = editorial_look().unwrap();
    let encoded = look.clone().encode().unwrap();
    for length in [
        0,
        abi::conduit_browser_owner_face_history_look_input_capacity() + 1,
    ] {
        assert_ne!(
            abi::conduit_browser_owner_face_prepare_with_history_look(1, 1, length),
            0
        );
    }
    assert_ne!(
        abi::conduit_browser_owner_face_prepare(1, MAX_OWNER_FACE_RESPONSE_BYTES + 1),
        0
    );
    let host_basis: HostBasis = serde_json::from_value(basis.clone()).unwrap();
    let host = crate::installed_browser::membership_advertisement(
        host_basis.host_id.clone(),
        host_basis.boot_id.clone(),
    );
    let ordinary = conduit_browser_mask_offer::planned_mask(
        &host,
        conduit_browser_mask_offer::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    assert!(
        OwnerBrowserMask::prepare_planned_with_look(
            host_basis,
            face.clone(),
            ordinary,
            None,
            None,
            99,
            true,
            Some(&look)
        )
        .is_err()
    );
    // A wrong Root at the public ABI refuses even before its stale route seal
    // could be used; the typed internal entrance above uses a real ordinary Plan.
    let mut foreign = route.clone();
    foreign.planned_mask.mask.plot_name = "browser-graphical".into();
    assert_ne!(prepare_selected_frame(&basis, &face, &foreign, &encoded), 0);
    let unchanged = serde_json::to_vec(&serde_json::json!({"kind":"face-snapshot-response","protocol":1,"response":OwnerFaceSnapshotResponse::Unchanged{schema:OWNER_FACE_RESPONSE_SCHEMA.into(),revision:face.revision,identity:face.identity.clone()}})).unwrap();
    let basis_bytes = serde_json::to_vec(&basis).unwrap();
    let mut bytes = basis_bytes.clone();
    bytes.extend_from_slice(&unchanged);
    input(&bytes);
    assert_eq!(
        abi::conduit_browser_owner_face_prepare(basis_bytes.len(), unchanged.len()),
        0
    );
    assert_eq!(output()["show_id"], show);
    assert_eq!(output()["show_state"], "available");
    abi::conduit_browser_owner_face_clear();
}

#[test]
fn history_startup_reproduction_refuses_a_validly_resealed_changed_interaction_queue() {
    let look = editorial_look().unwrap();
    let (basis, face, route) = fixture_with_look(Some(&look));
    let mut planned = route.planned_mask.clone();
    let interaction = planned.mask.interaction_output.front_port_id.clone();
    let port = planned
        .plan
        .fragments
        .iter_mut()
        .flat_map(|f| &mut f.fore_ports)
        .find(|p| p.front_port_id == interaction && p.direction == PortDirection::Output)
        .unwrap();
    assert!(port.byte_capacity > 1);
    port.byte_capacity -= 1;
    let plan = &planned.plan;
    planned.plan = seal_plan_with_activation_entries(
        PlotIdentity {
            source_document_id: plan.source_document_id.clone(),
            checked_plot_id: plan.checked_plot_id.clone(),
            expanded_plot_id: plan.expanded_plot_id.clone(),
        },
        plan.completion_policy,
        plan.realization_backs.clone(),
        plan.activations.clone(),
        plan.fragments.clone(),
    );
    assert!(verify_plan(&planned.plan));
    let planned = PlannedMaskPlot::admit(&planned.mask, &planned.plan).unwrap();
    assert!(
        OwnerBrowserMask::prepare_planned_with_look(
            serde_json::from_value(basis).unwrap(),
            face,
            planned,
            None,
            Some((route.face_line, route.return_line, route.interaction_line)),
            99,
            true,
            Some(&look)
        )
        .is_err()
    );
}
