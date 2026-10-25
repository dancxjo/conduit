//! Existing Body and complete checked Source custody, not a native UI proof.
use super::*;
use std::fs;
const HELLO: &str = "plot hello {\n show: face/text(\"Message\")\n \"Hello.\" >> show\n}.";
fn fixture() -> (Owner, std::path::PathBuf) {
    let (mut owner, root, _, _, _) =
        super::super::creche_worker::tests::fixture_with_source(HELLO, false);
    owner.lull().unwrap();
    owner.persist(&root).unwrap();
    fs::write(root.join("body/source.conduit"), HELLO).unwrap();
    (owner, root)
}
#[test]
fn admits_once_preserves_primary_source_body_member_and_all_partitions() {
    let (mut owner, root) = fixture();
    let before = owner.session.evidence().clone();
    let resident = owner.resident.clone();
    let host = owner.host.advertisement().clone();
    let revision = before.body.workload_revision;
    let creche = owner.admit_shipped_creche(&root, revision).unwrap();
    assert_eq!(owner.session.evidence().body_id, before.body_id);
    assert_eq!(owner.session.evidence().membership, before.membership);
    assert_eq!(owner.resident, resident);
    assert_eq!(owner.host.advertisement(), &host);
    assert_eq!(
        fs::read(root.join("body/source.conduit")).unwrap(),
        HELLO.as_bytes()
    );
    assert_eq!(owner.session.evidence().body.workset.len(), 2);
    assert!(owner
        .session
        .evidence()
        .body
        .workset
        .contains(resident.as_ref().unwrap()));
    let admitted = owner.session.evidence().clone();
    owner
        .admit_shipped_creche(&root, admitted.body.workload_revision)
        .unwrap();
    assert_eq!(owner.session.evidence(), &admitted);
    owner.plan_creche_workset(&root).unwrap();
    let proposed = owner.session.realization().unwrap();
    assert_eq!(proposed.plan.plots.len(), 2);
    assert!(proposed.plan.plots.iter().any(|p| p.plot == creche));
    assert!(proposed
        .plan
        .plots
        .iter()
        .any(|p| Some(&p.plot) == resident.as_ref()));
    assert!(proposed.play.is_none());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn missing_or_mutated_member_source_never_silently_shrinks_the_workset() {
    let (mut owner, root) = fixture();
    let revision = owner.session.evidence().body.workload_revision;
    owner.admit_shipped_creche(&root, revision).unwrap();
    let retained = owner.session.evidence().clone();
    let path = fs::read_dir(root.join("body/sources"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&path, HELLO).unwrap();
    assert!(owner.plan_creche_workset(&root).is_err());
    assert_eq!(owner.session.evidence(), &retained);
    assert!(owner.session.realization().is_none());
    fs::remove_file(path).unwrap();
    assert!(owner.plan_creche_workset(&root).is_err());
    assert_eq!(owner.session.evidence(), &retained);
    assert_eq!(retained.body.workset.len(), 2);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn stale_or_active_admission_is_atomic_and_primary_source_is_untouched() {
    let (mut owner, root) = fixture();
    let before = owner.truth();
    let revision = owner.session.evidence().body.workload_revision;
    assert!(owner.admit_shipped_creche(&root, revision + 1).is_err());
    assert_eq!(owner.truth(), before);
    assert!(!root.join("body/sources").exists());
    let checked = crate::plot_source::parse(HELLO)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    owner.plan(&checked).unwrap();
    let proposed = owner.truth();
    assert!(owner.admit_shipped_creche(&root, revision).is_err());
    assert_eq!(owner.truth(), proposed);
    assert_eq!(
        fs::read(root.join("body/source.conduit")).unwrap(),
        HELLO.as_bytes()
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn failed_biography_write_blocks_execution_even_before_any_prior_play() {
    let (mut owner, root) = fixture();
    let revision = owner.session.evidence().body.workload_revision;
    let blocked = root.join("body/owner-transaction.json");
    fs::create_dir(&blocked).unwrap();
    assert!(owner.admit_shipped_creche(&root, revision).is_err());
    assert!(owner.execution_persistence_pending);
    assert!(owner.ensure_execution_retained(&root).is_err());
    assert!(owner.execution_persistence_pending);
    assert_eq!(owner.session.evidence().body.workset.len(), 1);
    assert!(owner.last_execution.is_none());
    fs::remove_dir(blocked).unwrap();
    owner.admit_shipped_creche(&root, revision).unwrap();
    assert!(!owner.execution_persistence_pending);
    assert_eq!(owner.session.evidence().body.workset.len(), 2);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn checked_identity_can_select_another_plot_from_the_same_primary_source() {
    let text = format!(
        "{}\nplot spare_greeting {{\n show: face/text(\"Second\")\n \"Two.\" >> show\n}}.\n",
        HELLO.trim_end_matches('.')
    );
    let (mut owner, root, _, _, _) =
        super::super::creche_worker::tests::fixture_with_source(&text, false);
    owner.lull().unwrap();
    owner.persist(&root).unwrap();
    fs::write(root.join("body/source.conduit"), &text).unwrap();
    let source = crate::plot_source::parse(&text).unwrap();
    let checked = source.check().unwrap();
    let first = checked.plots.iter().find(|p| p.name == "hello").unwrap();
    let resident = ResidentPlot::new(
        checked.source_document_id.clone(),
        first.checked_plot_id.clone(),
    );
    let host = owner.host.advertisement().clone();
    let revision = owner.session.evidence().body.workload_revision;
    owner
        .session
        .admit_plot(revision, resident.clone(), &host.host_id, &host.boot_id)
        .unwrap();
    owner.persist(&root).unwrap();
    let revision = owner.session.evidence().body.workload_revision;
    owner.admit_shipped_creche(&root, revision).unwrap();
    owner.plan_creche_workset(&root).unwrap();
    assert_eq!(owner.session.realization().unwrap().plan.plots.len(), 3);
    assert!(owner
        .session
        .realization()
        .unwrap()
        .plan
        .plots
        .iter()
        .any(|p| p.plot == resident));
    assert_eq!(
        fs::read_to_string(root.join("body/source.conduit")).unwrap(),
        text
    );
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn another_activation_needs_its_real_owner_before_any_proposal() {
    let (mut owner, root) = fixture();
    let todo = include_str!("../../../../../plots/todo/live.conduit");
    let checked = crate::plot_source::parse(todo)
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
    );
    super::super::retained_sources::retain(&root, &resident, todo).unwrap();
    let host = owner.host.advertisement().clone();
    owner
        .session
        .admit_plot(
            owner.session.evidence().body.workload_revision,
            resident,
            &host.host_id,
            &host.boot_id,
        )
        .unwrap();
    owner.persist(&root).unwrap();
    let revision = owner.session.evidence().body.workload_revision;
    owner.admit_shipped_creche(&root, revision).unwrap();
    let before = owner.session.evidence().clone();
    assert!(owner
        .plan_creche_workset(&root)
        .unwrap_err()
        .contains("activation"));
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(before.body.workset.len(), 3);
    assert!(owner.session.realization().is_none());
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn source_directory_symlink_cannot_substitute_a_member_source() {
    let (mut owner, root) = fixture();
    let revision = owner.session.evidence().body.workload_revision;
    owner.admit_shipped_creche(&root, revision).unwrap();
    let before = owner.session.evidence().clone();
    let original = root.join("body/sources");
    let other = root.join("unadmitted-sources");
    fs::rename(&original, &other).unwrap();
    std::os::unix::fs::symlink(&other, &original).unwrap();
    assert!(owner.plan_creche_workset(&root).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert!(owner.session.realization().is_none());
    fs::remove_dir_all(root).unwrap();
}
