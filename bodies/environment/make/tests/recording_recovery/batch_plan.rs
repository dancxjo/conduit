//! Production retained software and ordinary Plan; external storage/grants
//! are explicitly fixtures, not a browser store or a real Body installation.
use super::*;
use conduit_environment_make::RecordingBatchRecoverySource;
use std::collections::BTreeMap;
fn select<B>(
    owner: &ReadyRecordingSoftware<B>,
    choices: &mut BTreeMap<c::GearId, conduit_planner::PlacementChoice>,
) {
    let (gear, host, capability) = owner.current_selection().unwrap();
    assert!(choices
        .insert(
            gear.clone(),
            conduit_planner::PlacementChoice {
                host_id: host.clone(),
                capability_id: capability.clone()
            }
        )
        .is_none());
}
pub struct Fixture {
    pub source: RecordingBatchRecoverySource,
    pub host: c::HostAdvertisement,
    pub grants: Vec<c::AuthorityGrant>,
    pub choices: conduit_planner::PlacementChoices,
    pub generations: Vec<ReadyRecordingGenerations>,
    pub singleton: Vec<ReadyRecordingSingleton>,
    pub concat: Vec<ReadyRecordingConcat>,
    pub recovery: ReadyRecordingRecovery,
}
pub fn fixture(references: &[c::BoundedResourceRef]) -> Fixture {
    let source = RecordingBatchRecoverySource::compile(references).unwrap();
    let (mut host, _) = fixture_store(&references[0]);
    host.capabilities.clear();
    host.resources.clear();
    let mut grants = Vec::new();
    let mut choices = BTreeMap::new();
    let mut generations = Vec::new();
    let mut singleton = Vec::new();
    let mut concat = Vec::new();
    for (index, reference) in references.iter().enumerate() {
        let (mut store, mut grant) = fixture_store(reference);
        let capability: c::CapabilityId = format!("fixture/snapshot/read/{index}").into();
        store.capabilities[0].capability_id = capability.clone();
        grant.capability_id = capability.clone();
        grant.grant_id = format!("fixture/grant/{index}").into();
        store.resources[0].pool_id = format!("fixture/pool/{index}").into();
        let base: c::HostBaseId = format!("fixture/read-base/{index}").into();
        store.resources[0].content.as_mut().unwrap().base_id = base.clone();
        host.bases.push(c::BaseProviderAdvertisement {
            base_id: base,
            provider_instance_id: format!("fixture/read-provider/{index}").into(),
            provider_generation: 1,
            implementation_id: "fixture/snapshot/base@1".into(),
            mechanism_family: "fixture/indexeddb-generation".into(),
            enforcement_class: c::BaseEnforcementClass::Cooperative,
            lifecycle: c::BaseLifecycle::Ready,
            capability_ids: vec![capability.clone()],
            resource_pool_ids: vec![store.resources[0].pool_id.clone()],
        });
        host.capabilities.extend(store.capabilities);
        host.resources.extend(store.resources);
        grants.push(grant);
        let hex: String = reference
            .encode()
            .unwrap()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let read = source
            .expanded()
            .gears
            .iter()
            .find(|g| {
                g.kind_id.as_str() == "resource/read-json-snapshot"
                    && g.configuration.iter().any(|entry| {
                        entry.key == "reference"
                            && entry.value == c::ConfigurationValue::Text(hex.clone())
                    })
            })
            .unwrap();
        choices.insert(
            read.gear_id.clone(),
            conduit_planner::PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: capability,
            },
        );
        let generation = &source
            .expanded()
            .connections
            .iter()
            .find(|cord| cord.sink_gear_id == read.gear_id)
            .unwrap()
            .source_gear_id;
        let mut owner = ReadyRecordingGenerations::prepare_generation_for(
            source.expanded(),
            host.host_id.clone(),
            host.boot_id.clone(),
            generation,
            reference,
        )
        .unwrap();
        assert!(owner.current_selection().is_none());
        owner.append_to_advertisement(&mut host).unwrap();
        select(&owner, &mut choices);
        generations.push(owner);
    }
    let wrong_role = &source
        .expanded()
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "resource/read-json-snapshot")
        .unwrap()
        .gear_id;
    assert!(matches!(
        ReadyRecordingGenerations::prepare_generation_for(
            source.expanded(),
            host.host_id.clone(),
            host.boot_id.clone(),
            wrong_role,
            &references[0]
        ),
        Err(RecoveryInstallationRefusal::Source)
    ));
    if references.len() > 1 {
        assert!(ReadyRecordingGenerations::prepare_generation(
            source.expanded(),
            host.host_id.clone(),
            host.boot_id.clone(),
            &references[0]
        )
        .is_err());
    }
    for gear in &source.expanded().gears {
        match gear.kind_id.as_str() {
            "flow/singleton" => {
                let mut owner = ReadyRecordingSingleton::prepare_singleton_for(
                    source.expanded(),
                    host.host_id.clone(),
                    host.boot_id.clone(),
                    &gear.gear_id,
                )
                .unwrap();
                owner.append_to_advertisement(&mut host).unwrap();
                select(&owner, &mut choices);
                singleton.push(owner);
            }
            "flow/concat/finite" => {
                let mut owner = ReadyRecordingConcat::prepare_concat_for(
                    source.expanded(),
                    host.host_id.clone(),
                    host.boot_id.clone(),
                    &gear.gear_id,
                )
                .unwrap();
                owner.append_to_advertisement(&mut host).unwrap();
                select(&owner, &mut choices);
                concat.push(owner);
            }
            _ => {}
        }
    }
    let mut recovery = ReadyRecordingRecovery::prepare(
        source.expanded(),
        host.host_id.clone(),
        host.boot_id.clone(),
        context(),
        &ObservationContract::prepare().unwrap(),
        references.len() as u16,
    )
    .unwrap();
    recovery.append_to_advertisement(&mut host).unwrap();
    select(&recovery, &mut choices);
    host.bases
        .sort_by(|left, right| left.base_id.cmp(&right.base_id));
    host.resources
        .sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
    let choices = conduit_planner::PlacementChoices { by_gear: choices };
    Fixture {
        source,
        host,
        grants,
        choices,
        generations,
        singleton,
        concat,
        recovery,
    }
}
#[test]
fn complete_batch_plan_selects_separate_retained_owners_and_exact_generations() {
    let references: Vec<_> = (1..=15)
        .map(|version| {
            let mut r = reference();
            r.lifetime.version = c::ResourceVersionIdentity::from_digest([version; 32]);
            r
        })
        .collect();
    let Fixture {
        source,
        host,
        grants,
        choices,
        mut generations,
        singleton,
        concat,
        recovery,
    } = fixture(&references);
    assert_eq!(choices.by_gear.len(), 60);
    let prepared =
        PreparedRecordingRecovery::prepare_batch(&source, &host, &choices, &bases(), &grants)
            .unwrap();
    assert!(c::verify_plan(prepared.plan()));
    let fragment = &prepared.plan().fragments[0];
    assert_eq!(fragment.placements.len(), 60);
    assert_eq!(prepared.lowered().host_calls.len(), 15);
    assert_eq!(prepared.lowered().cords.len(), 59);
    for owner in &generations {
        check(owner, fragment);
    }
    for owner in &singleton {
        check(owner, fragment);
    }
    for owner in &concat {
        check(owner, fragment);
    }
    check(&recovery, fragment);
    let mut ambiguous = host.clone();
    ambiguous.bases.clear();
    assert!(matches!(
        PreparedRecordingRecovery::prepare_batch(&source, &ambiguous, &choices, &bases(), &grants),
        Err(E::Planning(
            conduit_planner::PlannerError::InvalidResourceContract(_)
        ))
    ));
    // Every read still needs independent current authority.
    assert!(PreparedRecordingRecovery::prepare_batch(
        &source,
        &host,
        &choices,
        &bases(),
        &grants[..14]
    )
    .is_err());
    // A semantically compatible Back for another immutable generation cannot
    // satisfy this Source's exact configured reference.
    let mut wrong = choices.clone();
    let reads: Vec<_> = source
        .expanded()
        .gears
        .iter()
        .filter(|g| g.kind_id.as_str() == "resource/read-json-snapshot")
        .collect();
    let left = wrong.by_gear[&reads[0].gear_id].clone();
    let right = wrong.by_gear[&reads[1].gear_id].clone();
    wrong.by_gear.insert(reads[0].gear_id.clone(), right);
    wrong.by_gear.insert(reads[1].gear_id.clone(), left);
    assert!(matches!(
        PreparedRecordingRecovery::prepare_batch(&source, &host, &wrong, &bases(), &grants),
        Err(E::ReferenceBinding)
    ));
    let placement = fragment
        .placements
        .iter()
        .find(|gear| &gear.gear_id == generations[0].current_selection().unwrap().0)
        .unwrap()
        .placement_id
        .clone();
    assert!(matches!(
        generations[1].check_selected(fragment, &placement),
        Err(RecoveryInstallationRefusal::Selection)
    ));
    let _transferred = generations[0].take_selected(fragment, &placement).unwrap();
    assert!(generations[0].current_selection().is_none());
    assert!(generations[0].take_selected(fragment, &placement).is_err());
    check(&generations[1], fragment);
}
fn check<B>(owner: &ReadyRecordingSoftware<B>, fragment: &c::PlanFragment) {
    let gear = owner.current_selection().unwrap().0;
    let placement = &fragment
        .placements
        .iter()
        .find(|p| &p.gear_id == gear)
        .unwrap()
        .placement_id;
    owner.check_selected(fragment, placement).unwrap();
}
