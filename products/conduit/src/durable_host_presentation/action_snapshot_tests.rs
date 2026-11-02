//! Local authenticated-snapshot custody regression; not TLS or membership proof.
use super::*;
use conduit_core::*;
use conduit_face::{FaceBasis, FaceRole, FaceSubject, RemoteMaskHostBasis, RemoteMaskRouteProfile};

fn host_basis(host: &HostAdvertisement) -> RemoteMaskHostBasis {
    use sha2::Digest;
    RemoteMaskHostBasis {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        offer_digest: sha2::Sha256::digest(serde_json::to_vec(host).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}

fn fixture(revision: u64) -> (Snapshot, OwnerFaceSnapshotRequest, MaskShow) {
    let body: conduit_body::BodyId = serde_json::from_value(json!("body/action-rebind")).unwrap();
    let host = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/guest".into(),
        boot_id: "boot/guest".into(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/mask".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut owner = host.clone();
    owner.host_id = "host/owner".into();
    owner.boot_id = "boot/owner".into();
    let planned = conduit_conduitos_mask_offer::prepare_stage(
        conduit_conduitos_mask_offer::Adapter::Native,
        &host.host_id,
        &host.boot_id,
        1,
        None,
    )
    .unwrap()
    .planned_mask;
    let face = Face::new(
        revision,
        FaceBasis {
            body_id: Some(body.clone()),
            wake_id: None,
            source_document_id: Some(planned.plan.source_document_id.clone()),
            checked_plot_id: Some(planned.plan.checked_plot_id.clone()),
            expanded_plot_id: Some(planned.plan.expanded_plot_id.clone()),
            plan_id: Some(planned.plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "front/snapshot".into(),
            role: FaceRole::Document,
            name: "Snapshot".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let line = |source, sink, suffix: &str| {
        process_owned_line_offer(
            &format!("line/{suffix}"),
            &format!("binding/{suffix}"),
            "conduit.base/websocket-rfc6455@1".into(),
            &format!("base/{suffix}"),
            source,
            sink,
            1,
            65536,
        )
        .admitted_line()
    };
    let route = RemoteOwnerMaskRouteSeal {
        route_plan_id: format!("route/{revision}").as_str().into(),
        profile: RemoteMaskRouteProfile::Native,
        body_id: body.clone(),
        workload_revision: 1,
        face_id: face.identity.clone(),
        face_revision: face.revision,
        face_basis: face.basis.clone(),
        owner_host: host_basis(&owner),
        mask_host: host_basis(&host),
        face_line: line(&owner, &host, "face"),
        return_line: line(&host, &owner, "show"),
        interaction_line: None,
        planned_mask: planned.clone(),
    };
    let request = OwnerFaceSnapshotRequest {
        schema: OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: "credential/guest".into(),
        body_id: body,
        part_id: serde_json::from_value(json!("part/guest")).unwrap(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    };
    let show = MaskShow::prepared(
        &planned,
        &face,
        bind_active_play(
            &planned.plan.plan_id,
            &host.host_id,
            &host.boot_id,
            revision,
        ),
        "front/snapshot".into(),
        "mask/test".into(),
        format!("sign/prepared/{revision}").as_str().into(),
    )
    .unwrap()
    .transition(
        conduit_face::ManifestationLifecycle::Available,
        format!("sign/available/{revision}").as_str().into(),
    )
    .unwrap();
    (
        Snapshot {
            face,
            route,
            interactions_admitted: false,
            document_bytes: 0,
        },
        request,
        show,
    )
}
fn response(snapshot: Snapshot) -> Value {
    json!({"schema":"conduit.body/native-owner-return-response@1", "accepted":true,"code":"accepted",
        "face": OwnerFaceSnapshotResponse::Snapshot { schema: OWNER_FACE_RESPONSE_SCHEMA.into(), face: Box::new(snapshot.face), route: Some(Box::new(snapshot.route)), interactions_admitted: false }})
}
#[test]
fn action_replacement_is_bound_before_return_and_rejects_previous_show() {
    let (old, request, old_show) = fixture(1);
    let (next, _, next_show) = fixture(2);
    let mut admitted = AdmittedSnapshot::bind(&old, &request).unwrap();
    assert!(admitted.validate_show(&old_show).is_ok());
    assert!(admitted.validate_show(&next_show).is_err());
    let document = response(next);
    let returned = bind_action_snapshot(&document, &request, &mut admitted)
        .unwrap()
        .unwrap();
    assert_eq!(returned.face.revision, 2);
    assert_eq!(
        returned.document_bytes,
        serde_json::to_vec(&document).unwrap().len()
    );
    assert!(admitted.validate_show(&next_show).is_ok());
    assert!(admitted.validate_show(&old_show).is_err());
}
#[test]
fn foreign_action_snapshot_cannot_replace_retained_scope() {
    let (old, request, old_show) = fixture(1);
    let (mut next, _, next_show) = fixture(2);
    next.route.mask_host.boot_id = "boot/foreign".into();
    let mut admitted = AdmittedSnapshot::bind(&old, &request).unwrap();
    assert!(bind_action_snapshot(&response(next), &request, &mut admitted).is_err());
    assert!(admitted.validate_show(&old_show).is_ok());
    assert!(admitted.validate_show(&next_show).is_err());
}
