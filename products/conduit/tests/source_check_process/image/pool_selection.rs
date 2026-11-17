//! The compiler keeps its sufficient pool while unrelated owners are present.
use super::*;
#[test]
fn image_definition_selects_only_the_unique_sufficient_original_compiler_pool() {
    let checked = configuration();
    let mut source = checked.configuration().clone();
    let original = source
        .resources
        .iter()
        .find(|p| p.class == RUNTIME_MEMORY_RESOURCE_CLASS)
        .unwrap()
        .clone();
    for (id, bytes) in [
        ("ambient/memory", 64 * 1024 * 1024),
        ("ambient/facets", 3130972),
    ] {
        let mut pool = original.clone();
        pool.id = id.into();
        pool.slots = bytes;
        pool.bytes = u64::from(bytes);
        source.resources.push(pool);
    }
    let selected = check_host_configuration(
        source.clone(),
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let image = image(&selected);
    let (_, actual) = definition(&selected, &image);
    assert_eq!(actual.base.resources[1].pool_id.as_str(), original.id);
    assert_eq!(actual.base.resources[1].capacity_units, original.slots);
    let mut second = original;
    second.id.push_str("/another-sufficient-pool");
    source.resources.push(second);
    let ambiguous = check_host_configuration(
        source,
        &conduit_workspace_make::catalog(),
        &conduit_workspace_make::package_set(),
    )
    .unwrap();
    let image = super::image(&ambiguous);
    let (config, definition) = super::definition(&selected, &super::image(&selected));
    let error = conduit::source_check_worker::derive_image_definition(
        &config,
        &image,
        &ambiguous,
        &conduit_workspace_make::catalog(),
        &SourceCheckNativeSchema::prepare().unwrap(),
        definition.grant,
        conduit::source_check_worker::SourceCheckProviderIdentity {
            instance: "fixture/current-provider".into(),
            generation: 1,
            resource_generation: ResourceGenerationId("fixture/current-resources".into()),
        },
    )
    .err()
    .expect("two sufficient pools must remain ambiguous");
    assert!(error.contains("ambiguous"), "{error}");
}
