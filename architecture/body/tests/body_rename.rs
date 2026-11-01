#[path = "../../plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_body::*;
use conduit_core::{seal_plan, BootId, HostId, OfferGeneration, PlotIdentity};
use ed25519_dalek::{Signer, SigningKey};
fn fixture() -> (BodyLifecycleSession, BodyPlotPlan, HostId, BootId) {
    let host: HostId = "host/local".into();
    let boot: BootId = "boot/local".into();
    let resident = ResidentPlot::new("source/work".into(), "checked/work".into());
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "born".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Test".into()).unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("local-birth").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "admitted".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: resident.source_document_id.clone(),
            checked_plot_id: resident.checked_plot_id.clone(),
            expanded_plot_id: "expanded/work".into(),
        },
        vec![],
    );
    (
        BodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap(),
        BodyPlotPlan {
            plot: resident,
            plan,
        },
        host,
        boot,
    )
}

fn request(
    session: &BodyLifecycleSession,
    host: &HostId,
    boot: &BootId,
    name: &str,
) -> BodyRenameRequest {
    let mut r = BodyRenameRequest {
        schema: BODY_RENAME_REQUEST_SCHEMA.into(),
        body_id: session.evidence().body_id.clone(),
        host_id: host.clone(),
        boot_id: boot.clone(),
        expected_sequence: session.evidence().last_sequence(),
        nonce: [23; 32],
        new_name: name.into(),
        attribution: "House operator".into(),
        signature: vec![],
    };
    r.signature = SigningKey::from_bytes(&[7; 32])
        .sign(&r.transcript())
        .to_bytes()
        .to_vec();
    r
}
fn authorized(r: &BodyRenameRequest) -> AuthorizedBodyRename {
    r.authorize(&SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes())
        .unwrap()
}
#[test]
fn rename_preserves_body_plan_and_play_and_reopens() {
    let (mut s, plan, host, boot) = fixture();
    s.propose(vec![plan], &host, &boot).unwrap();
    let current = s.realization().unwrap();
    let play = BodyPlayIdentity::bind(&current.plan, 1);
    let evidence_sign = |sequence| {
        conduit_core::bind_sign(&host, &boot, Some(&play.active_play_id), sequence).sign_id
    };
    let wake = current
        .wake
        .body_plan_ready(&current.plan, evidence_sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, evidence_sign(1))
        .unwrap();
    s.started(&host, &boot, play, wake).unwrap();
    let body = s.evidence().body.clone();
    let membership = s.evidence().membership.clone();
    let realization = s.realization().cloned();
    let r = request(&s, &host, &boot, "Hamm Creek House");
    let result = s.rename(&authorized(&r), &host, &boot).unwrap();
    assert!(result.changed);
    assert_eq!(s.evidence().body, body);
    assert_eq!(s.evidence().membership, membership);
    assert_eq!(s.realization(), realization.as_ref());
    let restored: BodyBiographyEvidence =
        serde_json::from_slice(&serde_json::to_vec(s.evidence()).unwrap()).unwrap();
    restored.validate().unwrap();
    assert_eq!(restored.friendly_name, "Hamm Creek House");
    assert_eq!(restored.schema, "conduit.body/biography-evidence@3");
    assert!(
        matches!(&restored.records.last().unwrap().kind, BodyBiographyRecordKind::Renamed { previous_name, new_name, .. } if previous_name == "Test" && new_name == "Hamm Creek House")
    );
    let before = s.evidence().clone();
    assert!(!s.rename(&authorized(&r), &host, &boot).unwrap().changed);
    assert_eq!(s.evidence(), &before);
}
#[test]
fn denied_stale_wrong_body_and_malformed_do_not_mutate() {
    let (mut s, _, host, boot) = fixture();
    let mut r = request(&s, &host, &boot, "Hamm Creek House");
    assert_eq!(
        r.authorize(&SigningKey::from_bytes(&[9; 32]).verifying_key().to_bytes())
            .unwrap_err(),
        BodyRenameRefusal::Denied
    );
    let before = s.evidence().clone();
    let mut stale = r.clone();
    stale.expected_sequence -= 1;
    stale.signature = SigningKey::from_bytes(&[7; 32])
        .sign(&stale.transcript())
        .to_bytes()
        .to_vec();
    assert_eq!(
        s.rename(&authorized(&stale), &host, &boot),
        Err(BodyRenameRefusal::Stale)
    );
    assert_eq!(
        s.rename(&authorized(&r), &host, &BootId::new("other")),
        Err(BodyRenameRefusal::Host)
    );
    r.new_name = "\n".into();
    assert_eq!(
        r.authorize(&[0; 32]).unwrap_err(),
        BodyRenameRefusal::InvalidRequest
    );
    assert_eq!(s.evidence(), &before);
}
#[test]
fn old_state_loads_without_synthetic_rename_and_same_name_is_noop() {
    let (mut s, _, host, boot) = fixture();
    let before = s.evidence().clone();
    let r = request(&s, &host, &boot, "Test");
    assert!(!s.rename(&authorized(&r), &host, &boot).unwrap().changed);
    assert_eq!(s.evidence(), &before);
    let reopened = BodyLifecycleSession::open(before).unwrap();
    assert_eq!(
        reopened.evidence().schema,
        "conduit.body/biography-evidence@2"
    );
}
#[test]
fn archived_names_preserve_latest_and_reject_tampering() {
    let (mut s, _, host, boot) = fixture();
    for name in ["First", "Second", "Hamm Creek House"] {
        let r = request(&s, &host, &boot, name);
        s.rename(&authorized(&r), &host, &boot).unwrap();
    }
    let mut e = s.evidence().clone();
    let segment = e.seal_name_history().unwrap().unwrap();
    segment.validate_as_head_of(&e).unwrap();
    assert_eq!(segment.records.len(), 2);
    assert_eq!(
        e.records
            .iter()
            .filter(|r| matches!(r.kind, BodyBiographyRecordKind::Renamed { .. }))
            .count(),
        1
    );
    assert_eq!(e.friendly_name, "Hamm Creek House");
    BodyLifecycleSession::open(e.clone()).unwrap();
    e.friendly_name = "forged".into();
    assert!(e.validate().is_err());
}
#[test]
fn rename_history_pressure_is_explicit_and_archivable() {
    let (mut s, _, host, boot) = fixture();
    for i in 0..MAX_BODY_BIOGRAPHY_RECORDS - s.evidence().records.len() {
        let r = request(&s, &host, &boot, &format!("House {i}"));
        s.rename(&authorized(&r), &host, &boot).unwrap();
    }
    let before = s.evidence().clone();
    let r = request(&s, &host, &boot, "Hamm Creek House");
    assert_eq!(
        s.rename(&authorized(&r), &host, &boot),
        Err(BodyRenameRefusal::Capacity)
    );
    assert_eq!(s.evidence(), &before);
    let mut compact = before;
    compact.seal_name_history().unwrap().unwrap();
    let mut reopened = BodyLifecycleSession::open(compact).unwrap();
    let r = request(&reopened, &host, &boot, "Hamm Creek House");
    reopened.rename(&authorized(&r), &host, &boot).unwrap();
}

