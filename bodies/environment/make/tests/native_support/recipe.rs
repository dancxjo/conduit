//! CPU ownership/Boot fixtures; no physical readiness or native start is claimed.
use super::*;
#[test]
fn resident_recipe_is_boot_independent_while_assignment_and_plan_remain_current() {
    let source = construction::compile_default().unwrap();
    let (ready, first_effects) = pulse::ready(0);
    let (_software, host) = retained_advertisement(&source, &ready);
    let first = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let first_recipe = first.software_recipe().unwrap();
    let mut identity = pulse::identity();
    identity.boot_id = "fixture/environment/next-boot".into();
    let (next_ready, next_effects) = pulse::ready_with_identity(0, identity.clone());
    let mut next_software =
        conduit_environment_runtime::appliance::software::ReadySoftwareBacks::prepare(
            &source,
            identity.host_id.clone(),
            identity.boot_id.clone(),
        )
        .unwrap();
    let mut next_host = host.clone();
    next_host.boot_id = identity.boot_id.clone();
    // Only the explicitly inert timer fixture is reused. All software and
    // capture offers come from separately retained owners in the new Boot.
    next_host
        .capabilities
        .retain(|offer| offer.kind_id.as_str() == conduit_time::TICK_KIND);
    next_host.bases.clear();
    next_host.resources.clear();
    next_ready
        .append_to_advertisement(
            &mut next_host,
            &conduit_pulse_capture::PulseReadContract::prepare().unwrap(),
        )
        .unwrap();
    next_software
        .append_to_advertisement(&mut next_host)
        .unwrap();
    let mut grant = pulse::grant();
    grant.boot_id = identity.boot_id;
    let second = PreparedNativeAppliance::prepare(
        &source,
        &next_host,
        &["conduit.base/local@1".into()],
        &[grant],
        limits(),
    )
    .unwrap();
    assert_ne!(first.plan().plan_id, second.plan().plan_id);
    assert_ne!(first.assignment(), second.assignment());
    let second_recipe = second.software_recipe().unwrap();
    assert_eq!(first_recipe, second_recipe);
    let mut first_rust = String::new();
    first_recipe.write_resident_rust(&mut first_rust).unwrap();
    let mut second_rust = String::new();
    second_recipe.write_resident_rust(&mut second_rust).unwrap();
    assert_eq!(first_rust, second_rust);
    for identity in [
        host.host_id.as_str(),
        host.boot_id.as_str(),
        first.plan().plan_id.as_str(),
    ] {
        assert!(!first_rust.contains(identity));
    }
    if let Some(path) = std::env::var_os("CONDUIT_RECIPE_PROOF_OUTPUT") {
        std::fs::write(path, &first_rust).unwrap();
    }
    assert_eq!((first_effects.get(), next_effects.get()), (0, 0));
}
#[test]
fn recipe_cadence_is_pinned_and_compiler_role_mapping_refuses_ambiguous_nodes() {
    use conduit_environment_runtime::appliance::assigned::ResidentRoles;
    let source = construction::compile(1, 0).unwrap();
    let (ready, effects) = pulse::ready(0);
    let (_software, host) = retained_advertisement(&source, &ready);
    let prepared = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let mut nodes: Vec<_> = prepared
        .image()
        .nodes
        .iter()
        .map(|n| (n.node, n.kind_id.as_str()))
        .collect();
    nodes[1].0 = nodes[0].0;
    assert!(ResidentRoles::from_nodes(nodes).is_err());
    assert!(ResidentRoles::from_nodes(
        prepared
            .image()
            .nodes
            .iter()
            .take(14)
            .map(|n| (n.node, n.kind_id.as_str()))
    )
    .is_err());
    let next = construction::compile(2, 0).unwrap();
    let (_next_software, next_host) = retained_advertisement(&next, &ready);
    let changed = PreparedNativeAppliance::prepare(
        &next,
        &next_host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    assert_ne!(
        prepared.software_recipe().unwrap(),
        changed.software_recipe().unwrap()
    );
    assert_eq!(effects.get(), 0);
}
