//! Host grants remain inert until exact checked selection and ordinary review.
use super::*;
use serde_json::json;
use std::fs;
fn fixture() -> (PathBuf, PathBuf, serde_json::Value) {
    let root = std::env::temp_dir().join(format!(
        "creche-policy-{}",
        crate::birth_identity::fresh_uuid().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("greeting.conduit");
    fs::write(
        &source,
        "plot greeting {\n show: face/text(\"Message\")\n \"Hello.\" >> show\n}.",
    )
    .unwrap();
    let policy = json!({
        "schema":"conduit.install/native-creche-grants@1",
        "construction":fs::canonicalize("../../proof/fixtures/bodies/std-line.body.conduit").unwrap(),
        "first_host":"clock", "starting_sources":[source],
        "reviewed_images":{},"source_builds":{},"staging_directory":root,"save_directory":root,
        "maximum_total_bytes":1024*1024,"maximum_millis":1_800_000,"width":800,"height":600,
    });
    let path = root.join("grants.json");
    (root, path, policy)
}
#[test]
fn actual_source_and_construction_make_an_unselected_inert_draft() {
    let (root, path, value) = fixture();
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let policy = Policy::load(&path).unwrap();
    let draft = policy.draft().unwrap();
    assert_eq!(draft.construction().hosts().len(), 2);
    assert_eq!(draft.birth().choices().len(), 1);
    assert_eq!(draft.birth().choices()[0].title, "greeting");
    assert!(!draft.birth().choices()[0].selected);
    assert!(draft.birth().choices()[0].refusal.is_some());
    assert!(draft
        .review(draft.revision(), &conduit_workspace_make::package_set())
        .is_err());
    assert!(!root.join("birth.json").exists());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn unknown_authority_and_excessive_lifetime_are_refused_without_effects() {
    let (root, path, mut value) = fixture();
    value["run_shell"] = json!("unadmitted-command");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(Policy::load(&path).is_err());
    value.as_object_mut().unwrap().remove("run_shell");
    value["maximum_millis"] = json!(86_400_001_u64);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(Policy::load(&path).is_err());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn metadata_bound_is_checked_before_decode() {
    let root = std::env::temp_dir().join(format!(
        "creche-policy-{}",
        crate::birth_identity::fresh_uuid().unwrap()
    ));
    fs::create_dir(&root).unwrap();
    let path = root.join("grants.json");
    File::create(&path)
        .unwrap()
        .set_len(1024 * 1024 + 1)
        .unwrap();
    assert!(small_file(&path).unwrap_err().contains("bound"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn default_catalogue_keeps_native_eligibility_distinct_from_checked_source_visibility() {
    let (root, path, mut value) = fixture();
    value.as_object_mut().unwrap().remove("starting_sources");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    let draft = Policy::load(&path).unwrap().draft().unwrap();
    let supported =
        conduitos::native_workset::resident(conduitos::native_workset::NativePlot::KeyboardCanvas)
            .unwrap();
    assert!(draft
        .birth()
        .choices()
        .iter()
        .any(|c| c.plot == supported && c.refusal.is_none()));
    assert!(draft.birth().choices().iter().all(|c| !c.selected));
    assert!(draft.birth().choices().iter().any(|c| c.refusal.is_some()));
    fs::remove_dir_all(root).unwrap();
}
