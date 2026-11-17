//! Global pool order is independent of initialized Base publication order.
use super::*;
use conduit_core::{
    BaseEnforcementClass, BaseLifecycle, BaseProviderEntry, OfferGeneration, TIMER_RESOURCE_CLASS,
};

fn timer(base: &str, pool: &str, publishes_tick: bool) -> BaseProviderEntry {
    BaseProviderEntry {
        base_id: base.into(),
        provider_instance_id: format!("{base}/provider").into(),
        provider_generation: 1,
        implementation_id: "hosted/monotonic-clock@1".into(),
        mechanism_family: "timer/monotonic".into(),
        enforcement_class: BaseEnforcementClass::Cooperative,
        lifecycle: BaseLifecycle::Ready,
        capabilities: if publishes_tick {
            vec![conduit_std_offers::tick_capability_offer()]
        } else {
            vec![]
        },
        resources: vec![conduit_core::resource_offer(pool, TIMER_RESOURCE_CLASS, 8)],
    }
}

#[test]
fn two_initialized_bases_project_canonical_global_pools_without_rebinding_owners() {
    let mut profile = parse(STD_COMPUTER);
    let mut second = profile
        .bases
        .iter()
        .find(|base| base.kind == "timer/monotonic")
        .unwrap()
        .clone();
    second.id = "timer/second".into();
    profile.bases.push(second);
    let (image, bytes) = test_build_host_image(profile, &test_catalog(), &build_inputs()).unwrap();
    let first = timer("timer/main", "z/timer", true);
    let second = timer("timer/second", "a/timer", false);
    for bases in [
        vec![first.clone(), second.clone()],
        vec![second.clone(), first.clone()],
    ] {
        let bound = bind_runtime_offer(
            &image.manifest,
            &image,
            &bytes,
            &test_catalog(),
            RuntimeOfferInputs {
                host_id: "host/pool-order".into(),
                boot_id: "boot/pool-order".into(),
                offer_generation: OfferGeneration(1),
                offer_sign_id: "sign/pool-order".into(),
                host_profile: "std/runtime-bound@1".into(),
                candidate_bases: bases,
                candidate_capabilities: vec![],
                planner_capabilities: vec![],
                facts: tick_runtime_facts(true),
            },
        )
        .unwrap();
        let ad = bound.advertisement();
        assert_eq!(
            ad.resources
                .iter()
                .map(|resource| resource.pool_id.as_str())
                .collect::<Vec<_>>(),
            ["a/timer", "z/timer"]
        );
        assert_eq!(
            ad.resources.iter().map(|r| r.capacity_units).sum::<u32>(),
            16
        );
        for original in [&first, &second] {
            let published = ad
                .bases
                .iter()
                .find(|b| b.base_id == original.base_id)
                .unwrap();
            assert_eq!(
                published.provider_instance_id,
                original.provider_instance_id
            );
            assert_eq!(
                published.resource_pool_ids,
                [original.resources[0].pool_id.clone()]
            );
        }
    }
}
