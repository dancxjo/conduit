use super::*;
use crate::durable_host::{digest, write_json_atomic, Installation, INSTALL_SCHEMA};
use conduit_body::{Body, BodyBiographyEvidence, BodyMembership};
use std::{fs, path::PathBuf};

const SOURCE: &str =
    "plot retained-clock {\n clock: time/every(2s)\n clock >> face/tick(\"Clock tick\")\n}\n";
const OTHER: &str =
    "plot another-clock {\n clock: time/every(3s)\n clock >> face/tick(\"Another tick\")\n}\n";

struct Installed(PathBuf);
impl Installed {
    fn new(checked_source: &str) -> Self {
        let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
            "retained-source-policy",
            "installed-body",
        ));
        fs::create_dir_all(&root).unwrap();
        let installation = Installation {
            schema: INSTALL_SCHEMA.into(),
            host_id: "host/source-policy".into(),
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
        let checked = crate::plot_source::parse(checked_source)
            .unwrap()
            .expand_entry_for_authoring()
            .unwrap();
        // A valid biography may carry an exact checked identity different from
        // this file's entry. Content identity alone must never authorize it.
        let body = Body::born(
            canonical_source_document_id(SOURCE),
            checked.expanded.checked_plot_id,
            1,
            conduit_core::bind_sign(
                &"host/source-policy".into(),
                &"boot/source-policy".into(),
                None,
                1,
            )
            .sign_id,
        )
        .unwrap();
        let evidence = BodyBiographyEvidence::born(
            body.clone(),
            BodyMembership::new(body.body_id.clone()).unwrap(),
            "Source policy".into(),
        )
        .unwrap();
        super::super::state::retain(&root, &evidence, None, None).unwrap();
        fs::write(root.join("body/source.conduit"), SOURCE).unwrap();
        Self(root)
    }
}
impl Drop for Installed {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn installed_source_rechecks_exact_resident_without_changing_body() {
    let installed = Installed::new(SOURCE);
    let before = fs::read(installed.0.join("body/biography.json")).unwrap();
    let checked = checked(&installed.0).unwrap().unwrap();
    assert_eq!(checked.expanded.name, "retained-clock");
    assert_eq!(
        fs::read(installed.0.join("body/biography.json")).unwrap(),
        before
    );
    fs::remove_file(installed.0.join("body/source.conduit")).unwrap();
    assert!(super::checked(&installed.0).unwrap().is_none());
}

#[test]
fn unrelated_valid_or_invalid_source_refuses_before_parsing() {
    let installed = Installed::new(SOURCE);
    for source in [OTHER, "plot unfinished {", ""] {
        fs::write(installed.0.join("body/source.conduit"), source).unwrap();
        assert_eq!(
            checked(&installed.0).err().unwrap(),
            "retained Source bytes differ from the installed Body workset"
        );
    }
}

#[test]
fn matching_source_identity_does_not_replace_checked_identity() {
    let installed = Installed::new(OTHER);
    assert_eq!(
        checked(&installed.0).err().unwrap(),
        "retained checked Plot differs from the installed Body workset"
    );
}

#[test]
fn a_file_without_an_installed_body_is_not_source_authority() {
    let installed = Installed::new(SOURCE);
    let path = installed.0.join("installation.json");
    let mut installation: Installation = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    installation.body_state = None;
    write_json_atomic(&path, &installation).unwrap();
    assert_eq!(
        checked(&installed.0).err().unwrap(),
        "retained Source has no installed Body owner"
    );
}

#[test]
fn corrupt_authority_and_oversized_source_do_not_enter_the_checker() {
    let installed = Installed::new(SOURCE);
    fs::write(
        installed.0.join("body/source.conduit"),
        vec![b' '; super::super::MAXIMUM_SOURCE as usize + 1],
    )
    .unwrap();
    assert_eq!(
        checked(&installed.0).err().unwrap(),
        "retained Source capture: Bytes"
    );
    let path = installed.0.join("installation.json");
    let mut installation: Installation = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    installation.body_state.as_mut().unwrap().biography_sha256 = digest(b"wrong biography");
    write_json_atomic(&path, &installation).unwrap();
    assert_eq!(
        checked(&installed.0).err().unwrap(),
        "retained body biography identity is invalid or stale"
    );
}

#[test]
fn captured_source_is_immutable_and_does_not_promote_checked_identity() {
    let installed = Installed::new(SOURCE);
    let before = fs::read(installed.0.join("body/biography.json")).unwrap();
    let captured = capture(&installed.0).unwrap().unwrap();
    fs::write(installed.0.join("body/source.conduit"), OTHER).unwrap();
    let expanded = captured.expand().unwrap();
    assert_eq!(expanded[0].plot.expanded.name, "retained-clock");
    assert_eq!(
        fs::read(installed.0.join("body/biography.json")).unwrap(),
        before
    );
    assert!(capture(&installed.0).is_err());

    let mismatched = Installed::new(OTHER);
    // Matching Source bytes can be acquired without compiling. They cannot
    // replace the independently retained checked entry identity.
    let captured = capture(&mismatched.0).unwrap().unwrap();
    assert_eq!(
        captured.expand().err().unwrap(),
        "retained checked Plot differs from the installed Body workset"
    );
}
