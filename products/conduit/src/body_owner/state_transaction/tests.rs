use super::*;
use conduit_body::{
    AdmissionManager, AuthenticatedHostObservation, Body, BodyLifecycleSession, BodyMembership,
    BodyPlotPlan, MembershipProofId, PartId, ResidentPlot,
};
use conduit_core::{seal_plan, BootId, HostId, OfferGeneration, PlotIdentity};

fn view(transaction: &Transaction) -> View<'_> {
    View {
        schema: &transaction.schema,
        biography: &transaction.biography,
        installation: &transaction.installation,
        last_execution: transaction.last_execution.as_ref(),
        execution_custody: transaction.execution_custody.as_ref(),
        admissions: transaction.admissions.as_ref(),
        admission_custody: transaction.admission_custody.as_ref(),
        source: transaction.source.as_deref(),
        workset_sources: transaction.workset_sources.as_ref(),
        archives: &transaction.archives,
    }
}

fn require_exact_bytes(transaction: &Transaction) {
    let borrowed = view(transaction);
    assert!(core::ptr::eq(borrowed.biography, &transaction.biography));
    assert!(core::ptr::eq(
        borrowed.installation,
        &transaction.installation
    ));
    assert_eq!(borrowed.archives.as_ptr(), transaction.archives.as_ptr());
    for pretty in [false, true] {
        let (owned, borrowed) = if pretty {
            (
                serde_json::to_vec_pretty(transaction).unwrap(),
                serde_json::to_vec_pretty(&borrowed).unwrap(),
            )
        } else {
            (
                serde_json::to_vec(transaction).unwrap(),
                serde_json::to_vec(&borrowed).unwrap(),
            )
        };
        assert_eq!(owned, borrowed);
        assert_eq!(digest(&owned), digest(&borrowed));
        let decoded: Transaction = serde_json::from_slice(&borrowed).unwrap();
        assert_eq!(
            serde_json::to_vec(&decoded).unwrap(),
            serde_json::to_vec(transaction).unwrap()
        );
    }
}

#[test]
fn borrowed_transaction_preserves_null_fields_and_original_recovery() {
    let (root, biography) = super::super::execution_custody_tests::fixture();
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography,
        installation: read_installation(&root.join("installation.json")).unwrap(),
        last_execution: None,
        execution_custody: None,
        admissions: None,
        admission_custody: None,
        source: None,
        workset_sources: None,
        archives: Vec::new(),
    };
    require_exact_bytes(&transaction);
    let bytes = serde_json::to_value(view(&transaction)).unwrap();
    for field in [
        "last_execution",
        "execution_custody",
        "admissions",
        "admission_custody",
        "source",
        "workset_sources",
    ] {
        assert_eq!(bytes[field], serde_json::Value::Null);
    }
    write_json_atomic(
        &root.join("body/owner-transaction.json"),
        &view(&transaction),
    )
    .unwrap();
    recover(&root).unwrap();
    assert_eq!(load(&root).unwrap().as_ref(), Some(&transaction.biography));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn borrowed_transaction_preserves_populated_receipt_and_custody_fields() {
    let (root, biography, manager) = super::super::admission_custody_tests::fixture();
    let installation = read_installation(&root.join("installation.json")).unwrap();
    let retirement = manager.prepare_receipt_retirement().unwrap().unwrap();
    let (manager, custody) =
        admission_custody::prepare(&root, &installation.host_id, &manager, &retirement).unwrap();
    let receipt = serde_json::json!({"body_id": biography.body_id, "observation": "quote \" / snowman ☃ / nul \u{0}"});
    let prepared_execution = execution_custody::prepare(
        &root,
        &installation.host_id,
        biography.body_id.as_str(),
        Some(&receipt),
    )
    .unwrap();
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography,
        installation,
        last_execution: Some(receipt),
        execution_custody: Some(prepared_execution),
        admissions: Some(manager),
        admission_custody: Some(custody),
        source: None,
        workset_sources: None,
        archives: Vec::new(),
    };
    require_exact_bytes(&transaction);
    assert!(core::ptr::eq(
        view(&transaction).admissions.unwrap(),
        transaction.admissions.as_ref().unwrap()
    ));
    assert!(core::ptr::eq(
        view(&transaction).last_execution.unwrap(),
        transaction.last_execution.as_ref().unwrap()
    ));
    write_json_atomic(
        &root.join("body/owner-transaction.json"),
        &view(&transaction),
    )
    .unwrap();
    recover(&root).unwrap();
    assert_eq!(
        admissions(&root, &transaction.biography.body_id)
            .unwrap()
            .as_ref(),
        transaction.admissions.as_ref()
    );
    assert_eq!(
        execution(&root).unwrap().as_ref(),
        transaction.last_execution.as_ref()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn borrowed_transaction_preserves_checked_source_and_workset_payloads() {
    const SOURCE: &str =
        "plot retained-clock {\n clock: time/every(2s)\n clock >> face/tick(\"Clock tick\")\n}\n";
    let (root, _) = super::super::execution_custody_tests::fixture();
    let checked = crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let body = Body::born(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
        1,
        "sign/transaction-parity".into(),
    )
    .unwrap();
    let biography = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Clock".into(),
    )
    .unwrap();
    let workset =
        WorksetSources::checked(&biography, vec![(SOURCE.into(), "retained-clock".into())])
            .unwrap();
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography,
        installation: read_installation(&root.join("installation.json")).unwrap(),
        last_execution: None,
        execution_custody: None,
        admissions: Some(AdmissionManager::new(body.body_id).unwrap()),
        admission_custody: None,
        source: Some(SOURCE.as_bytes().to_vec()),
        workset_sources: Some(workset),
        archives: Vec::new(),
    };
    // Both fields are compared independently and together. This is serialization
    // parity only; the existing Source transition policy remains authoritative.
    require_exact_bytes(&transaction);
    assert_eq!(
        view(&transaction).source.unwrap().as_ptr(),
        transaction.source.as_ref().unwrap().as_ptr()
    );
    assert!(core::ptr::eq(
        view(&transaction).workset_sources.unwrap(),
        transaction.workset_sources.as_ref().unwrap()
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn borrowed_transaction_preserves_nonempty_biography_archives() {
    let (root, _) = super::super::execution_custody_tests::fixture();
    let (biography, archives) = archive_fixture();
    assert!(!archives.is_empty());
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography,
        installation: read_installation(&root.join("installation.json")).unwrap(),
        last_execution: None,
        execution_custody: None,
        admissions: None,
        admission_custody: None,
        source: None,
        workset_sources: None,
        archives,
    };
    require_exact_bytes(&transaction);
    fs::remove_dir_all(root).unwrap();
}

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
