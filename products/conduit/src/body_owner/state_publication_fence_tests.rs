use super::*;
use conduit_body::{BodyLifecycleSession, ResidentPlot};
use conduit_std_host::{StdHost, StdHostConfig};
const OLD: &str = "plot old {\n show: face/text(\"Old\")\n \"old\" >> show\n}\n";
const NEW: &str = "plot next {\n show: face/text(\"New\")\n \"new\" >> show\n}\n";
fn resident(source: &str) -> ResidentPlot {
    let checked = crate::plot_source::parse(source)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    ResidentPlot::new(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
    )
}
fn fixture() -> (
    std::path::PathBuf,
    super::super::controller::Owner,
    BodyBiographyEvidence,
) {
    let root = std::env::temp_dir().join(super::super::super::fresh_identity(
        "replace-fence",
        "fixture",
    ));
    fs::create_dir_all(&root).unwrap();
    let installation: Installation = serde_json::from_value(serde_json::json!({
        "schema":super::super::super::INSTALL_SCHEMA,"host_id":"host/replace",
        "release_source_identity":"source/test","release_bundle_sha256":digest(b"bundle/test"),
        "product_executable":"fixture-unused","body_state":null,"joined_body_state":null
    }))
    .unwrap();
    write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let host = StdHost::new_with_config(StdHostConfig {
        host_id: "host/replace".into(),
        boot_id: "boot/replace".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let mut owner =
        super::super::controller::Owner::open(host, resident(OLD), None, "Hamm Creek House")
            .unwrap();
    owner.persist(&root).unwrap();
    let evidence: BodyBiographyEvidence =
        serde_json::from_value(owner.truth()["biography"].clone()).unwrap();
    retain_with_source(&root, &evidence, None, None, Some(OLD.as_bytes())).unwrap();
    (root, owner, evidence)
}
fn replacement(evidence: BodyBiographyEvidence, source: &str) -> BodyLifecycleSession {
    let mut session = BodyLifecycleSession::open_admitted(
        evidence,
        &"host/replace".into(),
        &"boot/replace".into(),
    )
    .unwrap();
    let revision = session.evidence().body.workload_revision;
    let old = session.evidence().body.workset.plots()[0].clone();
    session
        .remove_plot(
            revision,
            &old,
            &"host/replace".into(),
            &"boot/replace".into(),
        )
        .unwrap();
    session
        .admit_plot(
            revision + 1,
            resident(source),
            &"host/replace".into(),
            &"boot/replace".into(),
        )
        .unwrap();
    session
}
#[test]
fn recovered_source_transaction_fences_stale_live_owner_and_older_publication() {
    let (root, mut owner, old) = fixture();
    let staged = replacement(old.clone(), NEW);
    let biography = staged.evidence().clone();
    let mut installation = read_installation(&root.join("installation.json")).unwrap();
    installation.body_state.as_mut().unwrap().biography_sha256 =
        digest(&serde_json::to_vec_pretty(&biography).unwrap());
    let transaction = Transaction {
        schema: "conduit.body/owner-transaction@1".into(),
        root_signer: None,
        biography: biography.clone(),
        installation,
        last_execution: None,
        execution_custody: None,
        admissions: None,
        source: Some(NEW.as_bytes().to_vec()),
        archives: staged.pending_archives().to_vec(),
    };
    write_json_atomic(&root.join("body/owner-transaction.json"), &transaction).unwrap();
    // Interrupted after publication and source write, before biography/binding.
    fs::write(root.join("body/source.conduit"), NEW).unwrap();
    assert!(owner
        .replace_plot(&root, NEW, 0, resident(OLD).checked_plot_id.as_str())
        .unwrap_err()
        .contains("stale"));
    assert!(
        retain_with_source(&root, &old, None, None, Some(OLD.as_bytes()))
            .unwrap_err()
            .contains("stale owner publication")
    );
    assert_eq!(load(&root).unwrap().unwrap(), biography);
    assert_eq!(
        fs::read(root.join("body/source.conduit")).unwrap(),
        NEW.as_bytes()
    );
    assert!(!root.join("body/owner-transaction.json").exists());
    // A valid alternate remove/admit has the same sequence/revision; it is a
    // different payload/workset even if lifecycle Sign IDs are deterministic.
    let other = "plot other {\n show: face/text(\"Other\")\n \"other\" >> show\n}\n";
    let alternate = replacement(old.clone(), other);
    assert_eq!(
        alternate.evidence().last_sequence(),
        biography.last_sequence()
    );
    assert!(retain_with_source(
        &root,
        alternate.evidence(),
        None,
        None,
        Some(other.as_bytes())
    )
    .unwrap_err()
    .contains("same workload revision"));
    let mut stale = old;
    for index in 0..4 {
        let host = StdHost::new_with_config(StdHostConfig {
            host_id: "host/replace".into(),
            boot_id: format!("boot/stale-{index}").into(),
            offer_generation: conduit_core::OfferGeneration(1),
        });
        let owner = super::super::controller::Owner::resume(host, stale).unwrap();
        stale = serde_json::from_value(owner.truth()["biography"].clone()).unwrap();
    }
    assert!(stale.last_sequence() > biography.last_sequence());
    assert!(
        retain_with_source(&root, &stale, None, None, Some(OLD.as_bytes()))
            .unwrap_err()
            .contains("stale owner publication")
    );
    assert_eq!(load(&root).unwrap().unwrap(), biography);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn replacement_rejects_stale_revision_wrong_plot_and_unchanged_source_before_publication() {
    let (root, mut owner, before) = fixture();
    assert!(owner
        .replace_plot(&root, NEW, 1, resident(OLD).checked_plot_id.as_str())
        .is_err());
    assert!(owner.replace_plot(&root, NEW, 0, "wrong/plot").is_err());
    assert!(owner
        .replace_plot(&root, OLD, 0, resident(OLD).checked_plot_id.as_str())
        .is_err());
    assert_eq!(load(&root).unwrap().unwrap(), before);
    assert_eq!(
        fs::read(root.join("body/source.conduit")).unwrap(),
        OLD.as_bytes()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn active_proposal_or_moved_host_cannot_replace_resident_source() {
    let (root, mut owner, before) = fixture();
    let host = owner.host.take_for_play().unwrap();
    assert!(owner
        .replace_plot(&root, NEW, 0, resident(OLD).checked_plot_id.as_str())
        .unwrap_err()
        .contains("no current proposal or Play"));
    owner.host.restore_after_play(host).unwrap();
    let checked = crate::plot_source::parse(OLD)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    owner.plan(&checked).unwrap();
    assert!(owner
        .replace_plot(&root, NEW, 0, resident(OLD).checked_plot_id.as_str())
        .unwrap_err()
        .contains("no current proposal or Play"));
    assert_eq!(load(&root).unwrap().unwrap(), before);
    fs::remove_dir_all(root).unwrap();
}
#[cfg(unix)]
#[test]
fn failed_binding_publication_reports_unknown_and_recovery_never_reverts_source() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let (root, mut owner, _) = fixture();
    // An unprivileged installed owner can lose permission to replace the
    // installation binding after its body-directory journal/source writes.
    if fs::metadata(&root).unwrap().uid() == 0 {
        fs::remove_dir_all(root).unwrap();
        eprintln!("permission-fault proof requires an unprivileged owner");
        return;
    }
    struct Restore(std::path::PathBuf);
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700));
        }
    }
    let restore = Restore(root.clone());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o500)).unwrap();
    let result = owner.replace_plot(&root, NEW, 0, resident(OLD).checked_plot_id.as_str());
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    drop(restore);
    assert!(result.unwrap_err().contains("outcome-unknown"));
    assert!(root.join("body/owner-transaction.json").exists());
    assert_eq!(
        fs::read(root.join("body/source.conduit")).unwrap(),
        NEW.as_bytes()
    );
    assert!(owner
        .replace_plot(&root, OLD, 0, resident(OLD).checked_plot_id.as_str())
        .unwrap_err()
        .contains("stale"));
    let recovered = load(&root).unwrap().unwrap();
    assert_eq!(recovered.body.workset.plots(), &[resident(NEW)]);
    assert_eq!(recovered.friendly_name, "Hamm Creek House");
    assert_eq!(
        fs::read(root.join("body/source.conduit")).unwrap(),
        NEW.as_bytes()
    );
    fs::remove_dir_all(root).unwrap();
}
