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

#[test]
fn only_initialized_resident_software_lends_exact_host_free_offer_bytes() {
    use conduit_environment_runtime::appliance::assigned::PreparedResidentSoftware;
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
    let recipe = prepared.software_recipe().unwrap();
    let resident = recipe.resident();
    let texts: Vec<_> = recipe.resident_computations().collect();
    let owner = PreparedResidentSoftware::prepare(
        &resident,
        &texts,
        &conduit_time::TickConfiguration {
            count: 1,
            period_ms: 0,
        },
        conduit_environment_runtime::appliance::prepare_value_storage().unwrap(),
    )
    .unwrap();
    let offers = recipe.resident_offers();
    let (view, memory) = allocation_probe::observe(|| owner.offers(&resident, &offers).unwrap());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(view.iter().len(), 13);
    let mut decoded: Vec<c::CapabilityOffer> = view
        .iter()
        .map(|json| serde_json::from_slice(json).unwrap())
        .collect();
    let mut expected: Vec<_> = host
        .capabilities
        .iter()
        .filter(|offer| {
            offer.implementation.artifact_id.as_str()
                == conduit_environment_runtime::appliance::software::ARTIFACT
        })
        .cloned()
        .collect();
    decoded.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    expected.sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    assert_eq!(decoded, expected);
    assert_eq!(
        view.array_bytes(),
        serde_json::to_vec(&decoded).unwrap().len()
    );
    eprintln!("resident_software_offer_array_bytes={}", view.array_bytes());
    let mut duplicate = offers.clone();
    duplicate[1] = duplicate[0];
    let (_, memory) = allocation_probe::observe(|| {
        assert!(owner
            .offers(&resident, &offers[..offers.len() - 1])
            .is_err());
        assert!(owner.offers(&resident, &duplicate).is_err());
        assert_eq!(view.iter().count(), 13);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(effects.get(), 0);
}

#[test]
fn source_bootstrap_initializes_before_planning_then_accepts_exact_realization() {
    use conduit_environment_make::NativeSoftwareSourceRecipe;
    use conduit_environment_runtime::appliance::assigned::{
        PreparedResidentSoftware, ValidatedAssignedAppliance,
    };
    let source = construction::compile_default().unwrap();
    // No Host, native owner, advertisement or Plan exists at compilation time.
    let bootstrap = NativeSoftwareSourceRecipe::prepare(&source).unwrap();
    let texts: Vec<_> = bootstrap.resident_computations().collect();
    let mut owner = Some(
        PreparedResidentSoftware::prepare_source(
            &bootstrap.spec(),
            &texts,
            conduit_environment_runtime::appliance::prepare_value_storage().unwrap(),
        )
        .unwrap(),
    );
    let offers = bootstrap.resident_offers();
    let mut changed = bootstrap.spec();
    changed.cadence.period_ms += 1;
    let (refused, memory) =
        allocation_probe::observe(|| owner.as_ref().unwrap().source_offers(&changed, &offers));
    assert!(refused.is_err());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));

    let mut caps: Vec<c::CapabilityOffer> = owner
        .as_ref()
        .unwrap()
        .source_offers(&bootstrap.spec(), &offers)
        .unwrap()
        .iter()
        .map(|j| serde_json::from_slice(j).unwrap())
        .collect();
    let mut rust = String::new();
    bootstrap.write_source_rust(&mut rust).unwrap();
    if let Some(path) = std::env::var_os("CONDUIT_SOURCE_RECIPE_PROOF_OUTPUT") {
        std::fs::write(path, &rust).unwrap();
    }
    assert!(!rust.contains("ResidentLayout"));
    let (native, effects) = pulse::ready(0);
    let mut host = appliance_support::native_advertisement(&native);
    host.capabilities.append(&mut caps);
    let prepared = PreparedNativeAppliance::prepare(
        &source,
        &host,
        &["conduit.base/local@1".into()],
        &[pulse::grant()],
        limits(),
    )
    .unwrap();
    let realized = prepared.software_recipe().unwrap();
    assert_eq!(bootstrap.spec().roles, realized.resident().roles);
    assert_eq!(bootstrap.spec().source, realized.resident().source);
    assert_eq!(
        bootstrap.spec().computations.requirements,
        realized.resident().computations.requirements
    );
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(host.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(host.boot_id.as_str()),
        host_calls: &prepared
            .image()
            .host_calls
            .iter()
            .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
            .collect::<Vec<_>>(),
        resources: &prepared
            .image()
            .resources
            .iter()
            .map(|r| r.resource)
            .collect::<Vec<_>>(),
        remote_bindings: &[],
    };
    let selected = ValidatedAssignedAppliance::admit(
        prepared.assignment(),
        limits().assignment,
        required,
        &realized.resident(),
    )
    .unwrap();
    let (result, memory) =
        allocation_probe::observe(|| PreparedResidentSoftware::bind_retained(&mut owner, selected));
    assert!(result.is_ok());
    assert!(owner.is_none());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(effects.get(), 0);
    let mut stale = construction::compile(1, 0).unwrap();
    stale.count = 2;
    assert!(NativeSoftwareSourceRecipe::prepare(&stale).is_err());
}
