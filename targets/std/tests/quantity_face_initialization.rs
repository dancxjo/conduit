//! Public checked-IMAGE projection admission. No Source, Play, owner or Show proof.
use conduit_core::{
    BaseLifecycle, OfferGeneration, FACE_RESOURCE_CLASS, RUNTIME_MEMORY_RESOURCE_CLASS,
};
use conduit_host_make::{
    build_default_host_image, check_host_configuration, parse_host_configuration_conduit,
    BuildInputs, CheckedHostConfiguration, HostImage, MakeCatalog,
};
use conduit_std_host::{StdHost, StdHostConfig};
use conduit_std_offers::{
    QUANTITY_OBSERVATION_FACE_BASE as BASE, QUANTITY_OBSERVATION_FACE_DRIVER as DRIVER,
    QUANTITY_OBSERVATION_FACE_IMPLEMENTATION as IMPLEMENTATION,
};

const FACE_POOL: &str = "quantity/face";
const MEMORY_POOL: &str = "quantity/projection-memory";

fn config() -> StdHostConfig {
    StdHostConfig {
        host_id: "fixture/quantity-face".into(),
        boot_id: "boot/quantity-face".into(),
        offer_generation: OfferGeneration(3),
    }
}

fn source() -> String {
    let workspace = conduit_semantic_catalog::quantity_observation_face_workspace_bytes().unwrap();
    format!(
        r#"host quantity-face-fixture {{
  schema = 1
  target = {{architecture: "x86_64", machine: "computer", os: "linux"}}
  base = {{kind: "timer/monotonic", implementation: "hosted/monotonic-clock@1"}}
  base = {{kind: "{BASE}", implementation: "{DRIVER}", initialization: {{face_pool: "{FACE_POOL}", memory_pool: "{MEMORY_POOL}"}}}}
  capability = {{kind: "time/every", pinned_implementation: "{time}"}}
  capability = {{kind: "{BASE}", pinned_implementation: "{IMPLEMENTATION}"}}
  need = {{id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}}
  need = {{id: "{FACE_POOL}", class: "{FACE_RESOURCE_CLASS}", slots: 1, bytes: 1}}
  need = {{id: "{MEMORY_POOL}", class: "{RUNTIME_MEMORY_RESOURCE_CLASS}", slots: {workspace}, bytes: {workspace}}}
  limits = {{static_memory_bytes: 16777216, heap_arena_bytes: 134217728, queue_items: 4096, buffered_bytes: 16777216, active_instances: 512, operation_slots: 256, timer_slots: 128, line_sessions: 64, evidence_items: 4096}}
}}"#,
        time = conduit_std_offers::TIME_EVERY_IMPLEMENTATION,
    )
}

fn fixture(source: &str) -> Result<(CheckedHostConfiguration, HostImage, MakeCatalog), String> {
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let parsed =
        parse_host_configuration_conduit(source).map_err(|why| format!("parse: {why:?}"))?;
    let checked = check_host_configuration(parsed, &catalog, &packages)
        .map_err(|why| format!("check: {why:?}"))?;
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/quantity-face-initialization".into(),
            toolchain_available: true,
        },
    )
    .map_err(|why| format!("build: {why:?}"))?;
    Ok((checked, image, catalog))
}

fn initialize(source: &str) -> Result<StdHost, String> {
    let (checked, image, catalog) = fixture(source)?;
    StdHost::from_selected_quantity_observation_image(config(), &image, &catalog, &checked)
}

fn assert_refused(source: &str) {
    // Syntax must remain valid: refusal belongs to ordinary checking/admission.
    parse_host_configuration_conduit(source).unwrap();
    assert!(initialize(source).is_err(), "invalid selection initialized");
}

