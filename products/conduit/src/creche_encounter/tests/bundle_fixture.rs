use super::*;
pub(super) fn factory() -> (CrecheEncounterFactory, bundle_common::PrivateDirectory) {
    factory_for(source(), SporeOutputKind::NativeBundle)
}
pub(super) fn factory_for(
    source: &str,
    output: SporeOutputKind,
) -> (CrecheEncounterFactory, bundle_common::PrivateDirectory) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "conduit-creche-artifact-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut artifact = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    artifact.write_all(b"artifact").unwrap();
    std::fs::remove_file(&path).unwrap();
    let directory = bundle_common::PrivateDirectory::new(path.with_extension("packages"));
    let configuration = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(source).unwrap(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let (image, image_manifest) = build_host_image(
        configuration.profile().clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
        &output,
        &BuildInputs {
            source_identity: "git:reviewed".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    let release = bundle_common::release(&image, b"artifact");
    let files =
        std::collections::BTreeMap::from([("fixture.data".into(), artifact.try_clone().unwrap())]);
    let bundle = if output == SporeOutputKind::Uf2 {
        CrecheReviewedBundle::artifact(release, files, directory.0.clone())
    } else {
        CrecheReviewedBundle::native(release, files, directory.0.clone())
    }
    .unwrap();
    let factory = CrecheEncounterFactory::new(
        conduit_workspace_make::catalog(),
        conduit_workspace_make::package_set(),
        CrecheEncounterInstallation {
            encounter_id: "encounter/test".into(),
            resource_pool: "pool/creche".into(),
            limits: CrecheLimits {
                maximum_evidence_bytes: 65536,
                maximum_polls: 1000,
                minimum_poll_interval_ms: 1,
            },
            production_bounds: JoiningProductionBounds {
                maximum_artifact_bytes: 128 * 1024,
                maximum_manifest_bytes: 65536,
                maximum_receipt_bytes: 65536,
            },
            initial_source: Some(source.into()),
        },
        vec![CrecheReviewedArtifact {
            bundle: Some(bundle),
            configuration,
            output,
            image,
            image_manifest,
            artifact,
            artifact_digest: format!("sha256:{:x}", Sha256::digest(b"artifact")),
            artifact_bytes: 8,
            source_identity: "git:reviewed".into(),
        }],
    )
    .unwrap();
    (factory, directory)
}
