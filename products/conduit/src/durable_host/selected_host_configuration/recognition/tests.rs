use super::*;
use conduit_language::{
    LanguageCoverage, LanguageExternalIdentity, LanguageId, LanguageMappingDeclaration,
};
use conduit_plot::rust_binding::BoundedSequence;
use std::{fs, os::unix::fs::PermissionsExt};

fn fixture() -> (PathBuf, BaseInitialization) {
    let root = std::env::temp_dir().join(format!(
        "conduit-checked-whisper-{}",
        super::super::super::fresh_identity("fixture", "recognition").replace('/', "-")
    ));
    fs::create_dir(&root).unwrap();
    let exe = root.join("whisper-cli");
    // Any attempted recognition would leave this marker; initialization must not spawn.
    fs::write(
        &exe,
        format!(
            "#!/bin/sh\ntouch '{}'\nexit 99\n",
            root.join("spawned").display()
        ),
    )
    .unwrap();
    fs::set_permissions(&exe, fs::Permissions::from_mode(0o700)).unwrap();
    let model = root.join("model.bin");
    fs::write(&model, b"bounded recognition fixture").unwrap();
    let d = WhisperDiscovery::inspect(&exe, &model).unwrap();
    let language = LanguageId::new("language/english".into()).unwrap();
    let mapping = LanguageMappingDeclaration::new(
        LanguageExternalIdentity::new(d.provider_identity(), "en".into()).unwrap(),
        language.clone(),
        None,
    )
    .unwrap();
    let coverage = LanguageCoverage::new(
        d.provider_identity(),
        BoundedSequence::try_from_iter([language]).unwrap(),
        BoundedSequence::try_from_iter([mapping]).unwrap(),
        "fixture@1".into(),
        BoundedSequence::try_from_iter([]).unwrap(),
        false,
    )
    .unwrap();
    let coverage = coverage.encode().unwrap();
    let coverage_file = root.join("coverage.native");
    fs::write(&coverage_file, &coverage).unwrap();
    let init = serde_json::from_value(serde_json::json!({
        "executable":exe,"executable_sha256":d.executable_sha256,
        "model":model,"model_sha256":d.model_sha256,"model_bytes":d.model_bytes,
        "language_coverage":coverage_file,"language_coverage_sha256":super::super::super::digest(&coverage).trim_start_matches("sha256:"),
        "threads":1,"timeout_milliseconds":2000
    })).unwrap();
    (root, init)
}
fn source(init: BaseInitialization) -> String {
    let mut configuration = conduit_host_make::parse_host_configuration_conduit(include_str!(
        "../../../../../../targets/std/profiles/linux-computer.host.conduit"
    ))
    .unwrap();
    configuration
        .bases
        .push(conduit_host_make::ConfigurationBase {
            kind: "speech/recognizer".into(),
            implementation: Some(conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION.into()),
            implementations: vec![],
            initialization: Some(init),
        });
    conduit_host_make::canonical_host_configuration_conduit(&configuration).unwrap()
}
#[test]
fn checked_recognizer_is_exact_and_initializes_without_spawning_or_capture() {
    let (root, init) = fixture();
    let first =
        super::super::Selection::from_source(&source(init.clone()), "std/x86_64/computer").unwrap();
    let mut host = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: "host/checked-recognition".into(),
        boot_id: "boot/checked-recognition".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    first.attach_to_fresh_host(&mut host).unwrap();
    let ad = host.advertisement();
    assert_eq!(ad.offer_generation, conduit_core::OfferGeneration(2));
    assert_eq!(ad.bases.len(), 1);
    assert_eq!(ad.bases[0].lifecycle, conduit_core::BaseLifecycle::Ready);
    assert_eq!(ad.bases[0].capability_ids.len(), 1);
    assert!(ad.bases[0]
        .provider_instance_id
        .as_str()
        .starts_with("whisper/provider/"));
    assert_eq!(
        ad.resources
            .iter()
            .find(|p| p.pool_id.as_str() == "std/whisper-process")
            .unwrap()
            .capacity_units,
        1
    );
    assert!(!root.join("spawned").exists());
    let mut changed = init;
    changed
        .values
        .insert("threads".into(), serde_json::json!(2));
    let next =
        super::super::Selection::from_source(&source(changed), "std/x86_64/computer").unwrap();
    assert_ne!(first.configuration_id, next.configuration_id);
    drop(host);
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn checked_recognizer_refuses_unknown_unbounded_and_nonexact_declarations() {
    let (root, init) = fixture();
    for (key, value) in [
        ("threads", serde_json::json!(0)),
        ("timeout_milliseconds", serde_json::json!(120001)),
        ("model_bytes", serde_json::json!(0)),
        ("unexpected", serde_json::json!(1)),
        ("executable", serde_json::json!("whisper-cli")),
        ("model_sha256", serde_json::json!("unknown")),
    ] {
        let mut changed = init.clone();
        changed.values.insert(key.into(), value);
        assert!(
            super::super::Selection::from_source(&source(changed), "std/x86_64/computer").is_err(),
            "accepted {key}"
        );
    }
    let mut windows =
        conduit_host_make::parse_host_configuration_conduit(&source(init.clone())).unwrap();
    windows.target.machine = "windows-computer".into();
    windows.target.os = Some("windows".into());
    assert!(super::super::Selection::from_source(
        &conduit_host_make::canonical_host_configuration_conduit(&windows).unwrap(),
        "std/x86_64/windows-computer"
    )
    .is_err());
    let mut conflicting =
        conduit_host_make::parse_host_configuration_conduit(&source(init.clone())).unwrap();
    let mut duplicate = conflicting
        .bases
        .iter()
        .find(|b| b.kind == "speech/recognizer")
        .unwrap()
        .clone();
    duplicate
        .initialization
        .as_mut()
        .unwrap()
        .values
        .insert("threads".into(), serde_json::json!(2));
    conflicting.bases.push(duplicate);
    assert!(super::super::Selection::from_source(
        &conduit_host_make::canonical_host_configuration_conduit(&conflicting).unwrap(),
        "std/x86_64/computer"
    )
    .is_err());
    let mut declaration_only = init.clone();
    declaration_only.values.insert(
        "executable".into(),
        serde_json::json!(root.join("missing-executable")),
    );
    assert!(super::super::Selection::from_source(
        &source(declaration_only.clone()),
        "std/x86_64/computer"
    )
    .is_ok());
    assert!(initialize(&declaration_only).is_err());
    let mut changed = init;
    changed.values.remove("language_coverage_sha256");
    assert!(validate(&changed).is_err());
    assert!(!root.join("spawned").exists());
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn changed_missing_or_foreign_recognition_equipment_never_spawns() {
    let (root, init) = fixture();
    let mut changed = init.clone();
    changed
        .values
        .insert("model_sha256".into(), serde_json::json!("a".repeat(64)));
    assert!(initialize(&changed).is_err());
    let mut changed = init.clone();
    changed.values.insert(
        "language_coverage_sha256".into(),
        serde_json::json!("b".repeat(64)),
    );
    assert!(initialize(&changed).is_err());
    fs::write(root.join("model.bin"), b"different model").unwrap();
    assert!(initialize(&init).is_err());
    let mut foreign = init.clone();
    let observed =
        WhisperDiscovery::inspect(root.join("whisper-cli"), root.join("model.bin")).unwrap();
    foreign.values.insert(
        "model_sha256".into(),
        serde_json::json!(observed.model_sha256),
    );
    foreign.values.insert(
        "model_bytes".into(),
        serde_json::json!(observed.model_bytes),
    );
    assert!(initialize(&foreign).err().unwrap().contains("Artifact"));

    fs::remove_file(root.join("whisper-cli")).unwrap();
    assert!(initialize(&init).is_err());
    assert!(!root.join("spawned").exists());
    fs::remove_dir_all(root).unwrap();
}