#[test]
fn checked_quantity_image_advertises_one_ready_base_and_exact_finite_named_pools() {
    let (checked, image, catalog) = fixture(&source()).unwrap();
    let host =
        StdHost::from_selected_quantity_observation_image(config(), &image, &catalog, &checked)
            .unwrap();
    let advertisement = host.advertisement();
    assert_eq!(advertisement.host_id, config().host_id);
    assert_eq!(advertisement.boot_id, config().boot_id);
    assert_eq!(advertisement.offer_generation, config().offer_generation);
    assert!(host.image_identity().is_some());
    let bases: Vec<_> = advertisement
        .bases
        .iter()
        .filter(|base| base.mechanism_family.as_str() == BASE)
        .collect();
    assert_eq!(bases.len(), 1);
    let base = bases[0];
    assert_eq!(base.lifecycle, BaseLifecycle::Ready);
    assert_eq!(base.implementation_id.as_str(), DRIVER);
    assert_eq!(
        base.capability_ids
            .iter()
            .map(|id| id.as_str())
            .collect::<Vec<_>>(),
        [IMPLEMENTATION]
    );
    let mut pools: Vec<_> = base
        .resource_pool_ids
        .iter()
        .map(|id| id.as_str())
        .collect();
    pools.sort_unstable();
    assert_eq!(pools, [FACE_POOL, MEMORY_POOL]);
    let offer = conduit_std_offers::quantity_observation_face_offer().unwrap();
    let advertised: Vec<_> = advertisement
        .capabilities
        .iter()
        .filter(|candidate| candidate.implementation.implementation_id.as_str() == IMPLEMENTATION)
        .collect();
    assert_eq!(advertised, [&offer]);
    for requirement in &offer.resource_requirements {
        let pool_id = match requirement.class_id.as_str() {
            FACE_RESOURCE_CLASS => FACE_POOL,
            RUNTIME_MEMORY_RESOURCE_CLASS => MEMORY_POOL,
            other => panic!("unexpected resource class {other}"),
        };
        let matches: Vec<_> = advertisement
            .resources
            .iter()
            .filter(|pool| pool.pool_id.as_str() == pool_id)
            .collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].class_id, requirement.class_id);
        assert_eq!(matches[0].capacity_units, requirement.units);
        assert!(matches[0].compute.is_none() && matches[0].content.is_none());
    }
    assert!(offer.authority_requirements.is_empty());
    assert!(host.native_source_worker_inspection().is_none());
}

#[test]
fn named_projection_pools_must_exist_be_distinct_and_cover_the_exact_workspace() {
    let exact = source();
    assert!(initialize(&exact).is_ok());
    for missing in [FACE_POOL, MEMORY_POOL] {
        let source = exact
            .lines()
            .filter(|line| !line.contains(&format!("need = {{id: \"{missing}\"")))
            .collect::<Vec<_>>()
            .join("\n");
        assert_refused(&source);
    }
    assert_refused(&exact.replace(
        &format!("memory_pool: \"{MEMORY_POOL}\""),
        &format!("memory_pool: \"{FACE_POOL}\""),
    ));
    assert_refused(&exact.replace("slots: 1, bytes: 1", "slots: 1, bytes: 0"));
    let workspace = conduit_semantic_catalog::quantity_observation_face_workspace_bytes().unwrap();
    assert!(workspace > 1);
    for (slots, bytes) in [(workspace - 1, workspace), (workspace, workspace - 1)] {
        assert_refused(&exact.replace(
            &format!("slots: {workspace}, bytes: {workspace}"),
            &format!("slots: {slots}, bytes: {bytes}"),
        ));
    }
    assert_refused(&exact.replace(
        &format!("id: \"{FACE_POOL}\", class: \"{FACE_RESOURCE_CLASS}\""),
        &format!("id: \"{FACE_POOL}\", class: \"{RUNTIME_MEMORY_RESOURCE_CLASS}\""),
    ));
}

#[test]
fn checked_configuration_and_image_must_be_the_same_selection() {
    let source = source();
    let (_, image, catalog) = fixture(&source).unwrap();
    let (other, _, _) = fixture(&source.replace(FACE_POOL, "quantity/other-face")).unwrap();
    assert!(
        StdHost::from_selected_quantity_observation_image(config(), &image, &catalog, &other)
            .is_err()
    );
}

#[test]
fn simultaneous_ambient_projection_family_is_not_a_quantity_selection() {
    let extra = format!(
        "  base = {{kind: \"{}\", implementation: \"{}\"}}\n  capability = {{kind: \"{}\", pinned_implementation: \"{}\"}}\n",
        conduit_std_offers::AMBIENT_HOME_FACE_BASE,
        conduit_std_offers::AMBIENT_HOME_FACE_DRIVER,
        conduit_semantic_catalog::AMBIENT_CLOCK_FACE_KIND,
        conduit_std_offers::AMBIENT_CLOCK_FACE_IMPLEMENTATION,
    );
    let source = source().replacen("  limits =", &format!("{extra}  limits ="), 1);
    assert_refused(&source);
}
