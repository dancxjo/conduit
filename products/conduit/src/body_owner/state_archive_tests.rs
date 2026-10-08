use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyLifecycleSession, BodyMembership, BodyPlotPlan,
    MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::{seal_plan, BootId, HostId, OfferGeneration, PlotIdentity};

fn archive_fixture() -> (BodyBiographyEvidence, Vec<BodyBiographyArchiveSegment>) {
    let host: HostId = "host/archive-test".into();
    let boot: BootId = "boot/archive-test".into();
    let resident = ResidentPlot::new("source/archive".into(), "checked/archive".into());
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "sign/archive-born".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Archive".into()).unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("archive-test").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/archive-admitted".into(),
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
            "sign/archive-present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: resident.source_document_id.clone(),
            checked_plot_id: resident.checked_plot_id.clone(),
            expanded_plot_id: "expanded/archive".into(),
        },
        vec![],
    );
    let partition = BodyPlotPlan {
        plot: resident,
        plan,
    };
    let mut session = BodyLifecycleSession::open_admitted(evidence, &host, &boot).unwrap();
    for _ in 0..8 {
        session
            .propose(vec![partition.clone()], &host, &boot)
            .unwrap();
        if !session.pending_archives().is_empty() {
            return (
                session.evidence().clone(),
                session.pending_archives().to_vec(),
            );
        }
        session.lull(&host, &boot, None).unwrap();
    }
    panic!("fixture did not reach the archive boundary");
}

fn installation(root: &Path) {
    let value = Installation {
        schema: super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/archive-test".into(),
        release_source_identity: "source/archive".into(),
        release_bundle_sha256: digest(b"bundle/archive-test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_todo_checkpoint: None,
    };
    write_json_atomic(&root.join("installation.json"), &value).unwrap();
}

#[test]
fn archive_transaction_replays_before_active_biography_and_refuses_corruption() {
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-archive-test",
        "replay",
    ));
    fs::create_dir_all(&root).unwrap();
    installation(&root);
    let (biography, segments) = archive_fixture();
    assert_eq!(segments.len(), 1);
    retain_with_archives(&root, &biography, &segments, None, None).unwrap();
    assert_eq!(load(&root).unwrap(), Some(biography.clone()));
    let path = archive_path(&root, segments[0].ordinal);
    let bytes = fs::read(&path).unwrap();
    let installation = read_installation(&root.join("installation.json")).unwrap();
    write_json_atomic(
        &root.join("body/owner-transaction.json"),
        &Transaction {
            schema: "conduit.body/owner-transaction@1".into(),
            biography: biography.clone(),
            installation,
            last_execution: None,
            admissions: None,
            source: None,
            archives: segments.clone(),
        },
    )
    .unwrap();
    fs::write(&path, b"interrupted segment").unwrap();
    assert!(recover(&root).is_err());
    fs::remove_file(&path).unwrap();
    recover(&root).unwrap();
    assert_eq!(fs::read(&path).unwrap(), bytes);
    assert_eq!(load(&root).unwrap(), Some(biography));
    fs::write(&path, b"corrupt segment").unwrap();
    assert!(load(&root).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn archive_rejects_missing_predecessor_and_foreign_head_before_publication() {
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "owner-archive-test",
        "refusals",
    ));
    fs::create_dir_all(&root).unwrap();
    installation(&root);
    let (biography, segments) = archive_fixture();
    let mut wrong = segments.clone();
    wrong[0].body_id = Body::born(
        "source/other".into(),
        "checked/other".into(),
        1,
        "sign/other".into(),
    )
    .unwrap()
    .body_id;
    assert!(retain_with_archives(&root, &biography, &wrong, None, None).is_err());
    assert!(!root.join("body/owner-transaction.json").exists());
    retain_with_archives(&root, &biography, &segments, None, None).unwrap();
    fs::remove_file(archive_path(&root, segments[0].ordinal)).unwrap();
    assert!(load(&root).is_err());
    fs::remove_dir_all(root).unwrap();
}
