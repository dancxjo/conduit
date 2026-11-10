//! Durable custody proof, not authorized adoption or kernel execution.
use super::*;
use conduit_body::{Body, BodyMembership};
const SOURCE: &str = "plot first {\n show: face/text(\"First\")\n \"one\" >> show\n}\nplot second {\n show: face/text(\"Second\")\n \"two\" >> show\n}.";

fn fixture() -> (std::path::PathBuf, BodyBiographyEvidence, WorksetSources) {
    let root = std::env::temp_dir().join(super::super::super::super::fresh_identity(
        "workset-source-journal",
        "test",
    ));
    fs::create_dir_all(&root).unwrap();
    let installation = Installation {
        schema: super::super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/source-workset".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let sources = vec![
        (SOURCE.to_string(), "first".to_string()),
        (SOURCE.to_string(), "second".to_string()),
    ];
    let plots = sources.iter().map(|(text, entry)| {
        let plot = crate::plot_source::parse(text)
            .unwrap()
            .expand_named_for_authoring(entry)
            .unwrap();
        ResidentPlot::new(
            plot.expanded.source_document_id,
            plot.expanded.checked_plot_id,
        )
    });
    let body =
        Body::born_with_plots(BodyWorkset::from_plots(plots).unwrap(), 1, "born".into()).unwrap();
    let biography = BodyBiographyEvidence::born(
        body.clone(),
        BodyMembership::new(body.body_id.clone()).unwrap(),
        "Test".into(),
    )
    .unwrap();
    let sources = WorksetSources::checked(&biography, sources).unwrap();
    (root, biography, sources)
}
fn retain(
    root: &Path,
    biography: &BodyBiographyEvidence,
    sources: &WorksetSources,
) -> Result<(), String> {
    retain_inner(
        root,
        biography,
        None,
        None.into(),
        SourceUpdate::Complete(sources),
        None,
        &[],
    )
}