#[test]
fn signed_receipt_binds_root_body_name_and_request() {
    let (mut session, _, host, boot) = fixture();
    let r = request(&session, &host, &boot, "Hamm Creek House");
    let result = session.rename(&authorized(&r), &host, &boot).unwrap();
    let root = SigningKey::from_bytes(&[11; 32]);
    let mut receipt = BodyRenameReceipt {
        schema: "conduit.body/rename-receipt@1".into(),
        result,
        root_key: root.verifying_key().to_bytes(),
        signature: vec![],
    };
    receipt.signature = root
        .sign(&receipt.transcript().unwrap())
        .to_bytes()
        .to_vec();
    receipt
        .verify(&root.verifying_key().to_bytes(), &r)
        .unwrap();
    let (_, allocations) = allocation_probe::observe(|| receipt.transcript().unwrap());
    assert_eq!(allocations.allocations + allocations.reallocations, 0);
    let mut fresh = r.clone();
    fresh.nonce[0] ^= 1;
    fresh.sign(&[7; 32]);
    assert!(receipt
        .verify(&root.verifying_key().to_bytes(), &fresh)
        .is_err());
    let record = session.evidence().records.last().unwrap();
    receipt
        .verify_record(&root.verifying_key().to_bytes(), &r, record)
        .unwrap();
    let mut tampered = record.clone();
    if let BodyBiographyRecordKind::Renamed { previous_name, .. } = &mut tampered.kind {
        *previous_name = "forged old name".into();
    }
    assert!(receipt
        .verify_record(&root.verifying_key().to_bytes(), &r, &tampered)
        .is_err());
    receipt.result.friendly_name = "forged".into();
    assert!(receipt
        .verify(&root.verifying_key().to_bytes(), &r)
        .is_err());
}

#[test]
fn wrong_body_tampered_request_and_fulfilled_body_refuse() {
    let (mut session, _, host, boot) = fixture();
    let before = session.evidence().clone();
    let mut request = request(&session, &host, &boot, "Hamm Creek House");
    request.body_id = Body::born(
        "source/other".into(),
        "checked/other".into(),
        1,
        "born/other".into(),
    )
    .unwrap()
    .body_id;
    request.sign(&[7; 32]);
    assert_eq!(
        session.rename(&authorized(&request), &host, &boot),
        Err(BodyRenameRefusal::WrongBody)
    );
    request.new_name = "tampered".into();
    assert_eq!(
        request
            .authorize(&SigningKey::from_bytes(&[7; 32]).verifying_key().to_bytes())
            .unwrap_err(),
        BodyRenameRefusal::Denied
    );
    assert_eq!(session.evidence(), &before);
    session
        .fulfill(
            &host,
            &boot,
            "authority/operator".into(),
            "House operator".into(),
        )
        .unwrap();
    request.body_id = session.evidence().body_id.clone();
    request.expected_sequence = session.evidence().last_sequence();
    request.sign(&[7; 32]);
    assert_eq!(
        session.rename(&authorized(&request), &host, &boot),
        Err(BodyRenameRefusal::Fulfilled)
    );
}
