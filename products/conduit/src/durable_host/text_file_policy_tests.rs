use super::*;
pub(crate) fn fixture() -> (
    CheckedHostConfiguration,
    HostImage,
    StdHostConfig,
    Selection,
) {
    let source = include_str!("../../../../targets/std/profiles/raspberry-pi-3-tick-workset.host.conduit")
        .replace("  limits =", r#"  base = {kind: "storage/text-generation", implementation: "std/file-data-save-text@1", initialization: {maximum_text_bytes: 256, preparation_bytes: 32768, retained_bytes: 16384}}
  capability = {kind: "data/save/text", pinned_implementation: "std/file-data-save-text@1"}
  need = {id: "recording/generation", class: "data/immutable-generation@1", slots: 1, bytes: 256}
  limits ="#);
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let checked = conduit_host_make::check_host_configuration(
        conduit_host_make::parse_host_configuration_conduit(&source).unwrap(),
        &catalog,
        &packages,
    )
    .unwrap();
    let image = conduit_host_make::build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &conduit_host_make::BuildInputs {
            source_identity: "fixture/local-text-policy".into(),
            toolchain_available: true,
        },
    )
    .unwrap()
    .0;
    let config = StdHostConfig {
        host_id: "fixture/local-root".into(),
        boot_id: "fixture/current-boot".into(),
        offer_generation: OfferGeneration(1),
    };
    let reference = conduit_data::DataReference::new(BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([41; 32]),
        content_profile: kind_id("value/text"),
        access_class: DURABLE_TEXT_RESOURCE.into(),
        extent: ResourceExtent {
            bytes: 11,
            items: None,
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([42; 32]),
            expires_at: None,
        },
    })
    .unwrap()
    .encode()
    .unwrap();
    let mut encoded = [0; 1024];
    let length = base64::engine::general_purpose::STANDARD
        .encode_slice(&reference, &mut encoded)
        .unwrap();
    (
        checked,
        image,
        config,
        Selection {
            directory: std::env::temp_dir()
                .join(fresh_identity("conduit-text-file-policy", "test").replace('/', "-")),
            reference_base64: std::str::from_utf8(&encoded[..length]).unwrap().into(),
        },
    )
}
#[test]
fn local_file_selection_initializes_only_checked_operation_with_actual_directory() {
    let (checked, image, config, selection) = fixture();
    std::fs::create_dir(&selection.directory).unwrap();
    let initialization = prepare(&selection, &config, &image, &checked).unwrap();
    let grant = initialization.definition.authority.clone();
    assert_eq!(grant.host_id, config.host_id);
    assert_eq!(grant.boot_id, config.boot_id);
    assert_eq!(grant.subject_kind.as_str(), DURABLE_TEXT_SAVE_KIND);
    let host = conduit_std_host::StdHost::from_selected_timer_and_text_file_image(
        config,
        &image,
        &conduit_workspace_make::catalog(),
        &checked,
        initialization,
    )
    .unwrap();
    assert_eq!(host.text_file_authority_grant(), Some(&grant));
    assert_eq!(std::fs::read_dir(&selection.directory).unwrap().count(), 0);
    drop(host);
    std::fs::remove_dir(selection.directory).unwrap();
}
#[test]
fn malformed_or_ambiguous_local_selection_refuses_before_directory_access() {
    let (checked, image, config, selection) = fixture();
    for (directory, reference_base64) in [
        ("relative".into(), selection.reference_base64.clone()),
        (selection.directory.clone(), "?".into()),
        (selection.directory.clone(), "A".repeat(1025)),
    ] {
        let error = prepare(
            &Selection {
                directory,
                reference_base64,
            },
            &config,
            &image,
            &checked,
        )
        .err()
        .unwrap();
        assert!(!error.contains("open owner-selected"), "{error}");
    }
    let mut ambiguous = image;
    let mut pool = ambiguous
        .manifest
        .resource_budgets
        .iter()
        .find(|p| p.class == DURABLE_TEXT_RESOURCE)
        .unwrap()
        .clone();
    pool.id.push_str("/other");
    ambiguous.manifest.resource_budgets.push(pool);
    assert!(prepare(&selection, &config, &ambiguous, &checked)
        .err()
        .unwrap()
        .contains("one exact checked generation pool"));
    assert!(!selection.directory.exists());
}