#[test]
fn complete_source_workset_commits_reopens_and_replays_interrupted_publication() {
    let (root, biography, sources) = fixture();
    retain(&root, &biography, &sources).unwrap();
    assert_eq!(checked(&root, &biography).unwrap().unwrap().len(), 2);
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography: biography.clone(),
        installation: read_installation(&root.join("installation.json")).unwrap(),
        last_execution: None,
        execution_custody: None,
        admissions: None,
        admission_custody: None,
        source: None,
        workset_sources: Some(sources),
        archives: vec![],
    };
    write_json_atomic(&root.join("body/owner-transaction.json"), &transaction).unwrap();
    fs::write(root.join(FILE), b"interrupted").unwrap();
    fs::write(root.join("body/biography.json"), b"interrupted").unwrap();
    recover(&root).unwrap();
    assert_eq!(load(&root).unwrap().unwrap(), biography);
    assert_eq!(checked(&root, &biography).unwrap().unwrap().len(), 2);
    assert!(!root.join("body/owner-transaction.json").exists());
    assert!(super::super::super::retained_source::checked(&root)
        .unwrap_err()
        .contains("no partial selection"));
    // The fixture stages an existing lifecycle removal, not an authorized action.
    // Its complete replacement and biography must survive as the same Body.
    let removed = crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_named_for_authoring("second")
        .unwrap();
    let removed = ResidentPlot::new(
        removed.expanded.source_document_id,
        removed.expanded.checked_plot_id,
    );
    let sign = conduit_core::SignId::from("remove-second");
    let body = biography.body.remove_plot(&removed, sign.clone()).unwrap();
    let mut reduced = biography.clone();
    reduced
        .append_body_workload_events(body, &[(sign, 2)])
        .unwrap();
    let replacement =
        WorksetSources::checked(&reduced, vec![(SOURCE.into(), "first".into())]).unwrap();
    assert!(retain(&root, &reduced, &replacement)
        .unwrap_err()
        .contains("expected current revision"));
    retain_inner(
        &root,
        &reduced,
        None,
        None.into(),
        SourceUpdate::Revision(&replacement, &biography),
        None,
        &[],
    )
    .unwrap();
    let reopened = load(&root).unwrap().unwrap();
    assert_eq!(reopened.body_id, biography.body_id);
    assert_eq!(reopened.membership, biography.membership);
    assert_eq!(reopened.body.workset.plots().len(), 1);
    let checked = super::super::super::retained_source::checked(&root)
        .unwrap()
        .unwrap();
    assert_eq!(checked.expanded.name, "first");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_duplicate_wrong_entry_changed_source_and_overflow_refuse_before_publication() {
    let (root, biography, sources) = fixture();
    retain(&root, &biography, &sources).unwrap();
    let before = fs::read(root.join(FILE)).unwrap();
    let before_biography = fs::read(root.join("body/biography.json")).unwrap();
    for change in 0..6 {
        let mut invalid = sources.clone();
        match change {
            0 => {
                invalid.entries.pop();
            }
            1 => invalid.entries[1].entry = "first".into(),
            2 => invalid.entries[1].entry = "missing".into(),
            3 => invalid.entries[1]
                .source
                .push_str("\n# changed generation\n"),
            4 => {
                invalid.entries[0].source =
                    "x".repeat(super::super::super::MAXIMUM_SOURCE as usize + 1)
            }
            _ => invalid.schema = "unrecognized".into(),
        }
        assert!(
            retain(&root, &biography, &invalid).is_err(),
            "case {change}"
        );
        assert_eq!(fs::read(root.join(FILE)).unwrap(), before);
        assert_eq!(
            fs::read(root.join("body/biography.json")).unwrap(),
            before_biography
        );
        assert!(!root.join("body/owner-transaction.json").exists());
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn existing_complete_custody_cannot_be_silently_dropped_or_replaced_by_one_source() {
    let (root, biography, sources) = fixture();
    retain(&root, &biography, &sources).unwrap();
    let mut corrupted = sources.clone();
    corrupted.entries[1].entry = "first".into();
    write_json_atomic(&root.join(FILE), &corrupted).unwrap();
    assert!(retain_with_source(&root, &biography, None, None, None).is_err());
    assert!(retain_with_source(&root, &biography, None, None, Some(SOURCE.as_bytes())).is_err());
    assert!(!root.join("body/owner-transaction.json").exists());
    // Only an explicitly complete replacement can repair interrupted custody.
    retain(&root, &biography, &sources).unwrap();
    let mut transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography: biography.clone(),
        installation: read_installation(&root.join("installation.json")).unwrap(),
        last_execution: None,
        execution_custody: None,
        admissions: None,
        admission_custody: None,
        source: Some(SOURCE.as_bytes().to_vec()),
        workset_sources: Some(sources),
        archives: vec![],
    };
    assert!(validate_transaction(&root, &transaction)
        .unwrap_err()
        .contains("conflicting"));
    transaction.source = None;
    assert!(validate_transaction(&root, &transaction).is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_competing_or_foreign_revision_basis_cannot_replace_committed_state() {
    let (root, biography, sources) = fixture();
    retain(&root, &biography, &sources).unwrap();
    let removed = crate::plot_source::parse(SOURCE)
        .unwrap()
        .expand_named_for_authoring("second")
        .unwrap();
    let removed = ResidentPlot::new(
        removed.expanded.source_document_id,
        removed.expanded.checked_plot_id,
    );
    let sign = conduit_core::SignId::from("remove-second");
    let body = biography.body.remove_plot(&removed, sign.clone()).unwrap();
    let mut next = biography.clone();
    next.append_body_workload_events(body, &[(sign, 2)])
        .unwrap();
    let replacement =
        WorksetSources::checked(&next, vec![(SOURCE.into(), "first".into())]).unwrap();
    let before = fs::read(root.join("body/biography.json")).unwrap();
    // A copied description changed outside the owner cannot substitute for its
    // exact retained basis, even when Body/workset/revision labels still match.
    let mut foreign = biography.clone();
    foreign.friendly_name = "another retained owner view".into();
    assert!(retain_inner(
        &root,
        &next,
        None,
        None.into(),
        SourceUpdate::Revision(&replacement, &foreign),
        None,
        &[]
    )
    .unwrap_err()
    .contains("stale owner"));
    assert_eq!(fs::read(root.join("body/biography.json")).unwrap(), before);
    retain_inner(
        &root,
        &next,
        None,
        None.into(),
        SourceUpdate::Revision(&replacement, &biography),
        None,
        &[],
    )
    .unwrap();
    let committed = fs::read(root.join("body/biography.json")).unwrap();
    let committed_sources = fs::read(root.join(FILE)).unwrap();
    // Another request approved against the original state has lost the CAS.
    assert!(retain_inner(
        &root,
        &next,
        None,
        None.into(),
        SourceUpdate::Revision(&replacement, &biography),
        None,
        &[]
    )
    .unwrap_err()
    .contains("stale owner"));
    // Unconditional complete publication also cannot rewind the new revision.
    assert!(retain(&root, &biography, &sources)
        .unwrap_err()
        .contains("expected current revision"));
    assert_eq!(
        fs::read(root.join("body/biography.json")).unwrap(),
        committed
    );
    assert_eq!(fs::read(root.join(FILE)).unwrap(), committed_sources);
    assert!(!root.join("body/owner-transaction.json").exists());
    // Once journaled, recovery completes the exact accepted write, rather than
    // repeating the now-stale precondition after a partially installed commit.
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        biography: next.clone(),
        installation: read_installation(&root.join("installation.json")).unwrap(),
        last_execution: None,
        execution_custody: None,
        admissions: None,
        admission_custody: None,
        source: None,
        workset_sources: Some(replacement),
        archives: vec![],
    };
    write_json_atomic(&root.join("body/owner-transaction.json"), &transaction).unwrap();
    fs::write(root.join(FILE), b"interrupted").unwrap();
    recover(&root).unwrap();
    assert_eq!(load(&root).unwrap().unwrap(), next);
    assert_eq!(fs::read(root.join(FILE)).unwrap(), committed_sources);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn captured_inventory_does_not_promote_entry_labels_to_checked_source() {
    let (root, biography, sources) = fixture();
    retain(&root, &biography, &sources).unwrap();
    let mut inventory = captured(&root, &biography).unwrap().unwrap();
    assert_eq!(inventory.entries.len(), 2);
    // The bytes remain the retained Source, but this entry is not present.
    // Acquisition must not pretend it has performed expansion or compilation.
    inventory.entries[1].entry = "missing".into();
    write_json_atomic(&root.join(FILE), &inventory).unwrap();
    let captured = captured(&root, &biography).unwrap().unwrap();
    assert_eq!(captured.entries[1].entry, "missing");
    assert!(captured.expand(&biography).is_err());
    inventory.entries[1].source.push_str("\n# drift");
    write_json_atomic(&root.join(FILE), &inventory).unwrap();
    assert!(super::captured(&root, &biography).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn retained_complete_capture_expands_original_bytes_after_file_replacement() {
    let (root, biography, sources) = fixture();
    retain(&root, &biography, &sources).unwrap();
    let captured = super::super::super::retained_source::capture(&root)
        .unwrap()
        .unwrap();
    fs::write(root.join(FILE), b"interrupted replacement").unwrap();
    let plots = captured.expand().unwrap();
    assert_eq!(plots.len(), 2);
    assert_eq!(plots[0].plot.expanded.name, "first");
    assert_eq!(plots[1].plot.expanded.name, "second");
    assert!(super::super::super::retained_source::capture(&root).is_err());
    assert_eq!(load(&root).unwrap().unwrap(), biography);
    fs::remove_dir_all(root).unwrap();
}
